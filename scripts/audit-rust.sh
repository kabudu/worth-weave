#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repo_root/src-tauri/Cargo.toml"
lockfile="$repo_root/src-tauri/Cargo.lock"

# rust_decimal 1.42.1 declares rkyv 0.7.46 as an optional dependency, but
# Worthweave does not enable that feature. RUSTSEC-2026-0235 therefore does not
# affect the compiled application. Fail closed if rkyv ever enters the normal
# dependency graph, and remove this exception once rust_decimal upgrades it.
if cargo tree --manifest-path "$manifest" --locked -e normal -i rkyv 2>/dev/null | grep -q '^rkyv '; then
  echo "error: rkyv is active in Worthweave's normal dependency graph" >&2
  echo "Remove the RUSTSEC-2026-0235 exception and resolve the dependency." >&2
  exit 1
fi

cargo audit --file "$lockfile" --ignore RUSTSEC-2026-0235
