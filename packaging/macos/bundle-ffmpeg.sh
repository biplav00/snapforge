#!/bin/bash
# Bundle ffmpeg + its dylibs into Snapforge.app so the shipped app runs on
# machines without Homebrew. Sources ffmpeg from the local Homebrew install,
# copies its dylib dependencies into Contents/Frameworks/, and rewrites the
# load paths to @executable_path/../Frameworks/.
#
# Requires: brew, dylibbundler (install: brew install dylibbundler).
#
# Usage: bundle-ffmpeg.sh <path-to-Snapforge.app>
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "usage: $0 <path-to-Snapforge.app>" >&2
    exit 1
fi
APP="$1"

if [ ! -d "$APP" ]; then
    echo "error: $APP not found" >&2
    exit 1
fi

if ! command -v dylibbundler >/dev/null 2>&1; then
    echo "error: dylibbundler not installed — run: brew install dylibbundler" >&2
    exit 1
fi

FFMPEG_PREFIX="$(brew --prefix ffmpeg 2>/dev/null || true)"
if [ -z "$FFMPEG_PREFIX" ] || [ ! -x "$FFMPEG_PREFIX/bin/ffmpeg" ]; then
    echo "error: brew ffmpeg not found — run: brew install ffmpeg" >&2
    exit 1
fi

ARCH="$(uname -m)"
case "$ARCH" in
    arm64)   SUFFIX="aarch64-apple-darwin" ;;
    x86_64)  SUFFIX="x86_64-apple-darwin" ;;
    *)       echo "error: unsupported arch $ARCH" >&2; exit 1 ;;
esac

MACOS_DIR="$APP/Contents/MacOS"
FRAMEWORKS_DIR="$APP/Contents/Frameworks"
DEST="$MACOS_DIR/ffmpeg-$SUFFIX"

mkdir -p "$FRAMEWORKS_DIR"

echo "=== Bundling ffmpeg ($SUFFIX) from $FFMPEG_PREFIX ==="
cp "$FFMPEG_PREFIX/bin/ffmpeg" "$DEST"
chmod +x "$DEST"

# dylibbundler:
#   -of  overwrite existing files in destination
#   -b   bundle dependencies recursively
#   -x   binary to fix
#   -d   destination dir for copied dylibs
#   -p   load-path prefix to write into binaries
dylibbundler \
    -of -b \
    -x "$DEST" \
    -d "$FRAMEWORKS_DIR" \
    -p "@executable_path/../Frameworks/"

echo "=== ffmpeg bundled: $DEST ==="
