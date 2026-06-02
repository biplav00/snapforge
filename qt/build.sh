#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# L6: respect BUILD_TYPE env var (Release default).
BUILD_TYPE="${BUILD_TYPE:-Release}"
BUILD_TYPE_LOWER="$(echo "$BUILD_TYPE" | tr '[:upper:]' '[:lower:]')"

if [ "$BUILD_TYPE_LOWER" = "debug" ]; then
    CARGO_PROFILE_FLAG=""
    PROFILE_DIR="debug"
else
    CARGO_PROFILE_FLAG="--release"
    PROFILE_DIR="release"
fi
# CMake links ${ROOT}/target/<profile>/libsnapforge_ffi.a (see qt/CMakeLists.txt).
# Both the default single-arch build and the universal lipo path below must
# leave the final staticlib at exactly this path.
RUST_TARGET_DIR="$ROOT_DIR/target/$PROFILE_DIR"

# SNAPFORGE_UNIVERSAL=1 opts into a universal (arm64 + x86_64) build. Default
# (unset / not "1") behaviour is unchanged: a single host-arch cargo build and
# a host-arch CMake configure, exactly as before.
UNIVERSAL="${SNAPFORGE_UNIVERSAL:-0}"

if [ "$UNIVERSAL" = "1" ]; then
    # Universal staticlib: build both Apple targets and lipo them into the
    # path CMake already links. Each target's lib lands in
    # target/<triple>/<profile>/libsnapforge_ffi.a; we merge those two.
    ARM_TRIPLE="aarch64-apple-darwin"
    X86_TRIPLE="x86_64-apple-darwin"
    LIB_NAME="libsnapforge_ffi.a"

    echo "=== Building universal Rust FFI library (${BUILD_TYPE}; arm64 + x86_64) ==="
    cd "$ROOT_DIR"

    for TRIPLE in "$ARM_TRIPLE" "$X86_TRIPLE"; do
        echo "--- cargo build --target $TRIPLE ---"
        # If the std for this target isn't installed, cargo fails with a noisy
        # internal error. Catch it and print an actionable hint instead.
        if ! cargo build $CARGO_PROFILE_FLAG --target "$TRIPLE" -p snapforge-ffi; then
            echo "error: failed to build for $TRIPLE." >&2
            echo "       Install the missing Rust std for this target, e.g.:" >&2
            echo "         rustup target add $TRIPLE" >&2
            echo "       (Homebrew Rust ships only the host target; use rustup for" >&2
            echo "        cross-arch builds, then re-run with SNAPFORGE_UNIVERSAL=1.)" >&2
            exit 1
        fi
    done

    ARM_LIB="$ROOT_DIR/target/$ARM_TRIPLE/$PROFILE_DIR/$LIB_NAME"
    X86_LIB="$ROOT_DIR/target/$X86_TRIPLE/$PROFILE_DIR/$LIB_NAME"
    for L in "$ARM_LIB" "$X86_LIB"; do
        if [ ! -f "$L" ]; then
            echo "error: expected staticlib not produced: $L" >&2
            exit 1
        fi
    done

    echo "=== Merging into universal staticlib via lipo ==="
    mkdir -p "$RUST_TARGET_DIR"
    lipo -create "$ARM_LIB" "$X86_LIB" -output "$RUST_TARGET_DIR/$LIB_NAME"
    echo "Universal lib: $RUST_TARGET_DIR/$LIB_NAME ($(lipo -archs "$RUST_TARGET_DIR/$LIB_NAME"))"
else
    echo "=== Building Rust FFI library (${BUILD_TYPE}) ==="
    cd "$ROOT_DIR"
    cargo build $CARGO_PROFILE_FLAG -p snapforge-ffi
fi

echo "=== Configuring Qt project (${BUILD_TYPE}) ==="
cd "$SCRIPT_DIR"
QT_PREFIX="$(brew --prefix qt 2>/dev/null || echo /opt/homebrew/opt/qt)"
# In the universal case, ask CMake to emit a fat Mach-O for the app + tests so
# it can link the universal staticlib. SNAPFORGE_UNIVERSAL is forwarded as a
# cache var; default builds pass nothing extra and stay host-arch (see
# CMakeLists). Pass it as a single optional flag rather than an array so this
# stays correct under `set -u` on macOS's stock Bash 3.2.
CMAKE_UNIVERSAL_FLAG=""
if [ "$UNIVERSAL" = "1" ]; then
    CMAKE_UNIVERSAL_FLAG="-DSNAPFORGE_UNIVERSAL=ON"
fi
cmake -B build -DCMAKE_PREFIX_PATH="$QT_PREFIX" -DCMAKE_BUILD_TYPE="$BUILD_TYPE" \
    $CMAKE_UNIVERSAL_FLAG

echo "=== Building Qt app ==="
cmake --build build

echo "=== Done ==="
echo "Binary: $SCRIPT_DIR/build/Snapforge.app"
echo "Rust artefacts: $RUST_TARGET_DIR"
echo ""
echo "To run (development):"
echo "  QT_PLUGIN_PATH=\"$QT_PREFIX/share/qt/plugins\" $SCRIPT_DIR/build/Snapforge.app/Contents/MacOS/Snapforge"
