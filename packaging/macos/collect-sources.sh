#!/bin/bash
# Collect the complete corresponding source for the GPL/LGPL components
# bundled in Snapforge.app and pack it as
#   <out-dir>/Snapforge-<version>-third-party-sources.tar.gz
#
# The release workflow does NOT run this: releases rely on the three-year
# written offer in THIRD_PARTY_NOTICES.md. Run it when fulfilling a source
# request, or for a local compliance check.
#
# Which formulae: bundled-formulae.sh (ffmpeg + the Homebrew formula behind
# every dylib in Contents/Frameworks), filtered to those whose Homebrew licence
# expression mentions GPL (GPL/LGPL/AGPL; a missing licence counts as GPL so we
# err on the side of shipping). Permissively licensed libraries are listed in
# MANIFEST.tsv but their source isn't included.
#
# For each included formula:
#   <formula>-<version>/<upstream source archive>   via `brew fetch -s` (checksum
#                                                   verified by Homebrew); a
#                                                   git-sourced formula (x264)
#                                                   is `git archive`d at the
#                                                   formula's pinned revision
#   <formula>-<version>/<formula>.rb                Homebrew's build recipe:
#                                                   configure flags + inline
#                                                   (`patch :DATA`) patches
#   <formula>-<version>/patches/                    external patches the
#                                                   formula applies, if any
#
# Qt (LGPLv3, dynamically linked) is NOT included: its source is ~1 GB. The
# archive records the exact Qt version and upstream URL, and the written offer
# in THIRD_PARTY_NOTICES.md covers Qt.
#
# The source must match what was bundled: the script fails if an included
# formula's installed version differs from the formula Homebrew would fetch
# (set SNAPFORGE_ALLOW_SOURCE_MISMATCH=1 to downgrade that to a warning — only
# for local experiments, never for a release).
#
# Requires: brew, jq, git, curl. Network access (downloads ~30-40 MB).
# Usage: collect-sources.sh <path-to-Snapforge.app> [out-dir]
#        (out-dir defaults to qt/build/dist, which is gitignored)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [ $# -lt 1 ] || [ ! -d "$1" ]; then
    echo "usage: $0 <path-to-Snapforge.app> [out-dir]" >&2
    exit 1
fi
APP="$(cd "$1" && pwd)"
OUT_DIR="${2:-$REPO_ROOT/qt/build/dist}"
for tool in brew jq git curl; do
    command -v "$tool" >/dev/null 2>&1 || { echo "error: $tool not found" >&2; exit 1; }
done

VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
NAME="Snapforge-$VERSION-third-party-sources"
mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
ARCHIVE="$OUT_DIR/$NAME.tar.gz"

WORK="$(mktemp -d -t snapforge-sources)"
trap 'rm -rf "$WORK"' EXIT
ROOT="$WORK/$NAME"
mkdir -p "$ROOT"

MANIFEST="$ROOT/MANIFEST.tsv"
printf 'formula\tversion\tlicense\tsource_included\tfile\tsha256\tupstream\n' >"$MANIFEST"

FAILED=0
FORMULAE="$("$SCRIPT_DIR/bundled-formulae.sh" "$APP")"
[ -n "$FORMULAE" ] || { echo "error: no bundled Homebrew formulae found in $APP" >&2; exit 1; }

