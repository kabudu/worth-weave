from datetime import UTC, date, datetime
from decimal import Decimal

import pytest

from ledgerly.domain.models import CanonicalEvent, CoveragePeriod, EventType, Money


def test_money_normalizes_currency_without_losing_decimal_precision() -> None:
    money = Money(amount=Decimal("0.10000001"), currency="gbp")

    assert money.amount == Decimal("0.10000001")
    assert money.currency == "GBP"


def test_coverage_rejects_reversed_period() -> None:
    with pytest.raises(ValueError, match="coverage end"):
        CoveragePeriod(start=date(2026, 7, 11), end=date(2026, 7, 10))


def test_event_rejects_zero_quantity() -> None:
    with pytest.raises(ValueError, match="cannot be zero"):
        CanonicalEvent(
            source_id="trade-1",
            event_type=EventType.BUY,
            occurred_at=datetime.now(UTC),
            description="Example",
            quantity=Decimal("0"),
        )
