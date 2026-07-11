# Architecture

Worthweave is a single-user, local-first macOS application packaged as a Tauri app. Rust owns the ledger and all deterministic financial behavior; React communicates with it through a narrow Tauri command boundary.

```text
Broker CSV -> Rust adapter -> validated canonical events -> bundled SQLite
                                                        |
React UI <- typed Tauri IPC <- deterministic views <----+
                                |
                                +-> future local LLM explanations
```

## Invariants

1. Every event belongs to an explicit broker account and account type.
2. Content hashes and broker identifiers prevent duplicate and overlapping imports.
3. Parsing and validation complete before a transaction mutates ledger state.
4. Financial values use exact scaled integers: a signed coefficient plus a decimal scale.
5. Cash is normalized to the currency minor-unit scale when that is lossless; higher broker precision is retained rather than rounded.
6. Quantities, prices, FX rates, and cost basis retain their source precision. Binary floating point is never ledger truth.
7. Partial history is represented with coverage intervals and never reported as complete.
8. LLM output cannot create or alter holdings, returns, cost basis, or source records.
9. Reporting currency is a user preference stored independently of source amounts; changing it never mutates imported ledger events.

## Components

- `src-tauri/src/imports.rs`: bounded, account-aware Trading 212 and IBKR adapters.
- `src-tauri/src/db.rs`: bundled SQLite schema and persistence boundary.
- `src-tauri/src/lib.rs`: minimal typed commands exposed to the webview.
- `frontend`: React/TypeScript interface using Tailwind 4 design tokens.

## First-run settings

The application creates a singleton settings record during database initialization. Until a supported ISO reporting currency is selected, the interface remains in onboarding. The same backend-owned currency catalogue and validation path power the onboarding and Settings screens. A currency change invalidates reporting views while leaving broker-native currencies and exact values untouched.

## Valuation provenance

Market prices and FX rates are stored as exact coefficients and scales with their observation time and source. Manual entries are explicitly labelled `manual`. Direct and inverse FX pairs are supported. Consolidated portfolio value is returned only when every open holding has a price and every required reporting-currency conversion is available; missing inputs are counted and surfaced rather than treated as zero.

The frontend currently uses TypeScript 6.0 because the stable `typescript-eslint` parser does not yet declare TypeScript 7 support. This should be revisited when its supported range advances.
