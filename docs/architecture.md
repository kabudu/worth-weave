# Architecture

## Product boundaries

Ledgerly begins as a single-user, local-first macOS application. Its domain and API boundaries deliberately avoid assumptions that would prevent a future hosted, multi-user deployment.

```text
Broker export/API -> adapter -> validated canonical events -> SQLite ledger
                                                           |
React UI <- typed local API <- deterministic projections <-+
                                      |
                                      +-> Ollama explanations
```

## Invariants

1. An event always belongs to an explicit broker account.
2. A source file cannot be committed twice to the same account.
3. Import validation completes before a database transaction mutates ledger state.
4. Monetary quantities use `Decimal` and retain their original currency.
5. GBP conversion retains the applied rate, rate date, and source.
6. Partial history is represented with coverage intervals and never reported as complete.
7. LLM output cannot create or alter holdings, returns, cost basis, or source records.

## Components

- `ledgerly.domain`: canonical financial types and invariants.
- `ledgerly.importers`: isolated platform-specific schema adapters.
- `ledgerly.persistence`: SQLite models, sessions, and migrations.
- `ledgerly.services`: application use cases and transaction boundaries.
- `ledgerly.api`: loopback HTTP interface consumed by the frontend.
- `frontend`: accessible React/TypeScript application.

## Growth path

A hosted product would add authenticated users, tenant ownership on every aggregate, encrypted managed storage, background jobs, rate-limited integrations, and an external secrets manager. Those concerns remain outside the local MVP but the account and service boundaries are compatible with them.
