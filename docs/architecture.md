# Architecture

Worthweave is a single-user, local-first macOS application packaged as a Tauri app. Rust owns the ledger and all deterministic financial behavior; React communicates with it through a narrow Tauri command boundary.

```text
Broker CSV -> Rust adapter -> validated canonical events -> bundled SQLite
                                                        |
React UI <- typed Tauri IPC <- deterministic views <----+
                                |
                                +-> optional local LLM explanations
```

## Invariants

1. Every event belongs to an explicit broker account, jurisdiction, and legal account type.
2. Content hashes and broker identifiers prevent duplicate and overlapping imports.
3. Parsing and validation complete before a transaction mutates ledger state.
4. Financial values use exact scaled integers: a signed coefficient plus a decimal scale.
5. Cash is normalized to the currency minor-unit scale when that is lossless; higher broker precision is retained rather than rounded.
6. Quantities, prices, FX rates, and cost basis retain their source precision. Binary floating point is never ledger truth.
7. Partial history is represented with coverage intervals and never reported as complete.
8. LLM output cannot create or alter holdings, returns, cost basis, or source records.
9. Reporting currency is a user preference stored independently of source amounts; changing it never mutates imported ledger events.

## Components

- `src-tauri/src/imports.rs`: bounded, account-aware Trading 212 and IBKR adapters plus explicit rejection for unvalidated broker schemas.
- `src-tauri/src/db.rs`: bundled SQLite schema and persistence boundary.
- `src-tauri/src/lib.rs`: minimal typed commands exposed to the webview.
- `frontend`: React/TypeScript interface using Tailwind 4 design tokens.

## First-run settings

The application creates a singleton settings record during database initialization. Until a supported ISO reporting currency is selected, the interface remains in onboarding. Account setup records the broker, ISO jurisdiction (`GB` or `US`), and legal account type; Robinhood account types are validated against their region. The same backend-owned currency catalogue and validation path power the onboarding and Settings screens. A currency change invalidates reporting views while leaving broker-native currencies and exact values untouched.

The second onboarding step inspects only coarse local hardware characteristics needed for model sizing. Apple Silicon devices receive a pinned Rapid-MLX recommendation derived from its published unified-memory tiers; other devices receive an Ollama fallback. The user must explicitly approve runtime/model setup, which may download several gigabytes, or can continue without AI. The selected runtime, model and loopback endpoint are stored locally. Portfolio calculations remain deterministic application code; models may only explain application-produced analytics.

IBKR open-position sections are persisted as immutable, account-scoped broker snapshots. Reconciliation compares the latest snapshot per account with quantities reconstructed from canonical events and reports matched, mismatched, or unavailable for every instrument. Trading 212 transaction exports without positions remain explicitly unavailable. Broker symbols and descriptions populate a local instrument registry keyed by the stable ISIN or broker contract identifier.

Manual market prices and FX rates retain source and RFC 3339 timestamps. The native backend fetches the ECB's bounded HTTPS daily reference-rate XML feed, derives supported cross-rates through EUR using exact decimals, and stores the publication date and `ecb_reference` source. Manual rates take precedence over automatic refreshes. Prices older than 36 hours and FX rates older than 48 hours are surfaced as stale without silently discarding the last known deterministic valuation. Gain/loss is computed only when both cost basis and required currency conversion are complete; otherwise it remains unavailable.

True total-return attribution replays imported trades using average-cost performance lots and separates realised gains, unrealised gains, dividends, interest, fees, and taxes. Deposits and withdrawals are external cash flows and never appear as investment return. Coverage dates and calculation notes are always returned. Missing history, prices, exchange rates, corporate-action basis adjustments, in-kind transfers, or unclassified cash events keep the report partial. Foreign components may be translated at the latest available rate for an attributed subtotal, but consolidated total return and FX impact remain unavailable until transaction-date FX rates exist.

Local explanations use the OpenAI-compatible loopback endpoint of the configured runtime. Worthweave serializes only its deterministic valuation, allocation, reconciliation, income, and snapshot outputs. Requests reject non-loopback endpoints, limit question length and response size, use low temperature, and instruct the model not to calculate, predict, invent missing values, or provide personalised financial advice. Model text is never persisted into the ledger.

Configured AI runtimes start on demand when a question is asked rather than at application startup. Worthweave probes the local `/models` endpoint with one-second health-request bounds, starts the pinned runtime if required, and allows at most 20 seconds for readiness before returning a clear error. This avoids persistent model memory/CPU use when AI is not in use.

Allocation is calculated from the same complete reporting-currency valuation by platform, account, asset class, sector, geography, and source currency. IBKR asset classes are imported where supplied; missing classifications remain visibly `Unclassified` until the user adds local metadata. Metadata edits affect grouping only and never alter broker events or financial values.

Schema version 5 adds projection, activity, import, and latest-position indexes. Schema version 6 adds account jurisdiction, migrating existing accounts to `GB` for compatibility and assigning US Robinhood accounts a USD base currency. Import de-duplication relies on SQLite uniqueness inside the transaction instead of loading every historical source identifier into memory. The frontend defers account and reporting queries until onboarding is complete and the relevant view is opened. Backup/export paths are streaming, so memory use no longer scales with the entire SQLite database size.

Manrope Variable and Inter Variable are bundled as local WOFF2 assets. Manrope provides the display and brand voice; Inter is the application and financial-data face. The webview does not contact Google Fonts or another font CDN.

## Valuation provenance

Market prices and FX rates are stored as exact coefficients and scales with their observation time and source. Manual entries are explicitly labelled `manual`; automatic reference rates are labelled `ecb_reference`. Direct and inverse FX pairs are supported. The UI may show an explicitly labelled “Valued so far” subtotal for holdings with complete market inputs, while a portfolio snapshot and complete total remain unavailable until every open holding has a price and every required reporting-currency conversion is available. Missing inputs are counted and never treated as zero.

The frontend currently uses TypeScript 6.0 because the stable `typescript-eslint` parser does not yet declare TypeScript 7 support. This should be revisited when its supported range advances.

## Portability

JSON exports are versioned, human-readable portfolio reports. Complete backups use age passphrase encryption and contain a consistent SQLite snapshot. Restore is size-bounded, authenticated, integrity-checked, schema-checked, and copied into the live database only after validation succeeds.
