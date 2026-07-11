from dataclasses import dataclass

from ledgerly.domain.models import CanonicalEvent, CoveragePeriod


class ImportFormatError(ValueError):
    """Raised when an export cannot be safely interpreted."""


@dataclass(frozen=True, slots=True)
class ParsedImport:
    coverage: CoveragePeriod
    events: tuple[CanonicalEvent, ...]
    warnings: tuple[str, ...] = ()
