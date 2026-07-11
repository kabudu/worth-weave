import hashlib
from dataclasses import dataclass
from uuid import UUID

from sqlalchemy import select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from ledgerly.domain.models import AccountType, Broker
from ledgerly.importers import ParsedImport, parse_ibkr_csv, parse_trading212_csv
from ledgerly.persistence.tables import AccountRow, EventRow, ImportBatchRow


class DuplicateImportError(ValueError):
    pass


class AccountNotFoundError(ValueError):
    pass


@dataclass(frozen=True, slots=True)
class ImportResult:
    batch_id: UUID
    coverage_start: str
    coverage_end: str
    events_added: int
    warnings: tuple[str, ...]


def create_account(
    session: Session,
    *,
    broker: Broker,
    account_type: AccountType,
    external_id: str,
    display_name: str,
) -> AccountRow:
    account = AccountRow(
        broker=broker.value,
        account_type=account_type.value,
        external_id=external_id.strip(),
        display_name=display_name.strip(),
        base_currency="GBP",
    )
    session.add(account)
    session.commit()
    session.refresh(account)
    return account


def _parse(broker: Broker, content: bytes) -> ParsedImport:
    if broker is Broker.TRADING_212:
        return parse_trading212_csv(content)
    return parse_ibkr_csv(content)


def import_csv(
    session: Session,
    *,
    account_id: UUID,
    filename: str,
    content: bytes,
) -> ImportResult:
    account = session.get(AccountRow, account_id)
    if account is None:
        raise AccountNotFoundError("account does not exist")
    digest = hashlib.sha256(content).hexdigest()
    duplicate = session.scalar(
        select(ImportBatchRow.id).where(
            ImportBatchRow.account_id == account_id,
            ImportBatchRow.content_sha256 == digest,
        )
    )
    if duplicate is not None:
        raise DuplicateImportError("this file has already been imported for the account")

    parsed = _parse(Broker(account.broker), content)
    batch = ImportBatchRow(
        account_id=account_id,
        original_filename=filename,
        content_sha256=digest,
        coverage_start=parsed.coverage.start,
        coverage_end=parsed.coverage.end,
    )
    try:
        session.add(batch)
        session.flush()
        existing_ids = set(
            session.scalars(
                select(EventRow.source_id).where(
                    EventRow.account_id == account_id,
                    EventRow.source_id.in_(event.source_id for event in parsed.events),
                )
            )
        )
        new_events = [event for event in parsed.events if event.source_id not in existing_ids]
        session.add_all(
            EventRow(
                account_id=account_id,
                import_batch_id=batch.id,
                source_id=event.source_id,
                event_type=event.event_type.value,
                occurred_at=event.occurred_at,
                description=event.description,
                amount=str(event.amount.amount) if event.amount else None,
                currency=event.amount.currency if event.amount else None,
                quantity=str(event.quantity) if event.quantity is not None else None,
                instrument_id=event.instrument_id,
            )
            for event in new_events
        )
        session.commit()
    except IntegrityError:
        session.rollback()
        raise DuplicateImportError("the import overlaps an existing unique source event") from None

    return ImportResult(
        batch_id=batch.id,
        coverage_start=parsed.coverage.start.isoformat(),
        coverage_end=parsed.coverage.end.isoformat(),
        events_added=len(new_events),
        warnings=parsed.warnings,
    )
