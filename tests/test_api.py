from pathlib import Path

from fastapi.testclient import TestClient

from ledgerly.api.main import create_app
from ledgerly.config import get_settings


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
