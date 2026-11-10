#!/bin/bash
# Prepend a release section to CHANGELOG.md from the commits since the
# previous release tag, grouped by conventional-commit type.
#
# Usage:
#   packaging/ci/update-changelog.sh <version> [<previous-tag>] [<date>] [<head-ref>]
#
# Examples:
#   packaging/ci/update-changelog.sh 1.4.0 v1.3.21
#   packaging/ci/update-changelog.sh 1.4.0 v1.3.21 2026-11-10
#   packaging/ci/update-changelog.sh 1.3.20 v1.3.19 2026-10-01 v1.3.20
#
# The script is idempotent: running it twice for the same version replaces the
# generated section instead of duplicating it. Run it from the repository root,
# or set SNAPFORGE_ROOT to the repository root.
set -euo pipefail

VERSION="${1:?usage: update-changelog.sh <version> [<previous-tag>] [<date>]}"
PREV_TAG="${2:-}"
DATE="${3:-$(date -u +%Y-%m-%d)}"
HEAD_REF="${4:-HEAD}"
ROOT="${SNAPFORGE_ROOT:-$(git rev-parse --show-toplevel)}"
cd "$ROOT"

FILE="$ROOT/CHANGELOG.md"
REPO_SLUG="${SNAPFORGE_REPO_SLUG:-${GITHUB_REPOSITORY:-}}"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/snapforge-changelog.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

FEATURES="$WORK/features"
FIXES="$WORK/fixes"
PERFORMANCE="$WORK/performance"
REFACTORING="$WORK/refactoring"
DOCS="$WORK/docs"
TESTS="$WORK/tests"
MAINTENANCE="$WORK/maintenance"
OTHER="$WORK/other"
: >"$FEATURES"
: >"$FIXES"
: >"$PERFORMANCE"
: >"$REFACTORING"
: >"$DOCS"
: >"$TESTS"
: >"$MAINTENANCE"
: >"$OTHER"

if [ -n "$PREV_TAG" ]; then
    RANGE="$PREV_TAG..$HEAD_REF"
else
    RANGE="$HEAD_REF"
fi

LOG="$WORK/log"
git log --no-merges --pretty=format:'%H%x09%s' "$RANGE" >"$LOG"
# `git log --pretty=format:` does not emit a trailing newline; add one so the
# while-read loop sees the final commit.
printf '\n' >>"$LOG"

RE_FEAT='^feat(\([^)]*\))?!?:[[:space:]]'
RE_FIX='^fix(\([^)]*\))?!?:[[:space:]]'
RE_PERF='^perf(\([^)]*\))?!?:[[:space:]]'
RE_REFACTOR='^refactor(\([^)]*\))?!?:[[:space:]]'
RE_DOCS='^docs(\([^)]*\))?!?:[[:space:]]'
RE_TEST='^test(\([^)]*\))?!?:[[:space:]]'
RE_MAINT='^(chore|ci|build)(\([^)]*\))?!?:[[:space:]]'
RE_CONVENTIONAL='^[a-z]+(\(([^)]*)\))?!?:[[:space:]]+(.*)$'
RE_PR='\(#([0-9]+)\)$'

categorize() {
    local subject="$1"
    if [[ "$subject" =~ $RE_FEAT ]]; then
        echo features
    elif [[ "$subject" =~ $RE_FIX ]]; then
        echo fixes
    elif [[ "$subject" =~ $RE_PERF ]]; then
        echo performance
    elif [[ "$subject" =~ $RE_REFACTOR ]]; then
        echo refactoring
    elif [[ "$subject" =~ $RE_DOCS ]]; then
        echo docs
    elif [[ "$subject" =~ $RE_TEST ]]; then
        echo tests
    elif [[ "$subject" =~ $RE_MAINT ]]; then
        echo maintenance
    else
        echo other
    fi
}

