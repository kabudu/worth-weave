from dataclasses import dataclass
from datetime import date, datetime
from decimal import Decimal
from enum import StrEnum


class Broker(StrEnum):
    TRADING_212 = "trading_212"
    IBKR = "ibkr"


class AccountType(StrEnum):
    INVEST = "invest"
    STOCKS_AND_SHARES_ISA = "stocks_and_shares_isa"


class EventType(StrEnum):
    BUY = "buy"
    SELL = "sell"
    DIVIDEND = "dividend"
    DEPOSIT = "deposit"
    WITHDRAWAL = "withdrawal"
    FEE = "fee"
    INTEREST = "interest"
    CORPORATE_ACTION = "corporate_action"
    TRANSFER = "transfer"
    OTHER = "other"


@dataclass(frozen=True, slots=True)
class Money:
    amount: Decimal
    currency: str

    def __post_init__(self) -> None:
        if len(self.currency) != 3 or not self.currency.isalpha():
            raise ValueError("currency must be a three-letter ISO-style code")
        object.__setattr__(self, "currency", self.currency.upper())


@dataclass(frozen=True, slots=True)
class CoveragePeriod:
    start: date
    end: date

    def __post_init__(self) -> None:
        if self.end < self.start:
            raise ValueError("coverage end cannot precede coverage start")


@dataclass(frozen=True, slots=True)
class CanonicalEvent:
    source_id: str
    event_type: EventType
    occurred_at: datetime
    description: str
    amount: Money | None = None
    quantity: Decimal | None = None
    instrument_id: str | None = None

    def __post_init__(self) -> None:
        if not self.source_id.strip():
            raise ValueError("source_id is required")
        if self.quantity is not None and self.quantity == 0:
            raise ValueError("event quantity cannot be zero")
