import gc

import pytest

from ledgerly.persistence.database import engine


@pytest.fixture(scope="session", autouse=True)
def close_application_resources() -> None:
    yield
    engine.dispose()
    gc.collect()
