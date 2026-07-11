# macOS release process

Worthweave produces a native `.app` and `.dmg` through Tauri. Local verification may use an ad-hoc signed artifact; public distribution must use an Apple Developer ID and notarisation.

## Release gates

1. Run Rust tests and strict Clippy.
2. Run frontend unit tests, lint, production build, and Playwright E2E accessibility tests.
3. Run `cargo audit` and `pnpm audit --prod`; investigate any vulnerability before release.
4. From the repository root, build with `frontend/node_modules/.bin/tauri build`. Local builds may be unsigned or ad-hoc signed; the release workflow supplies the configured Developer ID identity.
5. Verify the application bundle with `codesign --verify --deep --strict --verbose=2` and inspect it with `spctl --assess --type execute --verbose=2`.
6. Public releases are built by `.github/workflows/macos-release.yml`. The workflow imports the Developer ID certificate into an ephemeral keychain, validates notarisation access, runs the release gates, asks Tauri to sign and notarise, verifies the hardened-runtime signature and stapled tickets, and publishes the DMG plus its SHA-256 checksum.

## GitHub configuration

Configure these repository secrets with the same values and encoding used by the Maabarium desktop release:

- `APPLE_CERTIFICATE`: base64-encoded Developer ID Application `.p12` certificate.
- `APPLE_CERTIFICATE_PASSWORD`: password protecting the `.p12` file.
- `APPLE_ID`: Apple developer account email used by `notarytool`.
- `APPLE_PASSWORD`: app-specific password for that Apple ID.

Configure these as repository variables where possible (secrets are also accepted for compatibility with the existing Maabarium setup):

- `APPLE_SIGNING_IDENTITY`: full Developer ID Application identity reported by `security find-identity`, including the team identifier.
- `APPLE_TEAM_ID`: Apple Developer team identifier.

Publishing a GitHub release triggers the signed build for that release tag. A manual workflow run can either build the selected commit as a downloadable workflow artifact or accept an existing `release_tag` and attach the verified artifacts to that release.

Never commit certificates, App Store Connect keys, passwords, broker exports, or notarisation credentials.

TypeScript is pinned to the newest release supported by the current TypeScript-ESLint peer range. Do not advance it across that compatibility boundary until the parser/plugin declares support.
