#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

frontend/node_modules/.bin/tauri build --config src-tauri/tauri.demo.conf.json

printf 'Demo app: %s\n' "$repo_root/src-tauri/target/release/bundle/macos/Worthweave Demo.app"
printf 'Demo data: ~/Library/Application Support/app.worthweave.portfolio.demo/worthweave.db\n'
