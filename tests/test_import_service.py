from sqlalchemy import create_engine, func, select
from sqlalchemy.orm import Session

from ledgerly.domain.models import AccountType, Broker
from ledgerly.persistence.database import Base
from ledgerly.persistence.tables import EventRow, ImportBatchRow
from ledgerly.services.imports import DuplicateImportError, create_account, import_csv


def test_import_is_transactional_and_rejects_duplicate_file() -> None:
    engine = create_engine("sqlite:///:memory:")
    Base.metadata.create_all(engine)
    content = (
        b"Action,Time,ISIN,Ticker,Name,Notes,ID,No. of shares,Total,Currency (Total)\n"
        b"Market buy,2026-07-01 10:00:00,GB00TEST0001,TEST,Example,,trade-1,1,10,GBP\n"
    )

    with Session(engine) as session:
        account = create_account(
            session,
            broker=Broker.TRADING_212,
            account_type=AccountType.STOCKS_AND_SHARES_ISA,
            external_id="trading212-isa",
            display_name="Trading 212 ISA",
        )
        result = import_csv(
            session,
            account_id=account.id,
            filename="export.csv",
            content=content,
        )

        assert result.events_added == 1
        assert session.scalar(select(func.count()).select_from(EventRow)) == 1
        assert session.scalar(select(func.count()).select_from(ImportBatchRow)) == 1

        try:
            import_csv(
                session,
                account_id=account.id,
                filename="renamed.csv",
                content=content,
            )
        except DuplicateImportError:
            pass
        else:
            raise AssertionError("duplicate file should be rejected")
