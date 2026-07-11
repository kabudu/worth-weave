from collections.abc import Generator
from pathlib import Path

from fastapi.testclient import TestClient
from sqlalchemy import create_engine
from sqlalchemy.orm import Session
from sqlalchemy.pool import StaticPool

from ledgerly.api.main import create_app
from ledgerly.config import get_settings
from ledgerly.persistence.database import Base, get_session
from ledgerly.persistence.database import engine as application_engine


def test_health_and_empty_summary(tmp_path: Path, monkeypatch: object) -> None:
    monkeypatch.setenv("LEDGERLY_DATABASE_PATH", str(tmp_path / "test.db"))  # type: ignore[attr-defined]
    monkeypatch.setenv("LEDGERLY_ENVIRONMENT", "test")  # type: ignore[attr-defined]
    get_settings.cache_clear()

    with TestClient(create_app()) as client:
        health = client.get("/health")
        summary = client.get("/api/v1/portfolio/summary")

    assert health.status_code == 200
    assert health.json() == {"status": "ok", "database": "ok", "mode": "local"}
    assert summary.status_code == 200
    assert summary.json()["data_status"] == "awaiting_imports"
    application_engine.dispose()


def test_create_account_and_upload_import() -> None:
    engine = create_engine(
        "sqlite://",
        connect_args={"check_same_thread": False},
        poolclass=StaticPool,
    )
    Base.metadata.create_all(engine)

    def test_session() -> Generator[Session]:
        with Session(engine) as session:
            yield session

    app = create_app()
    app.dependency_overrides[get_session] = test_session
    content = (
        b"Action,Time,ISIN,Ticker,Name,Notes,ID,No. of shares,Total,Currency (Total)\n"
        b"Market buy,2026-07-01 10:00:00,GB00TEST0001,TEST,Example,,trade-1,1,10,GBP\n"
    )

    with TestClient(app) as client:
        created = client.post(
            "/api/v1/accounts",
            json={
                "broker": "trading_212",
                "account_type": "stocks_and_shares_isa",
                "external_id": "t212-isa",
                "display_name": "Trading 212 ISA",
            },
        )
        account_id = created.json()["id"]
        imported = client.post(
            f"/api/v1/accounts/{account_id}/imports",
            data={"confirmed_account_type": "stocks_and_shares_isa"},
            files={"file": ("export.csv", content, "text/csv")},
        )
        duplicate = client.post(
            f"/api/v1/accounts/{account_id}/imports",
            data={"confirmed_account_type": "stocks_and_shares_isa"},
            files={"file": ("export.csv", content, "text/csv")},
        )

    assert created.status_code == 201
    assert created.json()["base_currency"] == "GBP"
    assert imported.status_code == 201
    assert imported.json()["events_added"] == 1
    assert duplicate.status_code == 409
    engine.dispose()
    application_engine.dispose()
