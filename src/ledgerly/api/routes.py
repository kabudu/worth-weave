from typing import Annotated, Literal

from fastapi import APIRouter, Depends
from pydantic import BaseModel
from sqlalchemy import text
from sqlalchemy.orm import Session

from ledgerly.persistence.database import get_session

router = APIRouter()
DatabaseSession = Annotated[Session, Depends(get_session)]


class HealthResponse(BaseModel):
    status: Literal["ok"]
    database: Literal["ok"]
    mode: Literal["local"]


class PortfolioSummaryResponse(BaseModel):
    base_currency: Literal["GBP"]
    account_count: int
    import_count: int
    data_status: Literal["awaiting_imports", "partial", "current"]


@router.get("/health", response_model=HealthResponse, tags=["system"])
def health(session: DatabaseSession) -> HealthResponse:
    session.execute(text("SELECT 1"))
    return HealthResponse(status="ok", database="ok", mode="local")


@router.get(
    "/api/v1/portfolio/summary",
    response_model=PortfolioSummaryResponse,
    tags=["portfolio"],
)
def portfolio_summary(session: DatabaseSession) -> PortfolioSummaryResponse:
    account_count = int(session.scalar(text("SELECT COUNT(*) FROM accounts")) or 0)
    import_count = int(session.scalar(text("SELECT COUNT(*) FROM import_batches")) or 0)
    status: Literal["awaiting_imports", "partial", "current"] = (
        "awaiting_imports" if import_count == 0 else "partial"
    )
    return PortfolioSummaryResponse(
        base_currency="GBP",
        account_count=account_count,
        import_count=import_count,
        data_status=status,
    )
