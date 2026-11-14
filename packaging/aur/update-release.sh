#!/usr/bin/env bash
# update-release.sh — Update a clone of an AUR repository ("hivemind-ai" or
# "hivemind-ai-bin") to a release and commit it. Pushing is left to the caller.
#
# Usage:
#   packaging/aur/update-release.sh <package> <version> <aur-clone-dir>
#
# Takes <package>/PKGBUILD from this repository, sets pkgver to <version> and
# pkgrel to 1, fills in the checksums of its sources (the GitHub source tarball
# or the released .deb), test-builds the package and regenerates .SRCINFO.
# Needs makepkg and the dependencies of the package installed (not run as root).

set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "Usage: $0 <package> <version> <aur-clone-dir>" >&2
    exit 1
fi

PACKAGE="$1"
VERSION="$2"
AUR_DIR="$(realpath "$3")"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ ! -f "$SCRIPT_DIR/$PACKAGE/PKGBUILD" ]]; then
    echo "No PKGBUILD for package '$PACKAGE' in $SCRIPT_DIR" >&2
    exit 1
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version '$VERSION' isn't a stable version like 2.0.0" >&2
    exit 1
fi

log() { echo "==> $*"; }

# Keep downloads and build output out of the AUR clone
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT
export SRCDEST="$WORK_DIR" BUILDDIR="$WORK_DIR" PKGDEST="$WORK_DIR"

cp "$SCRIPT_DIR/$PACKAGE/PKGBUILD" "$AUR_DIR/PKGBUILD"
cd "$AUR_DIR"
sed -i "s/^pkgver=.*/pkgver=$VERSION/; s/^pkgrel=.*/pkgrel=1/" PKGBUILD

log "Updating checksums..."
sums="$(makepkg -g)"
# Replace the sha256sums=(...) array, which may span several lines, with the new one
awk -v sums="$sums" '
    /^sha256sums=/ { print sums; skip = !/\)/; next }
    skip { skip = !/\)/; next }
    { print }
' PKGBUILD > PKGBUILD.new
mv PKGBUILD.new PKGBUILD

log "Test-building..."
makepkg -f --noconfirm

makepkg --printsrcinfo > .SRCINFO

git add PKGBUILD .SRCINFO
if git diff --cached --quiet; then
    log "AUR package $PACKAGE is already at $VERSION"
else
    git commit -q -m "Update to $VERSION"
    log "Committed $PACKAGE update to $VERSION"
fi
