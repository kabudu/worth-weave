# Security model

## Protected assets

- Brokerage exports and account identifiers
- Portfolio balances, activity, and derived analytics
- Broker and market-data credentials
- Local LLM prompts containing portfolio context

## Current controls

- `.dev/`, SQLite files, caches, and build output are ignored by Git.
- The Tauri webview uses a narrow IPC allowlist and a restrictive content security policy; no local HTTP server is exposed.
- Imports use bounded file sizes, schema validation, content hashes, and atomic transactions.
- Broker adapters are read-only and do not include order-placement capability.
- Account-specific Trading 212 API credentials and market-data tokens are stored in macOS Keychain rather than SQLite.
- Public macOS releases are signed, notarised and verified by the automated release workflow.
- Database errors do not expose raw records to the interface.
- Dependency lockfiles are committed and audited at milestones.
- Backups use the age file format with passphrase-based authenticated encryption. Passwords are passed directly to Rust, are never persisted, and must contain at least 12 characters.
- Restore is bounded to 1 GiB, validates SQLite integrity and the Worthweave schema, then uses SQLite's backup API to replace live state.

## Explicitly deferred

- Database-at-rest encryption
- Hosted authentication, authorization, tenant isolation, and abuse controls

These are release requirements before distribution beyond a trusted local user.

## Verification gates

- Rust commands compile under strict `clippy -D warnings`; unit tests cover import boundaries, exact arithmetic, reconciliation, schema versioning, backup restore, and loopback-only AI.
- Production JavaScript dependencies and the Rust lockfile are audited before release. The current audit reports no known vulnerabilities. RustSec lifecycle warnings originate from Tauri's Linux GTK dependency graph and `age`'s macro dependency; these are not known vulnerabilities in the macOS artifact and remain tracked for upstream upgrades.
- The browser E2E suite runs the production frontend, completes both onboarding steps, and requires zero axe WCAG violations on the first-run screen.
- Tauri capabilities expose only core defaults and file open/save dialogs. The webview CSP blocks remote scripts and network connections. Native networking is limited by feature code: local AI accepts loopback endpoints only, while exchange-rate refreshes use the fixed official ECB HTTPS endpoint with a 15-second timeout and a 256 KiB response limit.

## 2026-07 security and resilience sweep

- Local-AI endpoints are parsed structurally and reject remote hosts disguised with a loopback URL prefix or user-information authority. Responses are streamed with a hard 1 MiB ceiling even when the runtime omits `Content-Length`; broker text is explicitly treated as untrusted prompt data.
- Broker imports use bounded reads rather than trusting file metadata, enforce a 50 MiB/500,000-row ceiling, reject oversized identifiers, and preserve existing broker snapshots rather than rewriting imported source values.
- Trading 212 connections use only the documented HTTPS API hosts, validate credentials before storing them, request read-only account/portfolio/history scopes, bound downloaded reports to 50 MiB, and never log or persist credentials in SQLite. Broker-generated CSVs pass through the same validation and idempotency boundary as manual imports.
- Encrypted backup, restore, and JSON export stream through owner-only temporary files instead of retaining whole databases in memory. Restore validates integrity, schema compatibility, and foreign-key relationships before replacement; destructive restore requires explicit UI acknowledgement.
- The application data directory is forced to mode `0700` and the SQLite file to `0600` on Unix/macOS. The CSP separates inline style attributes from style resources and denies objects, frames, workers, base-URL changes, and form navigation.
- Backup/export commands accept only their documented file extensions, reducing arbitrary overwrite scope if the webview is ever compromised.
