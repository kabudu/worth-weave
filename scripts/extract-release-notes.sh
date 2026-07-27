#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 vMAJOR.MINOR.PATCH" >&2
  exit 2
fi

version="${1#v}"
changelog="${CHANGELOG_FILE:-CHANGELOG.md}"

awk -v heading="## [$version] - " '
  index($0, heading) == 1 { found=1; next }
  found && /^## \[/ { exit }
  found { print }
  END { if (!found) exit 1 }
' "$changelog" |
  sed -e '/./,$!d' -e 's/^### /## /' |
  awk '
    { lines[NR]=$0 }
    END {
      last=NR
      while (last > 0 && lines[last] == "") {
        last--
      }
      for (line=1; line <= last; line++) {
        print lines[line]
      }
    }
  '
