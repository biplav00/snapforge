#!/bin/bash
# Compute the next semantic version from the highest vX.Y.Z tag and the
# conventional commits since that tag.
#
# Rules (in priority order):
#   major  any `type!:` / `type(scope)!:` subject, a `BREAKING CHANGE:` footer,
#          or a `[release:major]` marker
#   minor  any `feat:` / `feat(scope):` subject or a `[release:minor]` marker
#   patch  everything else (the default)
#
# The marker can be in any commit subject since the previous tag; it overrides
# the conventional-commit inference for the whole release.
#
# Usage:
#   packaging/ci/next-version.sh [<previous-tag>]
#
# Prints shell-style key=value lines (prev_tag, bump, next, tag). Run it from
# the repository root, or set SNAPFORGE_ROOT to the repository root.
set -euo pipefail

ROOT="${SNAPFORGE_ROOT:-$(git rev-parse --show-toplevel)}"
cd "$ROOT"

if ! git rev-parse --verify HEAD >/dev/null 2>&1; then
    echo "error: no commits in $ROOT" >&2
    exit 1
fi

if [ "$#" -ge 1 ] && [ -n "$1" ]; then
    PREV_TAG="$1"
else
    # Highest stable tag only; prereleases such as v1.4.0-rc1 are ignored.
    TAGS="$(git tag -l 'v*' --sort=-v:refname)"
    PREV_TAG=""
    while IFS= read -r tag; do
        if [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            PREV_TAG="$tag"
            break
        fi
    done <<<"$TAGS"
fi

CURRENT="${PREV_TAG#v}"
CURRENT="${CURRENT:-0.0.0}"
IFS='.' read -r MAJOR MINOR PATCH <<<"$CURRENT"

if [ -n "$PREV_TAG" ]; then
    RANGE="$PREV_TAG..HEAD"
    SUBJECTS="$(git log --no-merges --format='%s' "$RANGE")"
    BODIES="$(git log --no-merges --format='%b' "$RANGE")"
else
    SUBJECTS="$(git log --no-merges --format='%s')"
    BODIES="$(git log --no-merges --format='%b')"
fi

BUMP=patch
if grep -qE '\[release:major\]' <<<"$SUBJECTS" ||
    grep -qiE 'BREAKING[ -]CHANGE:' <<<"$BODIES" ||
    grep -qE '^[[:space:]]*[a-z]+(\([^)]*\))?!:' <<<"$SUBJECTS"; then
    BUMP=major
elif grep -qE '\[release:minor\]' <<<"$SUBJECTS"; then
    BUMP=minor
elif grep -qE '\[release:patch\]' <<<"$SUBJECTS"; then
    BUMP=patch
elif grep -qE '^[[:space:]]*feat(\([^)]*\))?:' <<<"$SUBJECTS"; then
    BUMP=minor
fi

case "$BUMP" in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEXT="$MAJOR.$MINOR.$PATCH"
printf 'prev_tag=%s\n' "$PREV_TAG"
printf 'bump=%s\n' "$BUMP"
printf 'next=%s\n' "$NEXT"
printf 'tag=v%s\n' "$NEXT"
