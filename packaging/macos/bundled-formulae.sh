#!/bin/bash
# Print the Homebrew formulae whose code ships inside Snapforge.app, one per
# line, deduplicated: `ffmpeg` first (Contents/MacOS/ffmpeg), then the formula
# that provides each dylib in Contents/Frameworks (FFmpeg's codec libraries
# such as x264/x265, and the non-Qt libraries macdeployqt copied for Qt).
#
# A bundled dylib is mapped to its formula via <brew-prefix>/opt/<formula>/lib/
# <same file name>. Qt's own frameworks (*.framework) are not dylibs and are
# not listed; Qt is handled separately by the callers.
#
# Shared by bundle-licenses.sh (licence texts) and collect-sources.sh
# (corresponding source archives) so both cover exactly the same set.
#
# Usage: bundled-formulae.sh <path-to-Snapforge.app>
# Unmapped dylibs are reported on stderr; they don't fail the script.
set -euo pipefail

if [ $# -lt 1 ] || [ ! -d "$1" ]; then
    echo "usage: $0 <path-to-Snapforge.app>" >&2
    exit 1
fi
APP="$1"
FRAMEWORKS_DIR="$APP/Contents/Frameworks"
BREW_PREFIX="$(brew --prefix 2>/dev/null || echo /opt/homebrew)"

SEEN=" "
emit() {
    case "$SEEN" in *" $1 "*) return 0 ;; esac
    SEEN="$SEEN$1 "
    echo "$1"
}

if [ -x "$APP/Contents/MacOS/ffmpeg" ]; then
    emit ffmpeg
fi

if [ -d "$FRAMEWORKS_DIR" ]; then
    shopt -s nullglob
    for dylib in "$FRAMEWORKS_DIR"/*.dylib; do
        base="$(basename "$dylib")"
        # Several kegs can provide a file of the same name (zstd and pzstd
        # both ship libzstd.1.dylib; ffmpeg@7 next to ffmpeg). Prefer, in
        # order: an unversioned formula whose name the library is named after
        # (libzstd -> zstd), any unversioned formula, then anything.
        formula="" fallback="" versioned=""
        for libpath in "$BREW_PREFIX"/opt/*/lib/"$base"; do
            cand="$(basename "$(dirname "$(dirname "$libpath")")")"
            case "$cand" in
                *@*) [ -n "$versioned" ] || versioned="$cand"; continue ;;
            esac
            case "$base" in
                "lib$cand".*|"$cand".*|"lib$cand"-*|"$cand"-*) formula="$cand"; break ;;
            esac
            [ -n "$fallback" ] || fallback="$cand"
        done
        [ -n "$formula" ] || formula="${fallback:-$versioned}"
        if [ -z "$formula" ]; then
            echo "WARNING: no Homebrew formula found for bundled $base" >&2
            continue
        fi
        emit "$formula"
    done
    shopt -u nullglob
fi
