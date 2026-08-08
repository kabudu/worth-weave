# Changelog

All notable changes to Worthweave will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.2] - 2026-08-08

Worthweave 0.3.2 introduces a public project website and makes privacy, setup, market data, returns, backups, and private AI easier to understand without hiding the broker terms users still need.

### Added

- Publish a responsive project website with privacy, feature, product preview, and macOS download information.

### Changed

- Replace implementation-led wording across the website, settings, market data, returns, backups, and private AI setup with clearer everyday language.
- Keep broker-issued credential names available where users must copy them, while adding plain-English setup guidance.

## [0.3.1] - 2026-08-04

Worthweave 0.3.1 makes portfolio results easier to scan and keeps dependency auditing strict without treating an unused optional feature as part of the shipped application.

### Changed

- Show positive portfolio gains in green and losses in red so holding performance is easier to scan.

### Security

- Keep Rust dependency auditing strict while guarding a temporary exception for an unused optional archival dependency.

## [0.3.0] - 2026-07-26

### Added

- Read-only IBKR Flex Web Service connections for single-account CSV Activity Flex Queries, with tokens protected by macOS Keychain.
- Bounded asynchronous IBKR report polling, provider-specific error handling, account-identity validation, and reuse of the existing atomic, idempotent import pipeline.

### Fixed

- Correctly validate mixed IBKR Flex sections without mistaking primary-currency values for additional broker accounts.
- Give every application modal a wider responsive desktop layout and prevent broker synchronisation action labels from wrapping.
- Standardise primary and secondary modal actions at the same 48px control height.

## [0.2.0] - 2026-07-19

### Added

- Read-only Trading 212 API connections for Invest and Stocks and Shares ISA accounts, with account-specific credentials protected by macOS Keychain.
- Resilient daily broker synchronisation on launch, including official history exports, current position snapshots, idempotent imports, visible sync state, and CSV fallback.

### Fixed

- Preserve pending history reports and observe Trading 212 retry windows when the API rate-limits synchronisation requests.
- Accept the `Time (UTC)` column used by Trading 212's live API-generated history exports.

## [0.1.1] - 2026-07-15

### Fixed

- Include the project README in the crates.io package so the crate page displays its documentation.
- Restore production dependency auditing after npm retired the legacy audit API used by pnpm 10.

### Changed

- Upgrade the pinned package manager to pnpm 11.13.0, which uses npm's supported bulk-advisory API.

## [0.1.0] - 2026-07-14

### Added

- Local-first macOS portfolio application built with Tauri, Rust, React, and TypeScript.
- Account-aware Trading 212 and Interactive Brokers CSV imports.
- Region-aware Robinhood UK and US account tracking pending validated import fixtures.
- Deterministic holdings, cost basis, valuation, allocation, income, reconciliation, and true total-return attribution.
- Configurable reporting currency, encrypted backups, and human-readable exports.
- Optional device-aware local AI setup grounded in deterministic analytics.
- Signed and notarised macOS release automation.
- Idempotent crates.io publication for the Rust crate during tagged releases.
- Open-source community health files, privacy-aware contribution templates, pull-request CI, Dependabot, and immutable GitHub Action pins.
- Keep a Changelog validation and tag-driven GitHub Release creation using human-curated release notes.
- Node 24-compatible Checkout v7 and Dependency Review v5 workflow actions.
- Signed in-app updates with an automatic availability check, visible download progress, verified installation, and app restart.

### Changed

- Reworked onboarding, imports, portfolio reports, settings, and private AI guidance to use clear, task-focused language instead of internal technical terms.
- Use latest IBKR position snapshots for current quantities, repair repeated imports without duplicating events, link symbol-only IBKR trades, and distinguish incomplete history from current-position errors.
- Apply exact Trading 212 stock-split rows, refresh official ECB reference exchange rates automatically, count missing currency pairs accurately, and show explicitly partial portfolio values without permitting incomplete snapshots.

### Security

- Bounded, atomic broker imports with duplicate detection and immutable source events.
- Loopback-only local AI access, restrictive content security policy, and owner-only local storage.
- Update archives signed by a dedicated key and verified against a public key embedded in the application.

[Unreleased]: https://github.com/kabudu/worth-weave/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/kabudu/worth-weave/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/kabudu/worth-weave/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/kabudu/worth-weave/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/kabudu/worth-weave/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/kabudu/worth-weave/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/kabudu/worth-weave/releases/tag/v0.1.0
