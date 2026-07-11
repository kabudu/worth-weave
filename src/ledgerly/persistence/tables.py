from datetime import date, datetime
from uuid import UUID, uuid4

from sqlalchemy import Date, DateTime, ForeignKey, String, UniqueConstraint, func
from sqlalchemy.orm import Mapped, mapped_column, relationship

from ledgerly.persistence.database import Base


class AccountRow(Base):
    __tablename__ = "accounts"
    __table_args__ = (UniqueConstraint("broker", "external_id", name="uq_account_source"),)

    id: Mapped[UUID] = mapped_column(primary_key=True, default=uuid4)
    broker: Mapped[str] = mapped_column(String(32), nullable=False)
    account_type: Mapped[str] = mapped_column(String(64), nullable=False)
    external_id: Mapped[str] = mapped_column(String(128), nullable=False)
    display_name: Mapped[str] = mapped_column(String(160), nullable=False)
    base_currency: Mapped[str] = mapped_column(String(3), nullable=False, default="GBP")
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), server_default=func.now(), nullable=False
    )

    imports: Mapped[list["ImportBatchRow"]] = relationship(back_populates="account")


class ImportBatchRow(Base):
    __tablename__ = "import_batches"
    __table_args__ = (UniqueConstraint("account_id", "content_sha256", name="uq_import_content"),)

    id: Mapped[UUID] = mapped_column(primary_key=True, default=uuid4)
    account_id: Mapped[UUID] = mapped_column(ForeignKey("accounts.id"), nullable=False)
    original_filename: Mapped[str] = mapped_column(String(255), nullable=False)
    content_sha256: Mapped[str] = mapped_column(String(64), nullable=False)
    coverage_start: Mapped[date | None] = mapped_column(Date)
    coverage_end: Mapped[date | None] = mapped_column(Date)
    imported_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), server_default=func.now(), nullable=False
    )

    account: Mapped[AccountRow] = relationship(back_populates="imports")
