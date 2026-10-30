#!/usr/bin/env bash
# Runs the Workzone release binary, building it first if it does not exist yet.
set -euo pipefail
cd "$(dirname "$0")"

exe=src-tauri/target/release/workzone
if [ ! -x "$exe" ]; then
  ./build.sh
fi
exec "$exe" "$@"
