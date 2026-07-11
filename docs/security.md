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
