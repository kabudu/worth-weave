<p align="center">
  <img src="src-tauri/icons/icon.png" width="112" alt="Worthweave logo">
</p>

<h1 align="center">Worthweave</h1>

<p align="center">
  Private, local-first portfolio analysis across supported investment brokers.
</p>

<p align="center">
  <a href="https://github.com/kabudu/worth-weave/actions/workflows/macos-release.yml"><img alt="macOS release" src="https://github.com/kabudu/worth-weave/actions/workflows/macos-release.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <img alt="macOS 13 or later" src="https://img.shields.io/badge/macOS-13%2B-111111?logo=apple">
  <img alt="Rust 1.97" src="https://img.shields.io/badge/Rust-1.97-000000?logo=rust">
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri&logoColor=white">
  <img alt="Local-first" src="https://img.shields.io/badge/data-local--first-2F7D5B">
</p>

Worthweave brings investments held across multiple brokers and account types into one coherent macOS application. It reconstructs holdings from broker history, keeps Invest and Stocks and Shares ISA records separate, and reports portfolio value, performance, income, and allocation in a configurable reporting currency.

Planned reporting and analysis features are documented in the [product roadmap](ROADMAP.md).

Financial results come from deterministic Rust code using exact decimal representations. The optional local AI can explain those verified results, but it cannot create transactions, change the ledger, or substitute speculation for missing data.

## Highlights

- Account-aware Trading 212 and Interactive Brokers CSV imports.
- Region-aware Robinhood UK and US account tracking, with imports held behind validated export schemas.
- Separate Invest and Stocks and Shares ISA histories.
- Exact quantities, cost basis, average cost, value, and gain/loss calculations.
- Allocation by broker, account, asset class, sector, geography, and source currency.
- Transaction, dividend, interest, and valuation-snapshot history.
- True total-return attribution across realised and unrealised gains, dividends, interest, fees, and taxes, with explicit data-completeness diagnostics.
- Position comparison against the latest broker-reported holdings.
- Configurable reporting currency without rewriting source transactions.
- Human-readable JSON export plus encrypted, versioned backup and restore.
- Optional device-tuned local AI through Rapid-MLX or Ollama.
- Native Apple Silicon macOS application with no Python runtime or web server.

## First run

Onboarding keeps setup short and reversible:

1. Choose the reporting currency used for consolidated totals and performance.
2. Select the broker accounts to track so taxable and ISA histories remain separate.
3. Accept or skip the local AI recommendation generated for the Mac's hardware.
4. Import broker CSV exports; Worthweave checks file hashes and transaction identifiers to prevent duplicates.

Prices, exchange rates, and investment categories are configured after import, when Worthweave knows which holdings require them. All preferences can be revisited in Settings.

## Privacy and financial integrity

- Portfolio data is stored in a local SQLite database with owner-only filesystem permissions.
- Broker CSV files are parsed locally and broker credentials are not required.
- Imported source records are immutable.
- Missing history, prices, or exchange rates produce explicit partial or unavailable states rather than estimates.
- Local AI requests are restricted to loopback services and grounded in application-calculated analytics.
- Backups are encrypted with a user-supplied password that Worthweave never stores.

Worthweave is portfolio-analysis software, not financial advice. Local AI explanations must not be treated as price predictions or recommendations to trade.

## Technology

- [Tauri 2](https://tauri.app/) and Rust for the native application, broker adapters, calculations, and SQLite storage.
- React, TypeScript, TanStack Query, and Zod for the interface and native-command boundary.
- Tailwind CSS design tokens with purpose-built component styling.
- Locally bundled Manrope Variable and Inter Variable WOFF2 fonts; no remote font requests.
- Rapid-MLX or Ollama for optional local model inference.

The repository pins its Rust toolchain and JavaScript dependencies for reproducible builds.

## Development

Requirements:

- macOS 13 or later.
- Node.js 22 or later.
- pnpm 10.32.1.
- Rust 1.97.0, installed automatically from `rust-toolchain.toml` when using rustup.

Install dependencies and start the native development application:

```bash
pnpm --dir frontend install --frozen-lockfile
frontend/node_modules/.bin/tauri dev
```

Private broker exports belong in `.dev/`. That directory is excluded from source control.

## Validation

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
pnpm --dir frontend test
pnpm --dir frontend lint
pnpm --dir frontend build
pnpm --dir frontend test:e2e
```

The end-to-end suite exercises first-run onboarding, navigation, imports, settings, portfolio reporting, and automated accessibility checks.

## macOS builds and releases

Create local `.app` and `.dmg` bundles with:

```bash
frontend/node_modules/.bin/tauri build
```

Public GitHub releases use `.github/workflows/macos-release.yml` to import the Developer ID certificate into an ephemeral keychain, run the release gates, sign and notarise the application, verify Gatekeeper and stapled tickets, and publish the DMG with its SHA-256 checksum.

See the [release process](docs/release.md) for required GitHub secrets and variables.

## Documentation

- [Architecture and data model](docs/architecture.md)
- [Broker import contract](docs/data-imports.md)
- [Security model](docs/security.md)
- [Release process](docs/release.md)
- [v1 completion contract](docs/roadmap.md)

## License

Worthweave is licensed under the [Apache License 2.0](LICENSE). It is permissive and includes an explicit patent grant from contributors. See the license for its preservation, attribution, and change-notice requirements.
