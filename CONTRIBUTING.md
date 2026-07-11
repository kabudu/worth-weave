# Contributing to Worthweave

Thank you for helping improve Worthweave. Contributions should preserve its local-first privacy model and deterministic financial-calculation boundary.

## Before starting

- Search existing issues and the [roadmap](ROADMAP.md).
- Open an issue before a large architectural, schema, broker-import, tax, AI, or release change.
- Never share real broker exports or portfolio data. Create the smallest synthetic fixture that reproduces the format or bug.
- Security vulnerabilities must follow [SECURITY.md](SECURITY.md), not the public issue tracker.

## Development setup

Requirements and startup instructions are in the [README](README.md). The pinned Rust and JavaScript versions are authoritative.

```bash
pnpm --dir frontend install --frozen-lockfile
frontend/node_modules/.bin/tauri dev
```

Private local inputs belong in `.dev/`, which is ignored by Git.

## Engineering expectations

- Financial calculations must be deterministic Rust code using exact decimal values.
- The LLM may explain verified analytics but must not become ledger truth or invent missing figures.
- Imported source events are immutable.
- Missing history, classifications, prices, or FX rates must remain explicit.
- Broker adapters must reject unknown schemas rather than guessing.
- Database changes require a forward migration and compatibility test.
- User-facing terminology should be understandable without sacrificing financial accuracy.
- Keep work bounded for the documented 50 MiB and 500,000-row import limits.

## Validation

Run the relevant focused checks while developing and the complete suite before requesting review:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo audit --file src-tauri/Cargo.lock
pnpm --dir frontend test
pnpm --dir frontend lint
pnpm --dir frontend build
pnpm --dir frontend audit --prod
pnpm --dir frontend test:e2e
./scripts/check-release.sh
```

Changes to user-visible behavior, data contracts, security posture, or compatibility should update the relevant documentation and the `Unreleased` section of [CHANGELOG.md](CHANGELOG.md).

## Pull requests

- Keep each pull request focused and explain the user impact.
- Describe correctness, privacy, security, performance, migration, and rollback considerations where relevant.
- Add tests for success, partial-data, and failure paths.
- Confirm that generated output, broker data, secrets, databases, and local caches are absent.
- Link the issue being addressed.

By submitting a contribution, you agree that it is licensed under the repository's [Apache License 2.0](LICENSE). A Contributor License Agreement is not currently required.