for f in $FORMULAE; do
    json="$(brew info --json=v2 --formula "$f")"
    field() { jq -r ".formulae[0] | $1 // empty" <<<"$json"; }
    license="$(field .license)"
    stable="$(field .versions.stable)"
    revision="$(field .revision)"
    installed="$(field '.installed[0].version')"
    upstream="$(field .urls.stable.url)"
    expected="$stable"
    [ "${revision:-0}" != "0" ] && expected="${stable}_$revision"

    case "${license:-GPL}" in
        *GPL*) ;;
        *)
            printf '%s\t%s\t%s\tno (not copyleft)\t-\t-\t%s\n' \
                "$f" "${installed:-$expected}" "$license" "$upstream" >>"$MANIFEST"
            continue
            ;;
    esac

    if [ "$installed" != "$expected" ]; then
        msg="$f: installed $installed but Homebrew would fetch source for $expected"
        if [ "${SNAPFORGE_ALLOW_SOURCE_MISMATCH:-0}" = "1" ]; then
            echo "WARNING: $msg" >&2
        else
            echo "error: $msg — the bundled binary and the shipped source would differ." >&2
            echo "       Run 'brew upgrade $f' and rebuild the app." >&2
            FAILED=1
            continue
        fi
    fi

    echo "=== $f $expected ($license) ==="
    dest="$ROOT/$f-$expected"
    mkdir -p "$dest"
    brew fetch --build-from-source --formula "$f" >&2
    cached="$(brew --cache --build-from-source --formula "$f")"
    if [ -d "$cached/.git" ]; then
        # Git-sourced formula: archive exactly the pinned revision.
        rev="$(field .urls.stable.revision)"
        rev="${rev:-HEAD}"
        file="$f-$expected.tar.gz"
        git -C "$cached" archive --format=tar.gz --prefix="$f-$expected/" \
            -o "$dest/$file" "$rev"
        echo "git revision $(git -C "$cached" rev-parse "$rev") of $upstream" >"$dest/GIT-REVISION.txt"
    elif [ -f "$cached" ]; then
        # Cache names are "<url-hash>--<original-name>"; keep the original.
        file="${cached##*/}"
        file="${file#*--}"
        cp "$cached" "$dest/$file"
    else
        echo "error: $f: no fetched source at $cached" >&2
        FAILED=1
        continue
    fi
    sha="$(shasum -a 256 "$dest/$file" | awk '{print $1}')"

    brew cat --formula "$f" >"$dest/$f.rb"

    # External patches (inline `patch :DATA` patches are already in the .rb).
    tap_head="$(field .tap_git_head)"
    npatch="$(jq -r '.formulae[0].patches | length' <<<"$json")"
    i=0
    while [ "$i" -lt "${npatch:-0}" ]; do
        p_url="$(jq -r ".formulae[0].patches[$i].url // empty" <<<"$json")"
        p_file="$(jq -r ".formulae[0].patches[$i].file // empty" <<<"$json")"
        if [ -z "$p_url" ] && [ -n "$p_file" ] && [ -n "$tap_head" ]; then
            p_url="https://raw.githubusercontent.com/Homebrew/homebrew-core/$tap_head/$p_file"
        fi
        if [ -n "$p_url" ]; then
            mkdir -p "$dest/patches"
            curl -fsSL --retry 3 -o "$dest/patches/$((i + 1))-$(basename "$p_url")" "$p_url" || {
                echo "error: $f: could not download patch $p_url" >&2
                FAILED=1
            }
        fi
        i=$((i + 1))
    done

    printf '%s\t%s\t%s\tyes\t%s\t%s\t%s\n' \
        "$f" "$expected" "$license" "$f-$expected/$file" "$sha" "$upstream" >>"$MANIFEST"
done

if [ "$FAILED" != "0" ]; then
    echo "error: source collection failed — not writing $ARCHIVE" >&2
    exit 1
fi

# --- Qt: version + upstream location (written offer covers the rest) ---------
QT_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' \
    "$APP/Contents/Frameworks/QtCore.framework/Resources/Info.plist" 2>/dev/null \
    || brew list --versions qtbase | awk '{print $2}')"
QT_MM="${QT_VERSION%.*}"
mkdir -p "$ROOT/qt"
cat >"$ROOT/qt/SOURCE.txt" <<EOF
Qt $QT_VERSION (LGPL-3.0-only), dynamically linked into Snapforge $VERSION.

The Qt source is not included in this archive because of its size (~1 GB).
The exact upstream source is:
  https://download.qt.io/archive/qt/$QT_MM/$QT_VERSION/single/qt-everywhere-src-$QT_VERSION.tar.xz
built by Homebrew's "qtbase" formula (included here as qtbase.rb).

The written offer in Snapforge.app/Contents/Resources/Licenses/THIRD_PARTY_NOTICES.md
also covers Qt: for three years from the release date we will provide this
source on request.
EOF
brew cat --formula qtbase >"$ROOT/qt/qtbase.rb" 2>/dev/null || true

cat >"$ROOT/README.txt" <<EOF
Third-party sources for Snapforge $VERSION (macOS)
==================================================

This archive is the complete corresponding source for the GPL- and
LGPL-licensed components that ship inside Snapforge-$VERSION.dmg: FFmpeg and
the codec/support libraries bundled with it. Each component was taken from
Homebrew; every folder holds:

  <upstream source archive>  the exact upstream source Homebrew built from
                             (sha256 in MANIFEST.tsv)
  <formula>.rb               Homebrew's build recipe: configure flags,
                             dependencies and any inline patches
  patches/                   external patches the recipe applies (if any)

MANIFEST.tsv lists every bundled Homebrew library with its licence, including
permissively licensed ones whose source is not required (and not included).
Qt is LGPL-licensed too; see qt/SOURCE.txt.

To rebuild a component: unpack the source archive, apply the patches, and run
the configure/build steps from the matching .rb file.
EOF

tar -C "$WORK" -czf "$ARCHIVE" "$NAME"
echo ""
echo "=== Third-party sources: $ARCHIVE ($(du -h "$ARCHIVE" | awk '{print $1}')) ==="
column -t -s "$(printf '\t')" "$MANIFEST" | cut -c1-160
