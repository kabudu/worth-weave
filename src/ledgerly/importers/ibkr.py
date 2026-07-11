import csv
import io
import re
from datetime import date

from ledgerly.domain.models import CanonicalEvent, CoveragePeriod, EventType, Money
from ledgerly.importers.base import ImportFormatError, ParsedImport
from ledgerly.importers.common import parse_datetime, parse_decimal, stable_row_id


def _classify_header(header: list[str]) -> str | None:
    fields = set(header)
    if {"TradeID", "BuySell", "TradeMoney"} <= fields:
        return "trades"
    if {"TransactionID", "Amount", "Type", "ActionID"} <= fields:
        return "cash_transactions"
    if {"ActionDescription", "ActionID", "TransactionID"} <= fields:
        return "corporate_actions"
    if {"TransferCompany", "CashTransfer", "TransactionID"} <= fields:
        return "transfers"
    return None


def _is_header(values: list[str]) -> bool:
    if values[0] == "ClientAccountID":
        return True
    fields = set(values)
    return values[0] == "CurrencyPrimary" and {"Conid", "SettlementPolicyMethod"} <= fields


def _ibkr_type(section: str, row: dict[str, str]) -> EventType:
    if section == "trades":
        return EventType.BUY if row.get("BuySell", "").casefold() == "buy" else EventType.SELL
    if section == "corporate_actions":
        return EventType.CORPORATE_ACTION
    if section == "transfers":
        return EventType.TRANSFER
    label = f"{row.get('Type', '')} {row.get('Description', '')}".casefold()
    if "dividend" in label:
        return EventType.DIVIDEND
    if "deposit" in label:
        return EventType.DEPOSIT
    if "withdraw" in label:
        return EventType.WITHDRAWAL
    if "interest" in label:
        return EventType.INTEREST
    if "fee" in label or "commission" in label:
        return EventType.FEE
    return EventType.OTHER


def parse_ibkr_csv(content: bytes) -> ParsedImport:
    try:
        text = content.decode("utf-8-sig")
    except UnicodeDecodeError as exc:
        raise ImportFormatError("IBKR export must be UTF-8 CSV") from exc

    events: list[CanonicalEvent] = []
    dates: list[date] = []
    header: list[str] | None = None
    section: str | None = None
    ignored_sections = 0
    for index, values in enumerate(csv.reader(io.StringIO(text)), start=1):
        if not values:
            continue
        if _is_header(values):
            header = values
            section = _classify_header(values)
            if section is None:
                ignored_sections += 1
            continue
        if header is None:
            raise ImportFormatError("IBKR export does not begin with a recognized section header")
        row = dict(zip(header, values, strict=False))
        for date_field in ("DateTime", "Date", "ReportDate", "TradeDate", "DateOpened"):
            if row.get(date_field) and re.match(r"^20\d{2}-\d{2}-\d{2}", row[date_field]):
                parsed_date = parse_datetime(
                    row[date_field], field=f"{date_field} at row {index}"
                ).date()
                dates.append(parsed_date)
                break
        if section is None:
            continue
        raw_id = row.get("TransactionID") or row.get("TradeID") or row.get("ActionID")
        source_id = raw_id.strip() if raw_id else stable_row_id("ibkr", tuple(values))
        occurred_raw = next(
            (
                row[name]
                for name in ("DateTime", "Date", "ReportDate", "TradeDate")
                if row.get(name)
            ),
            None,
        )
        if occurred_raw is None:
            continue
        amount_value = parse_decimal(
            row.get("Amount") or row.get("NetCash") or row.get("CashTransfer"),
            field=f"amount at row {index}",
        )
        currency = (row.get("CurrencyPrimary") or "").strip()
        amount = Money(amount_value, currency) if amount_value is not None and currency else None
        quantity = parse_decimal(row.get("Quantity"), field=f"Quantity at row {index}")
        if quantity == 0:
            quantity = None
        description = (
            row.get("Description")
            or row.get("ActionDescription")
            or row.get("Type")
            or f"IBKR {section} event"
        )
        events.append(
            CanonicalEvent(
                source_id=f"ibkr:{section}:{source_id}",
                event_type=_ibkr_type(section, row),
                occurred_at=parse_datetime(occurred_raw, field=f"event date at row {index}"),
                description=description,
                amount=amount,
                quantity=quantity,
                instrument_id=(row.get("ISIN") or row.get("Conid") or "").strip() or None,
            )
        )

    if not dates:
        raise ImportFormatError("IBKR export contains no dated data rows")
    warnings = (
        (
            (
                "Non-transaction statement sections were retained for coverage detection "
                "but not ledger events."
            ),
        )
        if ignored_sections
        else ()
    )
    return ParsedImport(
        coverage=CoveragePeriod(min(dates), max(dates)),
        events=tuple(events),
        warnings=warnings,
    )
