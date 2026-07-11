#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

frontend_version="$(node -p "require('./frontend/package.json').version")"
tauri_version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
cargo_version="$(sed -n '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' src-tauri/Cargo.toml | head -1)"

if [[ -z "$cargo_version" || "$frontend_version" != "$cargo_version" || "$tauri_version" != "$cargo_version" ]]; then
  printf 'Version mismatch: frontend=%s tauri=%s cargo=%s\n' "$frontend_version" "$tauri_version" "$cargo_version" >&2
  exit 1
fi

grep -Fqx '## [Unreleased]' CHANGELOG.md || {
  echo 'CHANGELOG.md must contain an [Unreleased] section.' >&2
  exit 1
}

tag="${1:-}"
if [[ -z "$tag" ]]; then
  exit 0
fi

version="${tag#v}"
if [[ "$tag" != "v$version" || "$version" != "$cargo_version" ]]; then
  printf 'Release tag %s must equal v%s.\n' "$tag" "$cargo_version" >&2
  exit 1
fi

heading="$(grep -E "^## \[$version\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" CHANGELOG.md || true)"
if [[ -z "$heading" ]]; then
  printf 'CHANGELOG.md must contain a dated ## [%s] release section.\n' "$version" >&2
  exit 1
fi

notes="$(scripts/extract-release-notes.sh "$tag")"
if [[ -z "${notes//[[:space:]]/}" ]]; then
  printf 'CHANGELOG.md release section for %s is empty.\n' "$version" >&2
  exit 1
fi
