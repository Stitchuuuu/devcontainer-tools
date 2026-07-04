#!/usr/bin/env bash
# Re-vendor the macOS SDK stubs under `vendor/macos-sdk/` from an OSS mirror.
#
# Idempotent : safe to re-run — writes into `vendor/macos-sdk/` in place,
# leaving other files (this script, README.md) alone.
#
# Usage :
#   vendor/fetch-sdk.sh                # default : 14.5
#   vendor/fetch-sdk.sh 15.5           # bump when zig upstream fixes #24615
#
# Requires : `curl`, `tar` (xz-aware). Runs from `apps/notifier/` (or any cwd
# with vendor/macos-sdk/ reachable via relative traversal from the script's
# own directory).
set -euo pipefail

VERSION="${1:-14.5}"
MIRROR="https://github.com/joseluisq/macosx-sdks/releases/download"
TARBALL="MacOSX${VERSION}.sdk.tar.xz"

script_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
vendor_dir="$script_dir/macos-sdk"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "[fetch-sdk] downloading ${TARBALL}…"
curl -fsSL -o "$work/$TARBALL" "$MIRROR/$VERSION/$TARBALL"

echo "[fetch-sdk] extracting…"
tar -xJf "$work/$TARBALL" -C "$work"

sdk_root="$work/MacOSX${VERSION}.sdk"

want_frameworks=( Foundation UserNotifications CoreFoundation CoreLocation )
want_libs=( libobjc.tbd libobjc.A.tbd )

mkdir -p "$vendor_dir/System/Library/Frameworks" "$vendor_dir/usr/lib"

for fw in "${want_frameworks[@]}"; do
    src="$sdk_root/System/Library/Frameworks/${fw}.framework/${fw}.tbd"
    dst_dir="$vendor_dir/System/Library/Frameworks/${fw}.framework"
    mkdir -p "$dst_dir"
    cp -f "$src" "$dst_dir/${fw}.tbd"
    echo "[fetch-sdk]   ${fw}.tbd"
done

for lib in "${want_libs[@]}"; do
    cp -f "$sdk_root/usr/lib/$lib" "$vendor_dir/usr/lib/$lib"
    echo "[fetch-sdk]   $lib"
done

echo "[fetch-sdk] done — vendored MacOSX${VERSION} stubs under vendor/macos-sdk/"
