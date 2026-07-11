from functools import lru_cache
from pathlib import Path
from typing import Literal

from pydantic import field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="LEDGERLY_", extra="ignore")

    environment: Literal["development", "test", "production"] = "development"
    database_path: Path = Path(".local/ledgerly.db")
    bind_host: str = "127.0.0.1"
    max_import_bytes: int = 50 * 1024 * 1024

    @field_validator("bind_host")
    @classmethod
    def require_loopback(cls, value: str) -> str:
        if value not in {"127.0.0.1", "::1", "localhost"}:
            raise ValueError("local mode requires a loopback bind host")
        return value

    @field_validator("max_import_bytes")
    @classmethod
    def bound_import_size(cls, value: int) -> int:
        if value <= 0 or value > 250 * 1024 * 1024:
            raise ValueError("max import size must be between 1 byte and 250 MiB")
        return value

    @property
    def database_url(self) -> str:
        return f"sqlite:///{self.database_path}"


@lru_cache
def get_settings() -> Settings:
    return Settings()
