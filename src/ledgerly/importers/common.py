import hashlib
from datetime import UTC, datetime
from decimal import Decimal, InvalidOperation

from ledgerly.importers.base import ImportFormatError


def parse_decimal(value: str | None, *, field: str) -> Decimal | None:
    if value is None or not value.strip():
        return None
    try:
        return Decimal(value.replace(",", "").strip())
    except InvalidOperation as exc:
        raise ImportFormatError(f"invalid decimal value in {field}") from exc


def parse_datetime(value: str, *, field: str) -> datetime:
    normalized = value.strip().replace(";", "T")
    if normalized.endswith("Z"):
        normalized = f"{normalized[:-1]}+00:00"
    try:
        parsed = datetime.fromisoformat(normalized)
    except ValueError as exc:
        raise ImportFormatError(f"invalid date/time value in {field}") from exc
    return parsed if parsed.tzinfo else parsed.replace(tzinfo=UTC)


def stable_row_id(prefix: str, row: list[str] | tuple[str, ...]) -> str:
    digest = hashlib.sha256("\x1f".join(row).encode()).hexdigest()
    return f"{prefix}:{digest}"
