#!/usr/bin/env bash
# Stamps a version into src-tauri/Cargo.toml and src-tauri/Cargo.lock before a build.
#
# Git tags are the single source of truth for the version: the repository keeps the
# placeholder 0.0.0, and the release workflow and the AUR packages call this script with
# the version from the tag. Tauri takes the app version from Cargo.toml, so this is the
# only place that needs it. Don't commit the result.
#
# Usage: tools/set-version.sh 1.2.0
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"

version="${1:-}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "Usage: $0 <version>, e.g. $0 1.2.0 or $0 1.2.0-rc1" >&2
  exit 1
fi

# The first "version = ..." line is the one in [package]
sed -i -E "0,/^version = \"[^\"]*\"/s//version = \"$version\"/" Cargo.toml
# The version line right after name = "hivemind", so `cargo --locked` still works
awk -v v="$version" '
  found && /^version = / { print "version = \"" v "\""; found = 0; next }
  { found = ($0 == "name = \"hivemind\""); print }
' Cargo.lock > Cargo.lock.new
mv Cargo.lock.new Cargo.lock

if ! grep -qx "version = \"$version\"" Cargo.toml; then
  echo "Could not set the version in src-tauri/Cargo.toml" >&2
  exit 1
fi
echo "Version set to $version"
