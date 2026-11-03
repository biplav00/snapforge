#!/bin/bash
# Build a distributable .dmg for Snapforge.
#
# Usage:
#   packaging/macos/build_dmg.sh                # build release + package
#   SKIP_BUILD=1 packaging/macos/build_dmg.sh   # skip build, repackage existing app
#
# Output: qt/build/dist/Snapforge-<version>.dmg
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
QT_DIR="$REPO_ROOT/qt"
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

# 2b. Verification gate: refuse to package a bundle that can't run on a clean
#     machine. Every check prints what failed; any failure aborts.
echo "=== Verifying bundle ==="
VERIFY_FAILED=0
fail() { echo "verify: FAIL: $*" >&2; VERIFY_FAILED=1; }

# Signature covers every nested binary (ffmpeg, dylibs, Qt frameworks).
if ! codesign --verify --deep --strict --verbose=2 "$APP_PATH"; then
    fail "codesign --verify --deep --strict"
fi

# No Mach-O may load anything from a Homebrew prefix — it would only work on
# machines that happen to have the same Homebrew packages installed.
while IFS= read -r -d '' f; do
    if file -b "$f" | grep -q 'Mach-O'; then
        # Exclude a dylib's own install id (LC_ID_DYLIB, also listed by
        # otool -L): it isn't a load command, so a stale Homebrew id is
        # harmless. Every remaining entry is an LC_LOAD_DYLIB that dyld follows.
        own_id="$(otool -D "$f" 2>/dev/null | tail -n +2 | head -n1)"
        bad="$(otool -L "$f" 2>/dev/null | tail -n +2 | awk '{print $1}' \
               | grep -E '^(/opt/homebrew|/usr/local)/' | grep -vxF "${own_id:-<none>}" || true)"
        if [ -n "$bad" ]; then
            fail "${f#"$APP_PATH"/} references Homebrew paths:"
            echo "$bad" | sed 's/^/        /' >&2
        fi
    fi
done < <(find "$APP_PATH/Contents" -type f -print0)

# Bundled ffmpeg exists and actually runs (catches broken load paths/signature).
FFMPEG_BIN="$APP_PATH/Contents/MacOS/ffmpeg"
if [ ! -x "$FFMPEG_BIN" ]; then
    fail "bundled ffmpeg missing at Contents/MacOS/ffmpeg"
elif ! "$FFMPEG_BIN" -hide_banner -version >/dev/null 2>&1; then
    fail "bundled ffmpeg does not run (-version failed)"
fi

# Architectures: the app (and ffmpeg) must contain every expected slice.
if [ "${SNAPFORGE_UNIVERSAL:-0}" = "1" ]; then
    EXPECTED_ARCHS="arm64 x86_64"
else
    EXPECTED_ARCHS="$(uname -m)"
fi
for bin in "$APP_PATH/Contents/MacOS/Snapforge" "$FFMPEG_BIN"; do
    [ -e "$bin" ] || continue
    have="$(lipo -archs "$bin" 2>/dev/null || true)"
    for a in $EXPECTED_ARCHS; do
        case " $have " in
            *" $a "*) ;;
            *) fail "$(basename "$bin") has archs '$have', missing $a" ;;
        esac
    done
done

# Third-party licence notices must ship with the GPL/LGPL binaries.
if [ ! -f "$APP_PATH/Contents/Resources/Licenses/THIRD_PARTY_NOTICES.md" ]; then
    fail "Contents/Resources/Licenses/THIRD_PARTY_NOTICES.md missing"
fi

if [ "$VERIFY_FAILED" != "0" ]; then
    echo "error: bundle verification failed — not packaging" >&2
    exit 1
fi
echo "verify: OK (archs: $(lipo -archs "$APP_PATH/Contents/MacOS/Snapforge"))"

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

# 5. Ad-hoc sign the DMG (integrity seal only). This does NOT satisfy
#    Gatekeeper: an ad-hoc-signed, un-notarized download is still quarantined,
#    so users must right-click → Open (or run `xattr -dr com.apple.quarantine
#    /Applications/Snapforge.app`) on first launch, and must re-grant Screen
#    Recording after every update because the ad-hoc cdhash changes. Removing
#    those steps needs a Developer ID Application identity + notarization,
#    which this project currently does not use.
codesign --force --sign - "$DMG_PATH"

rm -rf "$STAGE_DIR"

echo ""
echo "=== Done ==="
echo "DMG: $DMG_PATH"
echo "Size: $(du -h "$DMG_PATH" | awk '{print $1}')"
echo ""
echo "Install: open \"$DMG_PATH\" and drag Snapforge to Applications."
