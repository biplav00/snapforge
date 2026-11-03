#!/bin/bash
# Copy third-party licence notices into Snapforge.app/Contents/Resources/Licenses.
#
# The shipped bundle redistributes FFmpeg (GPL, with x264/x265 and other
# Homebrew codec libraries) and Qt (LGPLv3), so their licence texts, the exact
# versions shipped, and the written source offer must travel with the binary.
#
#   Licenses/THIRD_PARTY_NOTICES.md   overview + source offer (from packaging/macos)
#   Licenses/ffmpeg/                  COPYING.* + `ffmpeg -L` + `ffmpeg -version`
#   Licenses/<formula>/               COPYING/LICENSE files of every Homebrew
#                                     formula whose dylib was bundled into
#                                     Contents/Frameworks (x264, x265, dav1d, ...)
#   Licenses/qt/                      Qt version + LGPLv3/GPLv3 texts
#
# Usage: bundle-licenses.sh <path-to-Snapforge.app> [qt-version]
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "usage: $0 <path-to-Snapforge.app> [qt-version]" >&2
    exit 1
fi
APP="$1"
QT_VERSION="${2:-unknown}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ ! -d "$APP" ]; then
    echo "error: $APP not found" >&2
    exit 1
fi

LIC_DIR="$APP/Contents/Resources/Licenses"
FRAMEWORKS_DIR="$APP/Contents/Frameworks"
BREW_PREFIX="$(brew --prefix 2>/dev/null || echo /opt/homebrew)"
FFMPEG_PREFIX="$(brew --prefix ffmpeg 2>/dev/null || echo "$BREW_PREFIX/opt/ffmpeg")"

rm -rf "$LIC_DIR"
mkdir -p "$LIC_DIR"
cp "$SCRIPT_DIR/THIRD_PARTY_NOTICES.md" "$LIC_DIR/THIRD_PARTY_NOTICES.md"

# copy_licenses <src-prefix> <dest-subdir>: every top-level licence-ish file.
copy_licenses() {
    local src="$1" dest="$2" found=0 f
    mkdir -p "$dest"
    for f in "$src"/COPYING* "$src"/LICENSE* "$src"/LICENCE* "$src"/LICENSES/*; do
        [ -f "$f" ] || continue
        cp "$f" "$dest/"
        found=1
    done
    [ "$found" = "1" ]
}

# --- FFmpeg ------------------------------------------------------------------
# Prefer the bundled binary (exactly what ships); fall back to Homebrew's.
mkdir -p "$LIC_DIR/ffmpeg"
if ! copy_licenses "$FFMPEG_PREFIX" "$LIC_DIR/ffmpeg"; then
    echo "WARNING: no COPYING/LICENSE files under $FFMPEG_PREFIX" >&2
fi
# record_ffmpeg <out-file> <args...>: run the bundled ffmpeg (exactly what
# ships), falling back to Homebrew's identical build if the bundled copy can't
# run yet (mid-build its re-linked dylibs may not be re-signed). Fails the
# build if neither produces output — the notice must name the version.
record_ffmpeg() {
    local out="$1" bin; shift
    for bin in "$APP/Contents/MacOS/ffmpeg" "$FFMPEG_PREFIX/bin/ffmpeg"; do
        [ -x "$bin" ] || continue
        if "$bin" "$@" >"$out" 2>&1 </dev/null && [ -s "$out" ]; then
            return 0
        fi
    done
    echo "error: could not record 'ffmpeg $*' into $out" >&2
    return 1
}
# -L prints the licence the build is actually distributed under (GPLv3+ for
# Homebrew's --enable-gpl --enable-version3 build).
record_ffmpeg "$LIC_DIR/ffmpeg/LICENSE-ffmpeg-L.txt" -hide_banner -L
record_ffmpeg "$LIC_DIR/ffmpeg/VERSION.txt" -hide_banner -version

# --- Every Homebrew formula whose dylib we bundled ---------------------------
# Map each bundled dylib back to the formula that provides it via
# <prefix>/opt/<formula>/lib/<name>. Covers x264, x265 and the rest of FFmpeg's
# codec dependency tree without hard-coding it.
if [ -d "$FRAMEWORKS_DIR" ]; then
    shopt -s nullglob
    for dylib in "$FRAMEWORKS_DIR"/*.dylib; do
        base="$(basename "$dylib")"
        for libpath in "$BREW_PREFIX"/opt/*/lib/"$base"; do
            formula="$(basename "$(dirname "$(dirname "$libpath")")")"
            case "$formula" in ffmpeg|ffmpeg@*) continue ;; esac
            [ -d "$LIC_DIR/$formula" ] && break
            if ! copy_licenses "$BREW_PREFIX/opt/$formula" "$LIC_DIR/$formula"; then
                echo "WARNING: no licence file found for formula '$formula' ($base)" >&2
                echo "See https://formulae.brew.sh/formula/$formula for its licence." \
                    >"$LIC_DIR/$formula/README.txt"
            fi
            break
        done
    done
    shopt -u nullglob
fi

# --- Qt ----------------------------------------------------------------------
# Homebrew's Qt keg ships no licence text. Qt is used under the LGPLv3, whose
# text (plus the GPLv3 it incorporates) is the same file FFmpeg ships, so copy
# those and record the exact version for the source offer.
mkdir -p "$LIC_DIR/qt"
for f in COPYING.LGPLv3 COPYING.GPLv3; do
    if [ -f "$FFMPEG_PREFIX/$f" ]; then
        cp "$FFMPEG_PREFIX/$f" "$LIC_DIR/qt/$f"
    else
        echo "WARNING: $f not found for the Qt notice; see https://www.gnu.org/licenses/" >&2
    fi
done
{
    echo "Qt $QT_VERSION (The Qt Company Ltd. and other contributors)"
    echo "Licensed under the GNU Lesser General Public License v3 (LGPL-3.0-only)."
    echo "Qt frameworks are dynamically linked and live in Contents/Frameworks;"
    echo "you may replace them with your own build of the same Qt version."
    echo "Source: https://download.qt.io/archive/qt/ (version $QT_VERSION)"
} >"$LIC_DIR/qt/README.txt"

echo "=== Licence notices bundled into $LIC_DIR ==="