format_entry() {
    local sha="$1"
    local subject="$2"
    local scope=""
    local desc="$subject"
    local pr=""
    local link=""

    if [[ "$subject" =~ $RE_CONVENTIONAL ]]; then
        scope="${BASH_REMATCH[2]}"
        desc="${BASH_REMATCH[3]}"
    fi

    if [[ "$desc" =~ $RE_PR ]]; then
        pr="${BASH_REMATCH[1]}"
        desc="${desc% \(#${pr}\)}"
    fi

    if [ -n "$pr" ] && [ -n "$REPO_SLUG" ]; then
        link=" ([#${pr}](https://github.com/${REPO_SLUG}/pull/${pr}))"
    elif [ -n "$REPO_SLUG" ]; then
        link=" ([${sha:0:7}](https://github.com/${REPO_SLUG}/commit/${sha}))"
    fi

    if [ -n "$scope" ]; then
        printf -- '- **%s:** %s%s\n' "$scope" "$desc" "$link"
    else
        printf -- '- %s%s\n' "$desc" "$link"
    fi
}

while IFS=$'\t' read -r sha subject; do
    [ -n "$sha" ] || continue
    # Some older commit subjects start with a space; trim so they still match
    # the conventional-commit prefixes.
    subject="${subject#"${subject%%[![:space:]]*}"}"
    case "$subject" in
        'chore(release):'*) continue ;;
    esac

    case "$(categorize "$subject")" in
        features) target="$FEATURES" ;;
        fixes) target="$FIXES" ;;
        performance) target="$PERFORMANCE" ;;
        refactoring) target="$REFACTORING" ;;
        docs) target="$DOCS" ;;
        tests) target="$TESTS" ;;
        maintenance) target="$MAINTENANCE" ;;
        *) target="$OTHER" ;;
    esac
    format_entry "$sha" "$subject" >>"$target"
done <"$LOG"

SECTION="$WORK/section"
{
    printf '## [%s] - %s\n\n' "$VERSION" "$DATE"
    for group in \
        "Features:$FEATURES" \
        "Fixes:$FIXES" \
        "Performance:$PERFORMANCE" \
        "Refactoring:$REFACTORING" \
        "Documentation:$DOCS" \
        "Tests:$TESTS" \
        "Maintenance:$MAINTENANCE" \
        "Other Changes:$OTHER"; do
        title="${group%%:*}"
        entries="${group#*:}"
        if [ -s "$entries" ]; then
            printf '### %s\n\n' "$title"
            cat "$entries"
            printf '\n'
        fi
    done
    if [ ! -s "$FEATURES" ] && [ ! -s "$FIXES" ] && [ ! -s "$PERFORMANCE" ] &&
        [ ! -s "$REFACTORING" ] && [ ! -s "$DOCS" ] && [ ! -s "$TESTS" ] &&
        [ ! -s "$MAINTENANCE" ] && [ ! -s "$OTHER" ]; then
        printf 'No notable changes.\n\n'
    fi
} >"$SECTION"

# Keep every older section, but drop any existing section for this version so
# a rerun replaces it instead of duplicating it.
OLD="$WORK/old"
if [ -f "$FILE" ]; then
    awk -v marker="## [$VERSION]" '
        index($0, marker) == 1 { skip = 1; next }
        skip && /^## \[/ { skip = 0 }
        !skip && /^## \[/ { seen = 1 }
        !skip && seen { print }
    ' "$FILE" >"$OLD"
fi

TMP="$WORK/CHANGELOG.md"
{
    printf '# Changelog\n\n'
    printf 'All notable changes to Snapforge are documented here. This file is updated\n'
    printf 'by `packaging/ci/update-changelog.sh` during releases; generated sections\n'
    printf 'should not be edited by hand.\n\n'
    cat "$SECTION"
    if [ -s "$OLD" ]; then
        cat "$OLD"
    fi
} >"$TMP"
mv "$TMP" "$FILE"

echo "Updated $FILE for $VERSION"
