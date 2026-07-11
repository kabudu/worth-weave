# Security model

## Protected assets

- Brokerage exports and account identifiers
- Portfolio balances, activity, and derived analytics
- Future broker and market-data credentials
- Local LLM prompts containing portfolio context

## Current controls

- `.dev/`, SQLite files, caches, and build output are ignored by Git.
- The Tauri webview uses a narrow IPC allowlist and a restrictive content security policy; no local HTTP server is exposed.
- Imports use bounded file sizes, schema validation, content hashes, and atomic transactions.
- Broker adapters are read-only and do not include order-placement capability.
- Database errors do not expose raw records to the interface.
- Dependency lockfiles are committed and audited at milestones.
- Backups use the age file format with passphrase-based authenticated encryption. Passwords are passed directly to Rust, are never persisted, and must contain at least 12 characters.
- Restore is bounded to 1 GiB, validates SQLite integrity and the Worthweave schema, then uses SQLite's backup API to replace live state.

## Explicitly deferred

- Database-at-rest encryption
- macOS Keychain integration for broker tokens
- Hosted authentication, authorization, tenant isolation, and abuse controls
- Signed/notarized macOS application packaging

These are release requirements before distribution beyond a trusted local user.

## Verification gates

- Rust commands compile under strict `clippy -D warnings`; unit tests cover import boundaries, exact arithmetic, reconciliation, schema versioning, backup restore, and loopback-only AI.
- Production JavaScript dependencies and the Rust lockfile are audited before release. The current audit reports no known vulnerabilities. RustSec lifecycle warnings originate from Tauri's Linux GTK dependency graph and `age`'s macro dependency; these are not known vulnerabilities in the macOS artifact and remain tracked for upstream upgrades.
- The browser E2E suite runs the production frontend, completes both onboarding steps, and requires zero axe WCAG violations on the first-run screen.
- Tauri capabilities expose only core defaults and file open/save dialogs. The webview CSP blocks remote scripts and network connections; local AI networking occurs in Rust and accepts loopback endpoints only.
