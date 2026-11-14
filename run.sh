#!/usr/bin/env bash
# Runs the Hivemind release binary, building it first if it does not exist yet.
set -euo pipefail
cd "$(dirname "$0")"

# Named in tauri.linux.conf.json, the Cargo binary is renamed by `tauri build`
exe=src-tauri/target/release/hivemind-ai
if [ ! -x "$exe" ]; then
  ./build.sh
fi
exec "$exe" "$@"
