# macOS release process

Worthweave produces a native `.app` and `.dmg` through Tauri. Local verification may use an ad-hoc signed artifact; public distribution must use an Apple Developer ID and notarisation.

## Release source of truth

- Worthweave follows [Semantic Versioning](https://semver.org/) and [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
- `CHANGELOG.md` is human-curated. Automation validates and publishes its release section; it never invents or commits release notes.
- `frontend/package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` must contain the same version.
- A public release is initiated by pushing an annotated `vMAJOR.MINOR.PATCH` tag whose version matches those files and a dated changelog section.

## Preparing a release

1. Move relevant entries from `Unreleased` into `## [MAJOR.MINOR.PATCH] - YYYY-MM-DD`, leaving an empty `Unreleased` section for future work.
2. Update the version in all three package files.
3. Add Keep a Changelog comparison links at the bottom of `CHANGELOG.md`.
4. Run `./scripts/check-release.sh vMAJOR.MINOR.PATCH` and the release gates below.
5. Commit the release preparation.
6. Create and push an annotated tag:

   ```bash
   git tag -a vMAJOR.MINOR.PATCH -m "Worthweave vMAJOR.MINOR.PATCH"
   git push origin master vMAJOR.MINOR.PATCH
   ```

The tag workflow validates the version and changelog, builds and notarises the application, publishes the same version of the `worthweave` Rust crate to crates.io, then creates the GitHub Release using the matching changelog section. It uploads the DMG and checksum together with the signed updater archive, signature, and `latest.json` manifest. If any gate fails, no GitHub Release is created. Rerunning a release safely skips crates.io publication when that exact crate version already exists.

## Release gates

1. Run Rust tests and strict Clippy.
2. Run frontend unit tests, lint, production build, and Playwright E2E accessibility tests.
3. Run `cargo audit` and `pnpm audit --prod`; investigate any vulnerability before release.
4. From the repository root, build with `frontend/node_modules/.bin/tauri build`. Local builds may be unsigned or ad-hoc signed; the release workflow supplies the configured Developer ID identity.
5. Verify the application bundle with `codesign --verify --deep --strict --verbose=2` and inspect it with `spctl --assess --type execute --verbose=2`.
6. Run `./scripts/check-release.sh` to validate package-version alignment and changelog structure.
7. Public releases are built by `.github/workflows/macos-release.yml`. The workflow imports the Developer ID certificate into an ephemeral keychain, validates notarisation access, runs the release gates, asks Tauri to sign and notarise, verifies the hardened-runtime signature and stapled tickets, publishes the Rust crate to crates.io, creates the GitHub Release, and publishes the DMG plus its SHA-256 checksum.
8. Confirm the release also contains `Worthweave.app.tar.gz`, its `.sig` file, and `latest.json`. An installed app verifies the archive against the public key embedded in `tauri.conf.json` before installation.

## GitHub configuration

Configure these repository secrets with the same values and encoding used by the Maabarium desktop release:

- `APPLE_CERTIFICATE`: base64-encoded Developer ID Application `.p12` certificate.
- `APPLE_CERTIFICATE_PASSWORD`: password protecting the `.p12` file.
- `APPLE_ID`: Apple developer account email used by `notarytool`.
- `APPLE_PASSWORD`: app-specific password for that Apple ID.
- `TAURI_SIGNING_PRIVATE_KEY`: dedicated private key used only to sign in-app updater archives. The current recovery copy is stored at `~/.tauri/worthweave-updater.key` with owner-only permissions and must be backed up securely. Losing it prevents existing installations from trusting future updates.
- `CARGO_REGISTRY_TOKEN`: crates.io API token permitted to publish new versions of the `worthweave` crate.

Configure these as repository variables where possible (secrets are also accepted for compatibility with the existing Maabarium setup):

- `APPLE_SIGNING_IDENTITY`: full Developer ID Application identity reported by `security find-identity`, including the team identifier.
- `APPLE_TEAM_ID`: Apple Developer team identifier.

Pushing a valid `v*` tag triggers the signed build and creates the GitHub Release only after successful verification. A manual workflow run can build the selected commit as a downloadable workflow artifact, or accept an existing `release_tag` and create or update that release after verification.

The configured updater endpoint uses GitHub’s anonymous release-download URL. It will not be reachable while the repository and its releases are private. Do not solve that by embedding a GitHub token in the application. Before distributing Worthweave, either make this repository public or move updater files to a separate public, HTTPS-only release endpoint and update `tauri.conf.json`.

The workflow does not modify `CHANGELOG.md`; changing source after a tag would make the tag non-reproducible. Pull-request CI validates version alignment and changelog structure, while reviewers ensure user-visible changes are recorded under `Unreleased`.

Never commit certificates, updater private keys, App Store Connect keys, passwords, broker exports, or notarisation credentials.

TypeScript is pinned to the newest release supported by the current TypeScript-ESLint peer range. Do not advance it across that compatibility boundary until the parser/plugin declares support.
