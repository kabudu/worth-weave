from pathlib import Path
from typing import Annotated, Literal
from uuid import UUID

from fastapi import APIRouter, Depends, File, Form, HTTPException, UploadFile, status
from pydantic import BaseModel, Field
from sqlalchemy import select, text
from sqlalchemy.orm import Session

from ledgerly.config import get_settings
from ledgerly.domain.models import AccountType, Broker
from ledgerly.importers import ImportFormatError
from ledgerly.persistence.database import get_session
from ledgerly.persistence.tables import AccountRow
from ledgerly.services.imports import (
    AccountNotFoundError,
    DuplicateImportError,
    create_account,
    import_csv,
)

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


class AccountCreateRequest(BaseModel):
    broker: Broker
    account_type: AccountType
    external_id: str = Field(min_length=1, max_length=128)
    display_name: str = Field(min_length=1, max_length=160)


class AccountResponse(BaseModel):
    id: UUID
    broker: Broker
    account_type: AccountType
    display_name: str
    base_currency: Literal["GBP"]


class ImportResponse(BaseModel):
    batch_id: UUID
    coverage_start: str
    coverage_end: str
    events_added: int
    warnings: tuple[str, ...]


@router.get("/api/v1/accounts", response_model=list[AccountResponse], tags=["accounts"])
def list_accounts(session: DatabaseSession) -> list[AccountResponse]:
    accounts = session.scalars(select(AccountRow).order_by(AccountRow.created_at)).all()
    return [
        AccountResponse(
            id=account.id,
            broker=Broker(account.broker),
            account_type=AccountType(account.account_type),
            display_name=account.display_name,
            base_currency="GBP",
        )
        for account in accounts
    ]


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


@router.post(
    "/api/v1/accounts",
    response_model=AccountResponse,
    status_code=status.HTTP_201_CREATED,
    tags=["accounts"],
)
def add_account(request: AccountCreateRequest, session: DatabaseSession) -> AccountResponse:
    account = create_account(
        session,
        broker=request.broker,
        account_type=request.account_type,
        external_id=request.external_id,
        display_name=request.display_name,
    )
    return AccountResponse(
        id=account.id,
        broker=Broker(account.broker),
        account_type=AccountType(account.account_type),
        display_name=account.display_name,
        base_currency="GBP",
    )


@router.post(
    "/api/v1/accounts/{account_id}/imports",
    response_model=ImportResponse,
    status_code=status.HTTP_201_CREATED,
    tags=["imports"],
)
async def upload_import(
    account_id: UUID,
    session: DatabaseSession,
    file: Annotated[UploadFile, File(description="Broker CSV export")],
    confirmed_account_type: Annotated[AccountType, Form()],
) -> ImportResponse:
    settings = get_settings()
    content = await file.read(settings.max_import_bytes + 1)
    if len(content) > settings.max_import_bytes:
        raise HTTPException(status.HTTP_413_CONTENT_TOO_LARGE, "import exceeds the size limit")
    filename = Path(file.filename or "broker-export.csv").name
    if not filename.casefold().endswith(".csv"):
        raise HTTPException(status.HTTP_415_UNSUPPORTED_MEDIA_TYPE, "only CSV imports are accepted")
    try:
        destination = session.get(AccountRow, account_id)
        if destination is not None and destination.account_type != confirmed_account_type.value:
            raise HTTPException(
                status.HTTP_409_CONFLICT,
                "selected account type does not match the destination account",
            )
        result = import_csv(
            session,
            account_id=account_id,
            filename=filename,
            content=content,
        )
    except AccountNotFoundError as exc:
        raise HTTPException(status.HTTP_404_NOT_FOUND, str(exc)) from exc
    except DuplicateImportError as exc:
        raise HTTPException(status.HTTP_409_CONFLICT, str(exc)) from exc
    except ImportFormatError as exc:
        raise HTTPException(status.HTTP_422_UNPROCESSABLE_CONTENT, str(exc)) from exc
    return ImportResponse(
        batch_id=result.batch_id,
        coverage_start=result.coverage_start,
        coverage_end=result.coverage_end,
        events_added=result.events_added,
        warnings=result.warnings,
    )
