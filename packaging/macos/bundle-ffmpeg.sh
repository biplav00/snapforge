#!/bin/bash
# Bundle ffmpeg + its dylibs into Snapforge.app so the shipped app runs on
# machines without Homebrew. Sources ffmpeg from the local Homebrew install,
# copies its dylib dependencies into Contents/Frameworks/, and rewrites the
# load paths to @executable_path/../Frameworks/.
#
# Architecture handling:
#   - Single-arch app (the default arm64 build): unchanged — bundle the host
#     Homebrew ffmpeg + its dylibs via dylibbundler.
#   - Universal app (arm64 + x86_64, built via SNAPFORGE_UNIVERSAL=1): the
#     host-arch ffmpeg + dylibs are bundled first (dylibbundler), then each
#     bundled Mach-O is lipo-merged with its x86_64 counterpart from a second,
#     x86_64 Homebrew prefix so the bundle is fat. If the second arch isn't
#     available we WARN and ship an arch-limited bundle (won't run on the
#     missing arch) rather than fail the build.
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

# ---------------------------------------------------------------------------
# Universal pass (only when the app binary is fat). dylibbundler above produced
# a host-arch-only ffmpeg + dylibs; if the app is universal we need the other
# arch's slices too. We source them from a second Homebrew prefix that holds
# x86_64 builds (e.g. /usr/local under an Intel/Rosetta Homebrew) and lipo them
# onto the already-bundled, load-path-rewritten host slices.
# ---------------------------------------------------------------------------
APP_BIN="$MACOS_DIR/Snapforge"
APP_ARCHS=""
if [ -x "$APP_BIN" ]; then
    APP_ARCHS="$(lipo -archs "$APP_BIN" 2>/dev/null || true)"
fi

# Treat the app as universal only when lipo reports both arches.
if echo "$APP_ARCHS" | grep -q "arm64" && echo "$APP_ARCHS" | grep -q "x86_64"; then
    echo "=== App is universal ($APP_ARCHS); attempting fat ffmpeg bundle ==="

    # Which arch did we just bundle (the host)? We need the *other* one.
    if [ "$ARCH" = "arm64" ]; then
        OTHER_ARCH="x86_64"
        # Common default for an Intel Homebrew install. Override with
        # SNAPFORGE_X86_BREW_PREFIX if your x86_64 brew lives elsewhere.
        OTHER_BREW_PREFIX="${SNAPFORGE_X86_BREW_PREFIX:-/usr/local}"
    else
        OTHER_ARCH="arm64"
        OTHER_BREW_PREFIX="${SNAPFORGE_ARM_BREW_PREFIX:-/opt/homebrew}"
    fi
    OTHER_FFMPEG_PREFIX="$OTHER_BREW_PREFIX/opt/ffmpeg"

    if [ ! -x "$OTHER_FFMPEG_PREFIX/bin/ffmpeg" ]; then
        echo "WARNING: no $OTHER_ARCH ffmpeg found at $OTHER_FFMPEG_PREFIX/bin/ffmpeg." >&2
        echo "WARNING: the produced bundle is arch-limited to $ARCH and will NOT run" >&2
        echo "WARNING: on $OTHER_ARCH. Install an $OTHER_ARCH Homebrew + ffmpeg under" >&2
        echo "WARNING: $OTHER_BREW_PREFIX (or set SNAPFORGE_${OTHER_ARCH}_BREW_PREFIX)" >&2
        echo "WARNING: and re-run to ship a true universal app." >&2
    else
        echo "--- merging $OTHER_ARCH slices from $OTHER_FFMPEG_PREFIX ---"

        # lipo_merge_other_arch <bundled-macho> <other-arch-source-macho>
        # Adds the other-arch slice to an already-bundled (host-arch) Mach-O.
        # The host slice keeps its rewritten @executable_path load commands; we
        # only graft the missing arch's slice on top so the file becomes fat.
        lipo_merge_other_arch() {
            local bundled="$1" other_src="$2"
            if [ ! -f "$other_src" ]; then
                echo "WARNING: missing $OTHER_ARCH source $other_src — leaving" \
                     "$(basename "$bundled") $ARCH-only" >&2
                return 0
            fi
            # Skip if it's already fat (idempotent re-runs).
            if lipo -archs "$bundled" 2>/dev/null | grep -q "$OTHER_ARCH"; then
                return 0
            fi
            local other_slice
            other_slice="$(mktemp -t sf_other_slice)"
            # Extract just the other arch from the source (it may itself be fat).
            if lipo "$other_src" -thin "$OTHER_ARCH" -output "$other_slice" 2>/dev/null; then
                :
            elif [ "$(lipo -archs "$other_src" 2>/dev/null)" = "$OTHER_ARCH" ]; then
                cp "$other_src" "$other_slice"
            else
                echo "WARNING: $other_src has no $OTHER_ARCH slice — leaving" \
                     "$(basename "$bundled") $ARCH-only" >&2
                rm -f "$other_slice"
                return 0
            fi
            lipo -create "$bundled" "$other_slice" -output "$bundled"
            rm -f "$other_slice"
        }

        # 1. The ffmpeg binary itself.
        lipo_merge_other_arch "$DEST" "$OTHER_FFMPEG_PREFIX/bin/ffmpeg"

        # 2. Each dylib dylibbundler copied into Frameworks/. Match by basename
        #    against the other prefix's Cellar/opt tree. We search the whole
        #    OTHER_BREW_PREFIX so we find deps regardless of which formula owns
        #    them (ffmpeg pulls in dozens of transitive libs).
        shopt -s nullglob
        for dylib in "$FRAMEWORKS_DIR"/*.dylib; do
            base="$(basename "$dylib")"
            # Find the matching other-arch dylib (first hit wins). Resolve
            # symlinks so we lipo the real Mach-O, not a relative link.
            other_lib="$(find "$OTHER_BREW_PREFIX/opt" "$OTHER_BREW_PREFIX/Cellar" \
                -name "$base" -type f 2>/dev/null | head -n1 || true)"
            if [ -z "$other_lib" ]; then
                echo "WARNING: no $OTHER_ARCH match for $base under" \
                     "$OTHER_BREW_PREFIX — bundle stays $ARCH-only for this lib" >&2
                continue
            fi
            lipo_merge_other_arch "$dylib" "$other_lib"
        done
        shopt -u nullglob

        echo "--- fat bundle merge complete (ffmpeg: $(lipo -archs "$DEST")) ---"
    fi
else
    # Single-arch app — nothing to merge. This is the default arm64 path.
    if [ -n "$APP_ARCHS" ]; then
        echo "=== App is single-arch ($APP_ARCHS); arch-matched ffmpeg bundled ==="
    fi
fi

echo "=== ffmpeg bundled: $DEST ==="
