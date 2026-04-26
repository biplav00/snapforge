# Snapforge

[![CI](https://github.com/biplav00/snapforge/actions/workflows/ci.yml/badge.svg)](https://github.com/biplav00/snapforge/actions/workflows/ci.yml)

Fast, lightweight macOS screenshot and screen recording tool with annotation support.

## Features

- **Screenshot with region selection** — Lightshot/Flameshot-style transparent overlay for precise area capture
- **12 annotation tools** — Arrow, Rectangle, Circle, Line, Dotted Line, Freehand, Text, Highlight, Blur, Step Numbers, Color Picker, Measure
- **Draggable annotation toolbar** — Reposition the toolbar anywhere on screen
- **Screen recording** — Record to MP4 (H.264) or GIF via FFmpeg, with live click-ripple visualization
- **System tray integration** — Runs in the menu bar with no dock icon
- **Customizable global hotkeys** — Set your own shortcuts via Preferences
- **Preferences UI with dark mode** — Configure format, quality, hotkeys, and recording settings
- **Auto-copy to clipboard** — Annotated screenshots copied automatically
- **Remember last region** — Re-capture the same area instantly

## Install

### From a Release (recommended)

Download the latest `Snapforge-vX.Y.Z.dmg` from the [Releases page](https://github.com/biplav00/snapforge/releases), open it, and drag `Snapforge.app` to `/Applications`.

The `.dmg` is **unsigned** (no Apple Developer ID), so on first launch macOS will block it. Either:

- Right-click `Snapforge.app` → **Open** → confirm in the dialog, **or**
- Remove the quarantine flag from a terminal:

  ```bash
  xattr -dr com.apple.quarantine /Applications/Snapforge.app
  ```

On first screenshot/recording, macOS will prompt for **Screen Recording** permission — grant it in System Settings → Privacy & Security.

### Build from source

Prerequisites:

- [Rust](https://rustup.rs/) (1.70+)
- [Qt 6](https://www.qt.io/) — `brew install qt`
- [CMake](https://cmake.org/) (3.20+) — `brew install cmake`
- [FFmpeg](https://ffmpeg.org/) (for screen recording) — `brew install ffmpeg`

Build:

```bash
# One-shot build via the helper script
bash qt/build.sh

# Or manually
cargo build --release -p snapforge-ffi
cmake -S qt -B qt/build -DCMAKE_PREFIX_PATH="$(brew --prefix qt)"
cmake --build qt/build --parallel

# Package as .dmg (macOS)
cmake --build qt/build --target dmg
```

The built application is at `qt/build/snapforge-qt.app`.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Core | Rust (`snapforge-core`) — capture via ScreenCaptureKit |
| FFI | Rust (`snapforge-ffi`) — C API consumed by Qt |
| GUI | Qt 6 (C++17) |
| Recording | FFmpeg (piped via stdin) |

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

## Development

```bash
cargo build              # build the workspace
cargo test               # run tests
cargo clippy --workspace # lint
cargo fmt --all          # format
```

### Releases

Releases are fully automated. Every push to `main` (other than the workflow's own `chore(release):` commit) triggers `release.yml`, which:

1. Patch-bumps the version in `crates/snapforge-core/Cargo.toml` and `qt/CMakeLists.txt`
2. Updates `Cargo.lock`
3. Commits as `chore(release): vX.Y.Z [skip ci]` and tags `vX.Y.Z`
4. Creates a GitHub Release with auto-generated notes (commits since previous tag)
5. `release-build.yml` then fires on `release: published`, builds `Snapforge-vX.Y.Z.dmg` on macOS, and uploads it as a Release asset

Every `main` push is a release. Want to land changes without releasing? Include `[skip ci]` in the commit subject — `release.yml` will skip that push. Major/minor bumps need to be done by hand: edit `crates/snapforge-core/Cargo.toml` and `qt/CMakeLists.txt` to the desired `X.Y.Z` *minus one patch* (so the workflow bumps the patch *to* it), then push — or just push the desired version and tag manually and let `release-build.yml` build the dmg.

## License

MIT
