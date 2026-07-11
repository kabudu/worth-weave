# Security policy

Worthweave handles sensitive financial records locally. Security reports are welcome and should be kept private until a fix is available.

## Supported versions

Before the first stable release, security fixes are applied to the latest commit on `master`. After releases begin, the latest released minor version will receive security fixes. Older versions may be asked to upgrade.

| Version | Supported |
| --- | --- |
| `master` / latest release | Yes |
| Older releases | Best effort |

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/kabudu/worth-weave/security/advisories/new). Do not open a public issue for a suspected vulnerability. The maintainer must enable private vulnerability reporting in the repository Security settings before making the repository public; if the private reporting form is unavailable, do not disclose vulnerability details publicly.

Include:

- The affected version or commit.
- Reproduction steps or a minimal proof of concept.
- Expected and observed impact.
- Relevant logs with credentials, portfolio values, account identifiers, broker exports, and other personal data removed.
- Any suggested remediation or disclosure constraints.

Never upload real broker statements, database backups, signing credentials, API tokens, or personally identifiable financial data. Use synthetic fixtures only.

You should receive an acknowledgement within seven days. Triage, remediation, and disclosure timing depend on severity and reproducibility. Please allow a reasonable remediation period before public disclosure.

## Scope

High-priority areas include import parsing, path and file handling, encrypted backup/restore, SQLite integrity, Tauri command authorization, content security policy bypasses, local-AI endpoint validation, release signing, and secret exposure.

The application's technical security model and release controls are documented in [docs/security.md](docs/security.md).
