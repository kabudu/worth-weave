# Worthweave v1 completion contract

Worthweave v1 is complete when a macOS user can onboard, import supported broker history, reconcile positions, value the portfolio in a chosen reporting currency, explore performance and allocation, back up and restore local data, and optionally ask a local LLM to explain deterministic analytics.

## Milestones

- [x] Native Tauri/Rust shell, local SQLite storage, React interface, and macOS packaging.
- [x] Account-aware Trading 212 and IBKR imports with exact scaled-integer values.
- [x] First-run reporting-currency onboarding and editable local settings.
- [x] Device-aware, explicitly approved local-AI runtime and model onboarding.
- [x] Deterministic open-quantity, average-cost, activity, and income projections.
- [x] Position reconciliation against broker snapshots and explicit partial-history diagnostics.
- [x] Market prices and FX rates with timestamps, sources, staleness, and manual overrides.
- [x] Reporting-currency value, gain/loss, allocation, snapshots, and historical performance.
- [x] True total-return attribution with realised/unrealised gains, income, costs, coverage, and explicit FX limitations.
- [x] Human-readable portfolio export plus encrypted, versioned backup and restore.
- [x] Optional local-runtime explanations grounded only in deterministic analytics.
- [x] Versioned migrations, accessibility and end-to-end coverage, signing/notarisation readiness.

## Non-negotiable reporting rules

- Imported source values are immutable.
- Missing prices, FX rates, or history produce an explicit unavailable or partial state—not an estimate.
- Changing reporting currency recomputes views and never rewrites events.
- LLM output is explanatory and cannot become ledger truth.
