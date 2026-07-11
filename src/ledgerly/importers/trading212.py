import csv
import io
from datetime import date

from ledgerly.domain.models import CanonicalEvent, CoveragePeriod, EventType, Money
from ledgerly.importers.base import ImportFormatError, ParsedImport
from ledgerly.importers.common import parse_datetime, parse_decimal, stable_row_id

_REQUIRED_COLUMNS = {"Action", "Time", "ID"}


def _event_type(action: str) -> EventType:
    normalized = action.casefold()
    if "buy" in normalized:
        return EventType.BUY
    if "sell" in normalized:
        return EventType.SELL
    if "dividend" in normalized:
        return EventType.DIVIDEND
    if normalized == "deposit":
        return EventType.DEPOSIT
    if normalized == "withdrawal":
        return EventType.WITHDRAWAL
    if "interest" in normalized:
        return EventType.INTEREST
    if "fee" in normalized:
        return EventType.FEE
    if "split" in normalized or "adjustment" in normalized:
        return EventType.CORPORATE_ACTION
    return EventType.OTHER


def parse_trading212_csv(content: bytes) -> ParsedImport:
    try:
        text = content.decode("utf-8-sig")
    except UnicodeDecodeError as exc:
        raise ImportFormatError("Trading 212 export must be UTF-8 CSV") from exc

    reader = csv.DictReader(io.StringIO(text))
    fields = set(reader.fieldnames or ())
    missing = _REQUIRED_COLUMNS - fields
    if missing:
        missing_names = ", ".join(sorted(missing))
        raise ImportFormatError(f"Trading 212 export is missing columns: {missing_names}")

    events: list[CanonicalEvent] = []
    dates: list[date] = []
    for index, row in enumerate(reader, start=2):
        action = (row.get("Action") or "").strip()
        occurred_at = parse_datetime(row.get("Time") or "", field=f"Time at row {index}")
        dates.append(occurred_at.date())
        raw_values = tuple(row.get(name) or "" for name in (reader.fieldnames or ()))
        source_id = (row.get("ID") or "").strip() or stable_row_id("t212", raw_values)
        amount_value = parse_decimal(row.get("Total"), field=f"Total at row {index}")
        currency = (row.get("Currency (Total)") or "").strip()
        amount = Money(amount_value, currency) if amount_value is not None and currency else None
        quantity = parse_decimal(row.get("No. of shares"), field=f"shares at row {index}")
        instrument_id = (row.get("ISIN") or row.get("Ticker") or "").strip() or None
        events.append(
            CanonicalEvent(
                source_id=f"t212:{source_id}",
                event_type=_event_type(action),
                occurred_at=occurred_at,
                description=action or "Trading 212 event",
                amount=amount,
                quantity=quantity,
                instrument_id=instrument_id,
            )
        )

    if not events:
        raise ImportFormatError("Trading 212 export contains no data rows")
    return ParsedImport(
        coverage=CoveragePeriod(min(dates), max(dates)),
        events=tuple(events),
    )
