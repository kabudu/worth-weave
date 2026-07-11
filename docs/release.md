# macOS release process

Worthweave produces a native `.app` and `.dmg` through Tauri. Local verification may use an ad-hoc signed artifact; public distribution must use an Apple Developer ID and notarisation.

## Release gates

1. Run Rust tests and strict Clippy.
2. Run frontend unit tests, lint, production build, and Playwright E2E accessibility tests.
3. Run `cargo audit` and `pnpm audit --prod`; investigate any vulnerability before release.
4. From the repository root, build with `frontend/node_modules/.bin/tauri build`. Local artifacts receive a complete ad-hoc bundle signature; public builds override it with the configured Developer ID identity.
5. Verify the application bundle with `codesign --verify --deep --strict --verbose=2` and inspect it with `spctl --assess --type execute --verbose=2`.
6. For public release, configure the Developer ID certificate and Apple notarisation credentials documented by Tauri, rebuild, submit the DMG for notarisation, staple the ticket, and repeat the verification checks.

Never commit certificates, App Store Connect keys, passwords, broker exports, or notarisation credentials.

TypeScript is pinned to the newest release supported by the current TypeScript-ESLint peer range. Do not advance it across that compatibility boundary until the parser/plugin declares support.
