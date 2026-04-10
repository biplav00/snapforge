#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Building Rust FFI library ==="
cd "$ROOT_DIR"
cargo build --release -p snapforge-ffi

echo "=== Configuring Qt project ==="
cd "$SCRIPT_DIR"
QT_PREFIX="$(brew --prefix qt 2>/dev/null || echo /opt/homebrew/opt/qt)"
cmake -B build -DCMAKE_PREFIX_PATH="$QT_PREFIX"

echo "=== Building Qt app ==="
cmake --build build

echo "=== Done ==="
echo "Binary: $SCRIPT_DIR/build/snapforge-qt"
