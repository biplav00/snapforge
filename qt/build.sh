#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# L6: respect BUILD_TYPE env var (Release default).
BUILD_TYPE="${BUILD_TYPE:-Release}"
BUILD_TYPE_LOWER="$(echo "$BUILD_TYPE" | tr '[:upper:]' '[:lower:]')"

if [ "$BUILD_TYPE_LOWER" = "debug" ]; then
    CARGO_PROFILE_FLAG=""
    RUST_TARGET_DIR="$ROOT_DIR/target/debug"
else
    CARGO_PROFILE_FLAG="--release"
    RUST_TARGET_DIR="$ROOT_DIR/target/release"
fi

echo "=== Building Rust FFI library (${BUILD_TYPE}) ==="
cd "$ROOT_DIR"
cargo build $CARGO_PROFILE_FLAG -p snapforge-ffi

echo "=== Configuring Qt project (${BUILD_TYPE}) ==="
cd "$SCRIPT_DIR"
QT_PREFIX="$(brew --prefix qt 2>/dev/null || echo /opt/homebrew/opt/qt)"
cmake -B build -DCMAKE_PREFIX_PATH="$QT_PREFIX" -DCMAKE_BUILD_TYPE="$BUILD_TYPE"

echo "=== Building Qt app ==="
cmake --build build

echo "=== Done ==="
echo "Binary: $SCRIPT_DIR/build/Snapforge.app"
echo "Rust artefacts: $RUST_TARGET_DIR"
echo ""
echo "To run (development):"
echo "  QT_PLUGIN_PATH=\"$QT_PREFIX/share/qt/plugins\" $SCRIPT_DIR/build/Snapforge.app/Contents/MacOS/Snapforge"
