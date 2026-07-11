# Open-source readiness

The repository contains the project-level material needed for a public launch:

- Apache-2.0 license and matching package metadata.
- README, architecture, security model, support, governance, roadmap, and release documentation.
- Contributing guide, contributor record, code of conduct, issue forms, pull-request template, and CODEOWNERS.
- Keep a Changelog history and Semantic Versioning release contract.
- Pull-request CI, dependency auditing, dependency review for public pull requests, and Dependabot configuration.
- Tag-driven signed and notarised macOS releases with changelog-derived GitHub Release notes.
- Privacy warnings that prohibit real broker data in public issues and fixtures.

## GitHub settings required at public launch

Repository files cannot enforce all hosting settings. Immediately before or after changing visibility to public, a maintainer should:

1. Enable **Private vulnerability reporting** under Security → Code security and analysis.
2. Enable the dependency graph, Dependabot alerts, and Dependabot security updates.
3. Create a `master` branch ruleset requiring the `validate` CI check, resolved review conversations, and at least one approving review for non-maintainer contributions.
4. Prevent force pushes and branch deletion on `master`.
5. Limit Actions workflow permissions to read access by default, allowing write access only where a workflow declares it (the release job).
6. Configure the Apple signing and notarisation secrets documented in [release.md](release.md).
7. Run the first public CI workflow before accepting contributions.

The repository is currently private. GitHub's free branch-protection and public dependency-review capabilities may be unavailable until visibility changes; the CI workflow skips dependency review while private and enables it automatically when public.

## Release ownership

Only a maintainer should update package versions, promote `Unreleased` changelog entries, and push release tags. The release workflow creates a GitHub Release only after all validation, signing, notarisation, and artifact verification succeeds.
