# Security model

## Protected assets

- Brokerage exports and account identifiers
- Portfolio balances, activity, and derived analytics
- Future broker and market-data credentials
- Local LLM prompts containing portfolio context

## Current controls

- `.dev/`, SQLite files, caches, and build output are ignored by Git.
- API configuration rejects non-loopback binding unless explicitly enabled in a future deployment mode.
- Imports use bounded file sizes, schema validation, content hashes, and atomic transactions.
- Broker adapters are read-only and do not include order-placement capability.
- Database errors do not expose raw records in API responses.
- Dependency lockfiles are committed and audited at milestones.

## Explicitly deferred

- Encrypted backups and database-at-rest encryption
- macOS Keychain integration for broker tokens
- Hosted authentication, authorization, tenant isolation, and abuse controls
- Signed/notarized macOS application packaging

These are release requirements before distribution beyond a trusted local user.
