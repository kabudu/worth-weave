# Ledgerly

Ledgerly is a local-first portfolio application for consolidating investments held across Trading 212 and Interactive Brokers. Financial calculations are deterministic; local language models may explain results but never establish ledger truth.

## Status

The repository contains the first application foundation: a typed Python API, SQLite persistence, broker-import boundaries, and a React interface. Broker exports used during development belong in `.dev/`, which is excluded from source control.

## Development

Prerequisites:

- Python 3.13 managed by `uv`
- Node.js 24 or newer
- pnpm 10 or newer

```bash
UV_CACHE_DIR=.cache/uv uv sync --all-groups
pnpm --dir frontend install --frozen-lockfile
UV_CACHE_DIR=.cache/uv uv run alembic upgrade head
UV_CACHE_DIR=.cache/uv uv run uvicorn ledgerly.api.main:app --reload
pnpm --dir frontend dev
```

The API listens on `127.0.0.1:8000` and the frontend on `127.0.0.1:5173` in development.

## Security and privacy

- Brokerage data and local databases are excluded from Git.
- The server binds to loopback by default.
- Imported files are hashed and processed transactionally.
- Money is represented with decimal values, never binary floating point.
- Secrets will be stored in macOS Keychain when API integrations are introduced.
- Ollama integration is local-only and receives deterministic analytics rather than raw authority over calculations.

See [docs/architecture.md](docs/architecture.md) and [docs/security.md](docs/security.md).
