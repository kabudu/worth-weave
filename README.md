# Worthweave

Worthweave is a local-first macOS portfolio application that weaves Trading 212 and Interactive Brokers accounts into one coherent view. Financial calculations are deterministic; local language models may explain results but never establish ledger truth.

## Technology

- Tauri 2 and Rust 1.97 provide the native application, broker adapters, and SQLite ledger.
- React 19 and TypeScript provide the interface.
- Tailwind CSS 4 supplies design tokens and composable styling primitives alongside bespoke component CSS.
- SQLite is bundled into the application; no Python runtime or local web server is required.

## Development

Prerequisites are Rust 1.97, Node.js 24 or newer, and pnpm 10 or newer. The repository toolchain file installs the required Rust formatter and linter.

```bash
pnpm --dir frontend install --frozen-lockfile
frontend/node_modules/.bin/tauri dev
```

Validation and packaging:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
pnpm --dir frontend lint
pnpm --dir frontend test
pnpm --dir frontend build
frontend/node_modules/.bin/tauri build
```

Private broker exports belong in `.dev/`, which is excluded from source control. See [the architecture](docs/architecture.md), [import contract](docs/data-imports.md), and [security model](docs/security.md).
