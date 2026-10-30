#!/usr/bin/env bash
# Builds the Workzone release binary (no installers) into src-tauri/target/release/.
set -euo pipefail
cd "$(dirname "$0")"

if [ ! -d node_modules ]; then
  npm install
fi
npm run tauri build -- --no-bundle
