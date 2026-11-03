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

Requires **macOS 14 (Sonoma) or later** on Apple silicon (the default build is arm64-only).

Download the latest `Snapforge-vX.Y.Z.dmg` from the [Releases page](https://github.com/biplav00/snapforge/releases), open it, and drag `Snapforge.app` to `/Applications`.

The `.dmg` is **unsigned** (no Apple Developer ID), so on first launch macOS will block it. Either:

- Right-click `Snapforge.app` → **Open** → confirm in the dialog, **or**
- Remove the quarantine flag from a terminal:

  ```bash
  xattr -dr com.apple.quarantine /Applications/Snapforge.app
  ```

On first screenshot/recording, macOS will prompt for **Screen Recording** permission — grant it in System Settings → Privacy & Security, then **fully quit Snapforge from the menu bar and reopen it**. macOS only applies a newly-granted Screen Recording permission to processes started *after* the grant; without a restart, the running process can't see the grant.

> Snapforge is only ad-hoc signed, so macOS identifies each build by its code hash and treats every new release as a different app: **expect to re-grant Screen Recording (and Input Monitoring, if you use "show clicks") after each update.** If the toggle already looks on but captures fail, remove the stale entry: System Settings → Privacy & Security → Screen Recording → select Snapforge → **–**, then relaunch and grant again. A stable grant across updates needs a Developer ID signature, which this project doesn't use.

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

# Or manually (dev build: runs against Homebrew Qt/ffmpeg, also builds the tests)
export MACOSX_DEPLOYMENT_TARGET=14.0
cargo build --release -p snapforge-ffi
cmake -S qt -B qt/build -DCMAKE_PREFIX_PATH="$(brew --prefix qt)"
cmake --build qt/build --parallel
(cd qt/build && QT_QPA_PLATFORM=offscreen ctest --output-on-failure)

# Package as .dmg (macOS) — same script the release workflow runs
packaging/macos/build_dmg.sh
```

The built application is at `qt/build/Snapforge.app`.

`qt/build.sh` configures with `-DSNAPFORGE_BUNDLE=ON -DBUILD_TESTING=OFF`: it copies Qt (macdeployqt), ffmpeg and its dylibs (`dylibbundler`, `brew install dylibbundler`) and the third-party licence notices into the app, then ad-hoc signs it. A plain CMake configure leaves `SNAPFORGE_BUNDLE` OFF (faster; the app only runs on a machine with the same Homebrew packages) and `BUILD_TESTING` ON. `build_dmg.sh` refuses to package a bundle that fails its checks: a valid signature, no Mach-O loading from `/opt/homebrew` or `/usr/local`, a bundled ffmpeg that runs, the expected architectures, and the licence notices present.

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
| Capture Fullscreen | `Cmd+Shift+F` |
| Record Screen (press again to stop) | `Cmd+Shift+R` |
| History | `Cmd+Shift+H` |
| Preferences | `Cmd+,` |

A global shortcut must include Cmd, Ctrl or Option, so it can't take over normal typing. Letter, digit and punctuation keys follow your current keyboard layout (on AZERTY, `Cmd+A` is the key labelled A). If another app already owns a chord, Snapforge shows a menu-bar notification listing the shortcuts it couldn't register. While Preferences is recording a new shortcut, the global shortcuts are paused. In config files, the + key is written `Plus` (for example `Cmd+Plus`).

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

Releases are automated, and only commits that passed CI are released. When the `CI` workflow succeeds for a push to `main` (except the workflow's own `chore(release):` auto-commit), `release.yml` runs on that exact commit:

1. Computes the next patch version from the latest `v*` tag
2. Builds and verifies `Snapforge-X.Y.Z.dmg` on macOS with the bumped version
3. Only if the DMG built: patch-bumps `qt/CMakeLists.txt`, commits as `chore(release): vX.Y.Z`, tags `vX.Y.Z`, and creates the GitHub Release with auto-generated notes and the `.dmg` attached

To land changes without releasing, include the literal opt-out tag in the commit subject (square-bracketed `skip release`). Major/minor bumps need to be done by hand: tag the commit you want to release from with the desired `vX.Y.Z` *minus one patch* (the workflow bumps from the latest tag), then push.

## Third-party licences

The macOS app bundles third-party software, and their licence texts ship inside the app in `Snapforge.app/Contents/Resources/Licenses/` (start with `THIRD_PARTY_NOTICES.md`):

- **FFmpeg** (GPLv3+; Homebrew's build includes libx264 and libx265, both GPLv2+), plus the codec libraries it depends on. Snapforge runs `ffmpeg` as a **separate process** and sends it frames over a pipe; it doesn't link FFmpeg into the app. The exact bundled version and build configuration are recorded in `Licenses/ffmpeg/VERSION.txt`. The notices file includes a written offer for the corresponding source and links to the upstream source.
- **Qt 6** (LGPLv3), dynamically linked frameworks in `Contents/Frameworks`.

The licence files are collected at build time by `packaging/macos/bundle-licenses.sh`. The notice text is in `packaging/macos/THIRD_PARTY_NOTICES.md`.

## License

MIT
