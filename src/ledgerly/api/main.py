from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

from fastapi import FastAPI

from ledgerly.api.routes import router
from ledgerly.config import get_settings
from ledgerly.persistence.database import create_schema


@asynccontextmanager
async def lifespan(_: FastAPI) -> AsyncIterator[None]:
    create_schema()
    yield


def create_app() -> FastAPI:
    settings = get_settings()
    app = FastAPI(
        title="Ledgerly API",
        description="Local deterministic portfolio ledger API",
        version="0.1.0",
        docs_url="/docs" if settings.environment == "development" else None,
        redoc_url=None,
        lifespan=lifespan,
    )
    app.include_router(router)
    return app


app = create_app()
