#!/bin/bash
# Tests for the release-versioning and changelog scripts. Runs in a throwaway
# git repository so it never touches the real project history.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
NEXT_VERSION="$CI_DIR/next-version.sh"
UPDATE_CHANGELOG="$CI_DIR/update-changelog.sh"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/snapforge-ci-tests.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

assert_eq() {
    [ "$1" = "$2" ] || fail "expected '$2', got '$1'"
}

value_of() {
    printf '%s\n' "$1" | sed -n "s/^$2=//p"
}

cd "$TMP"
git init -q
git config user.email "ci-tests@example.com"
git config user.name "CI Tests"
git config commit.gpgsign false
git config tag.gpgsign false

commit() {
    local subject="$1"
    shift
    printf '%s\n' "$subject" >>"$TMP/history.txt"
    git add history.txt
    git commit -q -m "$subject" "$@"
}

next_version() {
    SNAPFORGE_ROOT="$TMP" "$NEXT_VERSION"
}

# --- next-version.sh -------------------------------------------------------
commit "chore: initial"
git tag v1.2.3

out="$(next_version)"
assert_eq "$(value_of "$out" prev_tag)" "v1.2.3"
assert_eq "$(value_of "$out" bump)" "patch"
assert_eq "$(value_of "$out" next)" "1.2.4"
assert_eq "$(value_of "$out" tag)" "v1.2.4"

commit "fix: repair the thing"
out="$(next_version)"
assert_eq "$(value_of "$out" bump)" "patch"
assert_eq "$(value_of "$out" next)" "1.2.4"
git tag v1.2.4

commit "feat(ui): add a widget"
out="$(next_version)"
assert_eq "$(value_of "$out" bump)" "minor"
assert_eq "$(value_of "$out" next)" "1.3.0"
git tag v1.3.0

commit "feat!: drop the old API"
out="$(next_version)"
assert_eq "$(value_of "$out" bump)" "major"
assert_eq "$(value_of "$out" next)" "2.0.0"
git tag v2.0.0

commit "fix: change the on-disk format" -m "BREAKING CHANGE: old files can no longer be read"
out="$(next_version)"
assert_eq "$(value_of "$out" bump)" "major"
assert_eq "$(value_of "$out" next)" "3.0.0"
git tag v3.0.0

commit "feat: add another feature [release:patch]"
out="$(next_version)"
assert_eq "$(value_of "$out" bump)" "patch"
assert_eq "$(value_of "$out" next)" "3.0.1"
git tag v3.0.1

commit "chore: tidy up"
out="$(next_version)"
assert_eq "$(value_of "$out" bump)" "patch"
assert_eq "$(value_of "$out" next)" "3.0.2"

# --- update-changelog.sh ---------------------------------------------------
commit "feat(ui): add widget (#123)"
commit "fix: repair thing (#124)"
commit "docs: update the guide"
commit "chore(release): v9.9.9"

SNAPFORGE_ROOT="$TMP" SNAPFORGE_REPO_SLUG="owner/repo" \
    "$UPDATE_CHANGELOG" "3.1.0" "v3.0.1" "2026-11-10"

grep -qF "## [3.1.0] - 2026-11-10" "$TMP/CHANGELOG.md" ||
    fail "missing 3.1.0 heading"
grep -qF "### Features" "$TMP/CHANGELOG.md" || fail "missing Features section"
grep -qF "### Fixes" "$TMP/CHANGELOG.md" || fail "missing Fixes section"
grep -qF "### Documentation" "$TMP/CHANGELOG.md" || fail "missing Documentation section"
grep -qF "**ui:** add widget ([#123](https://github.com/owner/repo/pull/123))" "$TMP/CHANGELOG.md" ||
    fail "missing linked feature entry"
grep -qF "repair thing ([#124](https://github.com/owner/repo/pull/124))" "$TMP/CHANGELOG.md" ||
    fail "missing linked fix entry"
if grep -qF "chore(release)" "$TMP/CHANGELOG.md"; then
    fail "release commits must not appear in the changelog"
fi

# Rerunning the same version replaces its section rather than duplicating it.
SNAPFORGE_ROOT="$TMP" SNAPFORGE_REPO_SLUG="owner/repo" \
    "$UPDATE_CHANGELOG" "3.1.0" "v3.0.1" "2026-11-10"
count="$(grep -cF "## [3.1.0]" "$TMP/CHANGELOG.md")"
assert_eq "$count" "1"

# A newer release keeps the older sections, newest first.
git tag v3.1.0
commit "fix: one more repair"
SNAPFORGE_ROOT="$TMP" SNAPFORGE_REPO_SLUG="owner/repo" \
    "$UPDATE_CHANGELOG" "3.1.1" "v3.1.0" "2026-11-11"
grep -qF "## [3.1.1] - 2026-11-11" "$TMP/CHANGELOG.md" || fail "missing 3.1.1 heading"
grep -qF "## [3.1.0] - 2026-11-10" "$TMP/CHANGELOG.md" || fail "lost 3.1.0 section"
new_line="$(grep -nF "## [3.1.1]" "$TMP/CHANGELOG.md" | cut -d: -f1)"
old_line="$(grep -nF "## [3.1.0]" "$TMP/CHANGELOG.md" | cut -d: -f1)"
[ "$new_line" -lt "$old_line" ] || fail "newest section must come first"

echo "release tooling tests: OK"
