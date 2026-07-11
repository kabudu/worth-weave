from decimal import Decimal

import pytest

from ledgerly.domain.models import EventType
from ledgerly.importers import ImportFormatError, parse_ibkr_csv, parse_trading212_csv


def test_trading212_import_normalizes_events_and_coverage() -> None:
    content = (
        b"Action,Time,ISIN,Ticker,Name,Notes,ID,No. of shares,Total,Currency (Total)\n"
        b"Market buy,2026-07-01 10:00:00,GB00TEST0001,TEST,Example,,trade-1,1.25,10.50,GBP\n"
        b"Dividend (Dividend),2026-07-03 10:00:00,GB00TEST0001,TEST,Example,,div-1,,0.25,GBP\n"
    )

    parsed = parse_trading212_csv(content)

    assert parsed.coverage.start.isoformat() == "2026-07-01"
    assert parsed.coverage.end.isoformat() == "2026-07-03"
    assert [event.event_type for event in parsed.events] == [EventType.BUY, EventType.DIVIDEND]
    assert parsed.events[0].quantity == Decimal("1.25")
    assert parsed.events[0].amount is not None
    assert parsed.events[0].amount.amount == Decimal("10.50")


def test_trading212_rejects_unrecognized_schema() -> None:
    with pytest.raises(ImportFormatError, match="missing columns"):
        parse_trading212_csv(b"Action,Time\nDeposit,2026-01-01 00:00:00\n")


@pytest.mark.parametrize(
    ("action", "expected"),
    [
        ("Market sell", EventType.SELL),
        ("Deposit", EventType.DEPOSIT),
        ("Withdrawal", EventType.WITHDRAWAL),
        ("Interest on cash", EventType.INTEREST),
        ("Transaction fee", EventType.FEE),
        ("Stock split open", EventType.CORPORATE_ACTION),
        ("Unknown activity", EventType.OTHER),
    ],
)
def test_trading212_action_mapping(action: str, expected: EventType) -> None:
    content = (
        "Action,Time,ISIN,Ticker,Name,Notes,ID,No. of shares,Total,Currency (Total)\n"
        f"{action},2026-07-01 10:00:00,,,,,event-1,,1,GBP\n"
    ).encode()

    parsed = parse_trading212_csv(content)

    assert parsed.events[0].event_type is expected


def test_importers_reject_invalid_encoding_and_numbers() -> None:
    with pytest.raises(ImportFormatError, match="UTF-8"):
        parse_trading212_csv(b"\xff\xfe")

    content = (
        b"Action,Time,ISIN,Ticker,Name,Notes,ID,No. of shares,Total,Currency (Total)\n"
        b"Market buy,2026-07-01 10:00:00,,,,,event-1,not-a-number,1,GBP\n"
    )
    with pytest.raises(ImportFormatError, match="invalid decimal"):
        parse_trading212_csv(content)


def test_ibkr_import_uses_transaction_sections_and_coverage_sections() -> None:
    content = (
        b"ClientAccountID,CurrencyPrimary,AccountType,DateOpened\n"
        b"U1,GBP,Individual,2024-03-15\n"
        b"ClientAccountID,CurrencyPrimary,TradeID,BuySell,TradeMoney,DateTime,Quantity,"
        b"NetCash,Description,ISIN,Conid\n"
        b"U1,GBP,T1,BUY,100.00,2024-03-19;10:30:00,2,-101.00,Example,GB00TEST0001,1\n"
        b"ClientAccountID,CurrencyPrimary,TransactionID,Amount,Type,ActionID,DateTime,"
        b"Description,ISIN\n"
        b"U1,GBP,C1,1.50,Dividends,A1,2024-03-25;09:00:00,Example dividend,GB00TEST0001\n"
    )

    parsed = parse_ibkr_csv(content)

    assert parsed.coverage.start.isoformat() == "2024-03-15"
    assert parsed.coverage.end.isoformat() == "2024-03-25"
    assert [event.event_type for event in parsed.events] == [EventType.BUY, EventType.DIVIDEND]
    assert len({event.source_id for event in parsed.events}) == 2
    assert parsed.warnings


def test_ibkr_accepts_instrument_header_without_account_id() -> None:
    content = (
        b"ClientAccountID,CurrencyPrimary,AccountType,DateOpened\n"
        b"U1,GBP,Individual,2024-03-15\n"
        b"CurrencyPrimary,AssetClass,Symbol,Conid,SettlementPolicyMethod\n"
        b"GBP,STK,TEST,1,Physical\n"
    )

    parsed = parse_ibkr_csv(content)

    assert parsed.coverage.start.isoformat() == "2024-03-15"
    assert parsed.events == ()
