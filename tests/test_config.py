from pathlib import Path

import pytest
from pydantic import ValidationError

from ledgerly.config import Settings


def test_settings_default_to_loopback_and_local_database() -> None:
    settings = Settings()

    assert settings.bind_host == "127.0.0.1"
    assert settings.database_path == Path(".local/ledgerly.db")


def test_settings_reject_non_loopback_binding() -> None:
    with pytest.raises(ValidationError):
        Settings(bind_host="0.0.0.0")  # noqa: S104 - verifies rejection of a wildcard bind
