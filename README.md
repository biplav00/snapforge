# Snapforge

Fast, lightweight screenshot and screen recording tool with annotation support.

## Features

- **Screenshot with region selection** -- Lightshot/Flameshot-style transparent overlay for precise area capture
- **12 annotation tools** -- Arrow, Rectangle, Circle, Line, Dotted Line, Freehand, Text, Highlight, Blur, Step Numbers, Color Picker, Measure
- **Draggable annotation toolbar** -- Reposition the toolbar anywhere on screen
- **Screen recording** -- Record to MP4 (H.264) or GIF via FFmpeg
- **System tray integration** -- Runs in the menu bar with no dock icon on macOS
- **Customizable global hotkeys** -- Set your own shortcuts via Preferences
- **Preferences UI with dark mode** -- Configure format, quality, hotkeys, and recording settings
- **Auto-copy to clipboard** -- Annotated screenshots copied automatically
- **Remember last region** -- Re-capture the same area instantly
- **Cross-platform** -- macOS, Windows, Linux
- **CLI for scripted captures** -- Automate screenshots and recordings from the terminal

## Tech Stack

| Layer | Technology |
|-------|------------|
| Core | Rust (`snapforge-core`) |
| FFI | Rust (`snapforge-ffi`) -- C API for Qt integration |
| GUI | Qt 6 (C++17) |
| Recording | FFmpeg (piped via stdin) |
| CLI | Rust (`cli`) |

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Qt 6](https://www.qt.io/) (Widgets, Gui)
- [CMake](https://cmake.org/) (3.20+)
- [FFmpeg](https://ffmpeg.org/) (for screen recording): `brew install ffmpeg`

### Build from Source

```bash
# Build the Rust FFI library
cargo build --release -p snapforge-ffi

# Build the Qt application
cd qt
mkdir -p build && cd build
cmake ..
make

# Create DMG (macOS)
make dmg
```

The built application will be in `qt/build/snapforge-qt.app`.

## Keyboard Shortcuts

### Global (system-wide)

| Action | Default |
|--------|---------|
| Screenshot | `Cmd+Shift+S` |
| Capture Last Region | `Cmd+Shift+L` |
| Record Screen | `Cmd+Shift+R` |

### Annotation Tools

| Tool | Key |
|------|-----|
| Arrow | `A` |
| Rectangle | `R` |
| Circle | `C` |
| Line | `L` |
| Freehand | `F` |
| Text | `T` |
| Highlight | `H` |
| Blur | `B` |
| Step Numbers | `N` |
| Color Picker | `I` |
| Measurement | `M` |

### Overlay Actions

| Action | Key |
|--------|-----|
| Save | `Cmd+S` / `Enter` |
| Copy | `Cmd+C` |
| Undo | `Cmd+Z` |
| Redo | `Cmd+Shift+Z` |
| Cancel | `Escape` |

All shortcuts are customizable via Preferences.

## CLI Usage

```bash
# Screenshot
snapforge capture --fullscreen
snapforge capture --region 100,200,800,600
snapforge capture --last-region

# Screen recording
snapforge record --fullscreen
snapforge record --region 0,0,1920,1080 --format gif --fps 15
```

## Development

```bash
# Build Rust workspace
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy --workspace

# Format
cargo fmt --all
```

## License

MIT
