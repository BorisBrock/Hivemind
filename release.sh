#!/usr/bin/env bash
# Builds a Linux release of Hivemind into .release/:
#   Hivemind-<version>-linux-<arch>.AppImage   portable executable with its libraries bundled
#   Hivemind-<version>-linux-<arch>.tar.gz     the bare binary plus README.md (needs WebKitGTK installed)
# Pass --installers to also build the .deb package and copy it there.
set -euo pipefail
cd "$(dirname "$0")"

bundles=appimage
if [ "${1:-}" = "--installers" ]; then
  bundles=appimage,deb
fi

version=$(node -p "require('./src-tauri/tauri.conf.json').version")
case "$(uname -m)" in
  x86_64) arch=x64 ;;
  aarch64 | arm64) arch=arm64 ;;
  *) arch=$(uname -m) ;;
esac
name="Hivemind-$version-linux-$arch"
out=.release

if [ ! -d node_modules ]; then
  npm install
fi
npm run tauri build -- --bundles "$bundles"

mkdir -p "$out"
bundle=src-tauri/target/release/bundle

appimage=$(ls "$bundle"/appimage/*_"$version"_*.AppImage | head -n 1)
cp "$appimage" "$out/$name.AppImage"
chmod +x "$out/$name.AppImage"

stage="$out/$name"
rm -rf "$stage"
mkdir -p "$stage"
cp src-tauri/target/release/hivemind "$stage/hivemind"
cp README.md "$stage/"
tar -czf "$out/$name.tar.gz" -C "$out" "$name"
rm -rf "$stage"

if [ "$bundles" = "appimage,deb" ]; then
  cp "$bundle"/deb/*_"$version"_*.deb "$out/"
fi

echo
echo "Release files in $PWD/$out:"
ls -1 "$out"
