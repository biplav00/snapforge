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

Download the latest `Snapforge-X.Y.Z.dmg` from the [Releases page](https://github.com/biplav00/snapforge/releases), open it, and drag `Snapforge.app` to `/Applications`.

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
| Force Quit (exits app) | `Cmd+Shift+C` |

All shortcuts are customizable via Preferences.

## Development

```bash
cargo build              # build the workspace
cargo test               # run tests
cargo clippy --workspace # lint
cargo fmt --all          # format
packaging/ci/tests/run.sh  # release versioning/changelog tests
```

### Branch model

- `main` is the staging branch and the default branch. All changes land there through a pull request.
- `production` is the release branch. It only receives merges from `main` through the promotion PR that `promote.yml` creates (or refreshes) after every push to `main`.
- CI runs on every pull request and on every push to `main` and `production`.
- `release.yml` runs only after CI succeeds for a push to `production`, so a release is cut only after the promotion PR is merged and the merged commit passes CI.

The promotion PR is created with the repository's `GITHUB_TOKEN`; the checks from the `main` push cover the same commit. This requires **Settings → Actions → General → Allow GitHub Actions to create and approve pull requests**. If you want the promotion PR to run its own `pull_request` CI run as well, add a `PROMOTE_TOKEN` secret (a fine-grained PAT with `contents: read` and `pull-requests: write`) and use it for `GH_TOKEN` in `promote.yml`.

### Versioning

Versions follow [Semantic Versioning](https://semver.org/). The next version is computed from the highest `vX.Y.Z` tag and the conventional commits since that tag:

- a breaking change (`type!:`, `type(scope)!:`, or a `BREAKING CHANGE:` footer) bumps the major version;
- a `feat:` commit bumps the minor version;
- everything else bumps the patch version.

Add `[release:major]`, `[release:minor]`, or `[release:patch]` to any commit subject to force the bump for that release. Add `[skip release]` to the head commit of a production push to skip the release entirely. Preview the next version locally with:

```bash
packaging/ci/next-version.sh
```

### Release pipeline

When CI succeeds on `production`, `release.yml` runs on that exact commit:

1. Computes the next semantic version from the latest tag and the commits since it.
2. Bumps the version in `qt/CMakeLists.txt`, regenerates `CHANGELOG.md`, and builds/verifies `Snapforge-X.Y.Z.dmg` on macOS.
3. Collects the third-party sources archive (`packaging/macos/collect-sources.sh`, below) and creates `Snapforge-X.Y.Z-source.zip` from the release commit.
4. Only if all assets exist: commits `chore(release): vX.Y.Z`, tags `vX.Y.Z`, pushes the tag and `production` atomically, and fast-forwards `main` when it hasn't moved during the build.
5. Creates the GitHub Release with the `.dmg` file, `Snapforge-X.Y.Z-third-party-sources.tar.gz`, and `Snapforge-X.Y.Z-source.zip`; the release notes are the new `CHANGELOG.md` section.

`CHANGELOG.md` is generated by `packaging/ci/update-changelog.sh`; don't edit generated sections by hand. To regenerate it locally:

```bash
packaging/ci/update-changelog.sh X.Y.Z vPREV
```

**Branch protection.** Require pull requests and CI on both `main` and `production`. The release job pushes the `chore(release):` commit to `production` (and best-effort to `main`), so allow the GitHub Actions app to bypass the PR requirement for those branches, or provide a PAT with bypass. If the release commit can't be pushed to `main` because `main` moved during the build, the release still publishes; the next promotion/release fast-forwards `main` over the release commit.

## Third-party licences

The macOS app bundles third-party software, and their licence texts ship inside the app in `Snapforge.app/Contents/Resources/Licenses/` (start with `THIRD_PARTY_NOTICES.md`):

- **FFmpeg** (GPLv3+; Homebrew's build includes libx264 and libx265, both GPLv2+), plus the codec libraries it depends on. Snapforge runs `ffmpeg` as a **separate process** and sends it frames over a pipe; it doesn't link FFmpeg into the app. The exact bundled version and build configuration are recorded in `Licenses/ffmpeg/VERSION.txt`.
- **Qt 6** (LGPLv3), dynamically linked frameworks in `Contents/Frameworks`.

**Source code.** Every release attaches `Snapforge-X.Y.Z-third-party-sources.tar.gz` next to the `.dmg`: the exact upstream source archives of FFmpeg and every bundled GPL/LGPL library, Homebrew's build recipe (`<formula>.rb`) and patches for each, and a `MANIFEST.tsv` of every bundled library with its version, licence and source checksum. Qt's source (~1 GB) isn't included; the archive records the exact Qt version and upstream URL, and `THIRD_PARTY_NOTICES.md` carries a three-year written offer for it. The release also attaches `Snapforge-X.Y.Z-source.zip`, a clean archive of the tagged project source including that release's version bump and `CHANGELOG.md` section.

To build the archive locally (output in the gitignored `qt/build/dist/`; downloads ~80 MB):

```bash
packaging/macos/collect-sources.sh qt/build/Snapforge.app
```

The licence files are collected at build time by `packaging/macos/bundle-licenses.sh`, and the source archive by `packaging/macos/collect-sources.sh`; both use `packaging/macos/bundled-formulae.sh` to map the bundled dylibs to their Homebrew formulae, so they cover the same set. The notice text is in `packaging/macos/THIRD_PARTY_NOTICES.md`.

## License

MIT
