#!/bin/bash
# Build a distributable .dmg for Snapforge.
#
# Usage:
#   qt/scripts/build_dmg.sh                # build release + package
#   SKIP_BUILD=1 qt/scripts/build_dmg.sh   # skip build, repackage existing app
#
# Output: qt/build/dist/Snapforge-<version>.dmg
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$QT_DIR/build"
APP_PATH="$BUILD_DIR/Snapforge.app"
DIST_DIR="$BUILD_DIR/dist"
STAGE_DIR="$BUILD_DIR/dmg-stage"

# 1. Build (unless skipped).
if [ "${SKIP_BUILD:-0}" != "1" ]; then
    echo "=== Building release ==="
    BUILD_TYPE=Release "$QT_DIR/build.sh"
fi

if [ ! -d "$APP_PATH" ]; then
    echo "error: $APP_PATH not found — build failed or skipped without prior build" >&2
    exit 1
fi

# 2. Extract version from Info.plist (single source of truth).
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
    "$APP_PATH/Contents/Info.plist")"
if [ -z "$VERSION" ]; then
    echo "error: could not read CFBundleShortVersionString from Info.plist" >&2
    exit 1
fi
echo "=== Packaging Snapforge $VERSION ==="

# 3. Stage: fresh dir with the app and an /Applications symlink so the user
#    can drag-install from the mounted DMG.
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"
cp -R "$APP_PATH" "$STAGE_DIR/Snapforge.app"
ln -s /Applications "$STAGE_DIR/Applications"

# 4. Build the DMG.
#    - hdiutil create with UDZO = zlib-compressed read-only image.
#    - -volname controls the Finder window title when mounted.
#    - -fs HFS+ for broadest compatibility (APFS would require macOS 10.13+
#      on the receiver side and rejects some legacy mounters).
mkdir -p "$DIST_DIR"
DMG_PATH="$DIST_DIR/Snapforge-$VERSION.dmg"
rm -f "$DMG_PATH"

hdiutil create \
    -volname "Snapforge $VERSION" \
    -srcfolder "$STAGE_DIR" \
    -ov \
    -format UDZO \
    -fs HFS+ \
    "$DMG_PATH"

# 5. Code-sign the DMG with the same ad-hoc identity the build uses so
#    Gatekeeper doesn't quarantine-strip the inner app on first mount.
#    Replace "-" with a real Developer ID Application identity if available.
codesign --force --sign - "$DMG_PATH"

rm -rf "$STAGE_DIR"

echo ""
echo "=== Done ==="
echo "DMG: $DMG_PATH"
echo "Size: $(du -h "$DMG_PATH" | awk '{print $1}')"
echo ""
echo "Install: open \"$DMG_PATH\" and drag Snapforge to Applications."
