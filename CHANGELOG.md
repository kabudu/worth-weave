# Changelog

All notable changes to Worthweave will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Local-first macOS portfolio application built with Tauri, Rust, React, and TypeScript.
- Account-aware Trading 212 and Interactive Brokers CSV imports.
- Region-aware Robinhood UK and US account tracking pending validated import fixtures.
- Deterministic holdings, cost basis, valuation, allocation, income, reconciliation, and true total-return attribution.
- Configurable reporting currency, encrypted backups, and human-readable exports.
- Optional device-aware local AI setup grounded in deterministic analytics.
- Signed and notarised macOS release automation.
- Open-source community health files, privacy-aware contribution templates, pull-request CI, Dependabot, and immutable GitHub Action pins.
- Keep a Changelog validation and tag-driven GitHub Release creation using human-curated release notes.
- Node 24-compatible Checkout v7 and Dependency Review v5 workflow actions.
- Signed in-app updates with an automatic availability check, visible download progress, verified installation, and app restart.

### Changed

- Reworked onboarding, imports, portfolio reports, settings, and private AI guidance to use clear, task-focused language instead of internal technical terms.

### Security

- Bounded, atomic broker imports with duplicate detection and immutable source events.
- Loopback-only local AI access, restrictive content security policy, and owner-only local storage.
- Update archives signed by a dedicated key and verified against a public key embedded in the application.

[Unreleased]: https://github.com/kabudu/worth-weave/commits/master
