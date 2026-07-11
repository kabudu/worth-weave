from collections.abc import Generator

from sqlalchemy import create_engine, event
from sqlalchemy.engine import Engine
from sqlalchemy.orm import DeclarativeBase, Session, sessionmaker

from ledgerly.config import get_settings


class Base(DeclarativeBase):
    pass


def _build_engine() -> Engine:
    settings = get_settings()
    settings.database_path.parent.mkdir(parents=True, exist_ok=True)
    engine = create_engine(
        settings.database_url,
        connect_args={"check_same_thread": False},
        pool_pre_ping=True,
    )

    @event.listens_for(engine, "connect")
    def configure_sqlite(dbapi_connection: object, _: object) -> None:
        cursor = dbapi_connection.cursor()  # type: ignore[attr-defined]
        cursor.execute("PRAGMA foreign_keys=ON")
        cursor.execute("PRAGMA journal_mode=WAL")
        cursor.execute("PRAGMA busy_timeout=5000")
        cursor.close()

    return engine


engine = _build_engine()
SessionLocal = sessionmaker(bind=engine, autoflush=False, expire_on_commit=False)


def create_schema() -> None:
    from ledgerly.persistence import tables  # noqa: F401

    Base.metadata.create_all(bind=engine)


def get_session() -> Generator[Session]:
    with SessionLocal() as session:
        yield session
