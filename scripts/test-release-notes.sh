#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

fixture="$(mktemp)"
actual="$(mktemp)"
expected="$(mktemp)"
trap 'rm -f "$fixture" "$actual" "$expected"' EXIT

cat > "$fixture" <<'EOF'
# Changelog

## [Unreleased]

## [1.2.3] - 2026-07-27

### Added

- A useful feature.

### Fixed

- A formatting problem.

## [1.2.2] - 2026-07-20

### Fixed

- An earlier fix.
EOF

cat > "$expected" <<'EOF'
## Added

- A useful feature.

## Fixed

- A formatting problem.
EOF

CHANGELOG_FILE="$fixture" scripts/extract-release-notes.sh v1.2.3 > "$actual"
diff -u "$expected" "$actual"

if [[ "$(head -c 1 "$actual")" == $'\n' ]]; then
  echo "release notes must not start with a blank line" >&2
  exit 1
fi

if [[ "$(tail -c 1 "$actual" | od -An -t x1 | tr -d '[:space:]')" != "0a" ]]; then
  echo "release notes must end with exactly one newline" >&2
  exit 1
fi
