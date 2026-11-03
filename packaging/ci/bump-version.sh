#!/bin/bash
# Set all three version fields in qt/CMakeLists.txt to $1 (used by the release
# workflow; they must move together or the bundle/tag versions drift).
set -euo pipefail
NEXT="${1:?usage: bump-version.sh <x.y.z>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
F="$ROOT/qt/CMakeLists.txt"
sed -i.bak -E \
    -e "s/(SNAPFORGE_VERSION \")[^\"]+(\")/\1$NEXT\2/" \
    -e "s/(MACOSX_BUNDLE_BUNDLE_VERSION \")[^\"]+(\")/\1$NEXT\2/" \
    -e "s/(MACOSX_BUNDLE_SHORT_VERSION_STRING \")[^\"]+(\")/\1$NEXT\2/" \
    "$F"
rm -f "$F.bak"
grep -q "SNAPFORGE_VERSION \"$NEXT\"" "$F" || { echo "error: version bump failed" >&2; exit 1; }
