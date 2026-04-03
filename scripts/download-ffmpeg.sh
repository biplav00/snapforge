#!/bin/bash
# Downloads a static FFmpeg binary for the current platform into src-tauri/binaries/
# Run this before `cargo tauri build` to bundle FFmpeg with the app.

set -e

BINARIES_DIR="$(dirname "$0")/../src-tauri/binaries"
mkdir -p "$BINARIES_DIR"

TARGET=$(rustc --print host-tuple 2>/dev/null || rustc -vV | grep host | cut -d' ' -f2)
FFMPEG_NAME="ffmpeg-${TARGET}"

if [ -f "$BINARIES_DIR/$FFMPEG_NAME" ] || [ -f "$BINARIES_DIR/${FFMPEG_NAME}.exe" ]; then
    echo "FFmpeg already exists for $TARGET"
    exit 0
fi

echo "Downloading FFmpeg for $TARGET..."

case "$TARGET" in
    x86_64-apple-darwin)
        curl -L "https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip" -o /tmp/ffmpeg-dl.zip
        unzip -o /tmp/ffmpeg-dl.zip -d /tmp/ffmpeg-extract
        mv /tmp/ffmpeg-extract/ffmpeg "$BINARIES_DIR/$FFMPEG_NAME"
        chmod +x "$BINARIES_DIR/$FFMPEG_NAME"
        rm -f /tmp/ffmpeg-dl.zip
        rm -rf /tmp/ffmpeg-extract
        ;;
    aarch64-apple-darwin)
        # evermeet.cx provides x86_64 static builds; for ARM64 macOS,
        # the x86_64 build works via Rosetta 2. For native ARM64,
        # build from source or use a static build provider.
        echo "Downloading x86_64 build (runs via Rosetta 2 on Apple Silicon)..."
        curl -L "https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip" -o /tmp/ffmpeg-dl.zip
        unzip -o /tmp/ffmpeg-dl.zip -d /tmp/ffmpeg-extract
        mv /tmp/ffmpeg-extract/ffmpeg "$BINARIES_DIR/$FFMPEG_NAME"
        chmod +x "$BINARIES_DIR/$FFMPEG_NAME"
        rm -f /tmp/ffmpeg-dl.zip
        rm -rf /tmp/ffmpeg-extract
        ;;
    x86_64-pc-windows-msvc)
        curl -L "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -o /tmp/ffmpeg-win.zip
        unzip -o /tmp/ffmpeg-win.zip -d /tmp/ffmpeg-win
        FFBIN=$(find /tmp/ffmpeg-win -name "ffmpeg.exe" | head -1)
        mv "$FFBIN" "$BINARIES_DIR/${FFMPEG_NAME}.exe"
        rm -rf /tmp/ffmpeg-win /tmp/ffmpeg-win.zip
        ;;
    x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu)
        ARCH=$(echo "$TARGET" | cut -d'-' -f1)
        if [ "$ARCH" = "x86_64" ]; then
            FFARCH="amd64"
        else
            FFARCH="arm64"
        fi
        curl -L "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-${FFARCH}-static.tar.xz" -o /tmp/ffmpeg-linux.tar.xz
        tar xf /tmp/ffmpeg-linux.tar.xz -C /tmp/
        FFBIN=$(find /tmp/ffmpeg-*-static -name "ffmpeg" -type f | head -1)
        mv "$FFBIN" "$BINARIES_DIR/$FFMPEG_NAME"
        chmod +x "$BINARIES_DIR/$FFMPEG_NAME"
        rm -rf /tmp/ffmpeg-linux.tar.xz /tmp/ffmpeg-*-static
        ;;
    *)
        echo "Unsupported platform: $TARGET"
        echo "Please manually place a static ffmpeg binary at: $BINARIES_DIR/$FFMPEG_NAME"
        exit 1
        ;;
esac

echo "FFmpeg downloaded to $BINARIES_DIR/$FFMPEG_NAME"
ls -la "$BINARIES_DIR/$FFMPEG_NAME"*
