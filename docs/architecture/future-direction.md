# Future direction

Scalable target for when growth hurts. Phases below are ordered by cost. Don't do any of this speculatively — wait for the pain.

## Target layout

```
/
├── Cargo.toml, Cargo.lock
├── crates/
│   ├── snapforge-domain/        Pure types. Zero I/O. Easy to unit-test
│   ├── snapforge-capture/       Screen capture (SCK; later X11/Wayland) + global click tap
│   ├── snapforge-encode/        Image format + ffmpeg wrapper + codec presets
│   ├── snapforge-storage/       History index, config, clipboard
│   ├── snapforge-app/           Use-case layer: TakeScreenshot, StartRecording, StartClickTracking. Orchestrates the above. Owns the canonical request schema (serde DTOs)
│   ├── snapforge-ffi/           Thin C ABI. Translates types + deserializes request JSON into the app DTOs. No logic
│   └── snapforge-cli/           (future) shell binary on top of snapforge-app
├── clients/
│   └── qt/                      (renamed from /qt)
│       ├── CMakeLists.txt
│       ├── Info.plist.in
│       ├── resources/
│       └── src/
│           ├── app/             main.cpp, DI, lifetime
│           ├── ui/
│           │   ├── windows/     Preferences, History
│           │   ├── overlay/     Region picker, click indicator
│           │   ├── annotation/  Annotation subsystem
│           │   └── tray/        Pill + menu builders (extracted from main.cpp)
│           ├── controllers/     RecordingController, ClickIndicatorController
│           ├── infra/           Logger
│           └── platform/macos/  All .mm files
├── packaging/
│   └── macos/                   build_dmg, bundle-ffmpeg, build_iconset (was qt/scripts/)
├── docs/
│   ├── architecture/            (this folder)
│   ├── design/                  Mock HTMLs (currently at docs/ root)
│   └── guides/                  User-facing (when added)
└── README.md, LICENSE
```

## Boundary rules (the principles, not the files)

1. **Domain is pure.** `snapforge-domain` has only types + traits. No `std::fs`, no `objc`, no ffmpeg, no Qt. Trait objects for I/O.
2. **FFI exposes use-cases, not primitives.** `snapforge_take_screenshot(req)` is right. A leaked `snapforge_open_encoder` is wrong. Frontends never reach past `snapforge-app`.
3. **Controllers** sit between UI events and FFI. Today these live as inline lambdas in `main.cpp`. Promoting them to `controllers/` keeps `main.cpp` small.
4. **`platform/macos/` only holds AppKit-dependent code.** Anything cross-platform belongs in core (as a trait + per-OS impl) or in `ui/`.
5. **`packaging/`** at top level — distribution isn't a frontend concern. An installer might bundle multiple binaries one day.

## Phases

### Phase 1 — boundary moves (**Done**)

| Move | Effort | Buys | Status |
|------|--------|------|--------|
| Rename `qt/` → `clients/qt/` | Low (CI + scripts + docs grep) | Signals frontend pluggability | **Deferred** — costs touching every script + CI path for no real win until a second client appears. Pulled into Phase 3. |
| Carve `qt/src/` into `{app, ui/{windows,overlay,annotation,tray}, controllers, capture, infra, platform/macos}` | Low (`git mv` + CMake paths) | No more god-files; clear extension points | Done. `qt/CMakeLists.txt` extends `target_include_directories` so the old `#include "Foo.h"` patterns still resolve. |
| Move `qt/scripts/` → `packaging/macos/` | Low | Separates dev tooling from app tree | Done. Scripts now at `packaging/macos/{build_dmg.sh, bundle-ffmpeg.sh, build_iconset.py}`. |
| Carve `docs/` into `{architecture, design, guides}` | Already partially done | Audience-specific docs | Done. |
| Extract tray pill + menu builders from `main.cpp` → `ui/tray/TrayIcon.{h,cpp}` | Medium (~200 LOC moved) | Smaller `main.cpp` focused on DI | Done. `main.cpp` is now ~560 LOC (down from ~849). |
| Extract recording lifecycle from `main.cpp` → `controllers/RecordingController.{h,cpp}` | Medium | Same | Done. |

### Phase 2 — Rust crate splits (**Done**)

| Move | Effort | Buys | Status |
|------|--------|------|--------|
| `snapforge-core` → `snapforge-domain` + `snapforge-capture` + `snapforge-encode` + `snapforge-storage` + `snapforge-app` | High | Per-crate testing, parallel work, swappable platform impls | Done (Phase 2A); facade **deleted** once the FFI was repointed at the leaf crates — nothing else depended on it. |
| Convert FFI to use-case surface | Medium | Frontends decoupled from internals | Done (Phases 2B–2D). Use cases: `snapforge_screenshot`, `snapforge_save_prerendered`, `snapforge_record_*`, `snapforge_clicks_*`. Errors flow through a single `snapforge_app_last_error()`. |
| Extract `clicks.rs` → `snapforge-clicks` equivalent. Delete `qt/src/ClickEventTap.{h,mm}`. Wire Qt overlay through `snapforge_clicks_start(callback)` FFI | Low–medium | Removes duplicated CGEventTap impl | Done (Phase 2C). Clicks live in `snapforge-capture` / `snapforge-app`; Qt uses `ClickTap` over the new FFI. |
| Delete deprecated primitive FFI (`snapforge_save_image`, `snapforge_copy_to_clipboard`, `snapforge_history_add`, recording-primitive set, `snapforge_last_recording_error`) | Low | Single error path, no parallel surface to keep in sync | Done (Phase 2D). |

### Phase 3 — second client (next milestone; layout prep complete)

| Move | Effort | Buys |
|------|--------|------|
| Rename `qt/` → `clients/qt/` (deferred from Phase 1) | Low | Required before adding a sibling client to keep paths uniform |
| Add `clients/swiftui/` or `clients/cli/` | High | Validates the architecture is real |
| Promote shared concepts (e.g. click ripple compositor) to core when duplicated | Low (per move) | DRY across frontends |

## Known debt

Resolved (kept here for changelog context):
- ~~`ClickEventTap.{h,mm}` duplicates `crates/snapforge-core/src/clicks.rs`~~ — deleted in Phase 2C; clicks live in `snapforge-capture::clicks` with a thin Qt `controllers/ClickTap` wrapper.
- ~~Primitive `snapforge_save_image` / `snapforge_copy_to_clipboard` / `snapforge_history_add` / recording-primitive FFI~~ — deleted in Phase 2D; every Qt callsite now uses the use-case surface.
- ~~`snapforge-core` god-crate~~ — split into domain / capture / encode / storage / app in Phase 2A.
- ~~`snapforge-core` back-compat facade lingering in the dep graph~~ — deleted; the FFI (its only consumer) now imports the leaf crates directly, and the duplicate `screenshot_fullscreen`/`screenshot_region` orchestrators + dead `ScreenError` went with it.
- ~~`main.cpp` god-file~~ — Phase 1 extractions of `TrayIcon` and `RecordingController` brought it from ~849 LOC down to ~560 LOC of DI + hotkey wiring.
- ~~Inline tray-menu builders and recording-signal slot bodies in `main.cpp`~~ — moved to `ui/tray/TrayIcon` and `controllers/RecordingController`.
- ~~`qt/scripts/` mixed with frontend source~~ — moved to `packaging/macos/`.

Still outstanding:
- **No tests on the Qt side.** Controllers now exist and are testable via QTest; nothing written yet.
- **`ffmpeg` is arm64-only by default.** The build is now universal-*ready* (see [Universal binary](#universal-binary-arm64--x86_64) below): `SNAPFORGE_UNIVERSAL=1` produces a fat app + lipo'd Rust staticlib, and `bundle-ffmpeg.sh` fat-merges ffmpeg + dylibs when an x86_64 Homebrew is present. The maintainer must still supply the x86_64 Rust target (`rustup target add x86_64-apple-darwin`) and an x86_64 Homebrew ffmpeg; absent those, the default arm64-only `.dmg` is unchanged. A true universal `.dmg` is gated on a CI runner that has both.
- **App is not notarized.** Users see Gatekeeper warning on first launch. Needs an Apple Developer ID + `notarytool` step in `packaging/macos/`.
- **Phase 3 (second client) is the next milestone.** Layout is ready; no client yet.

## Universal binary (arm64 + x86_64)

The build is universal-**ready**: the default path is unchanged (single host-arch
build, arm64 ffmpeg bundle — byte-for-byte what CI ships today), and a universal
build is produced only when the maintainer opts in *and* supplies the x86_64
toolchain + Homebrew artifacts. There are no fabricated x86_64 binaries anywhere.

### How to produce a universal build

```sh
SNAPFORGE_UNIVERSAL=1 ./qt/build.sh
# or for the DMG:
SNAPFORGE_UNIVERSAL=1 ./packaging/macos/build_dmg.sh
```

`SNAPFORGE_UNIVERSAL=1` flows through three layers:

1. **Rust staticlib (`qt/build.sh`).** Builds `aarch64-apple-darwin` *and*
   `x86_64-apple-darwin` separately, then `lipo -create`s the two
   `libsnapforge_ffi.a` slices into `target/<profile>/libsnapforge_ffi.a` — the
   exact path CMake already links, so CMake needs no path change.
2. **CMake (`qt/CMakeLists.txt`).** The `-DSNAPFORGE_UNIVERSAL=ON` option (set by
   `build.sh`, also honoured via the env var) forces
   `CMAKE_OSX_ARCHITECTURES="arm64;x86_64"` so the app + test Mach-Os are fat and
   can link the universal staticlib. When off, `CMAKE_OSX_ARCHITECTURES` is left
   empty (host arch).
3. **ffmpeg bundle (`packaging/macos/bundle-ffmpeg.sh`).** Detects the app
   binary's arches with `lipo -archs`. If the app is fat it bundles the host
   ffmpeg + dylibs as usual (dylibbundler), then lipo-merges the *other* arch's
   slices from a second Homebrew prefix onto each bundled Mach-O.

### What the maintainer must supply (not automated)

- **x86_64 Rust std.** Homebrew Rust ships only the host target. Install the
  cross target with rustup: `rustup target add x86_64-apple-darwin` (and
  `aarch64-apple-darwin`). Without it, `SNAPFORGE_UNIVERSAL=1 ./qt/build.sh`
  fails loudly with this exact instruction — it does not silently fall back.
- **An x86_64 Homebrew with ffmpeg + its dylibs.** `bundle-ffmpeg.sh` looks for
  the other arch under `/usr/local` (Intel Homebrew default) when running on
  arm64, or `/opt/homebrew` when running on x86_64. Override with
  `SNAPFORGE_X86_BREW_PREFIX` / `SNAPFORGE_ARM_BREW_PREFIX`. If the other-arch
  ffmpeg is absent, the script prints a `WARNING` and ships an **arch-limited**
  bundle (the app code is fat, but ffmpeg only runs on the build host's arch).

### Current limitations

- Producing a real universal `.dmg` requires a machine with both: the x86_64
  rustup target and an x86_64 Homebrew ffmpeg install. Neither is fabricated.
- CI does not yet provision the x86_64 toolchain, so released DMGs stay
  arm64-only until a universal CI runner is added.
- Qt itself must be universal for a fully universal app; Homebrew's `qt` is
  arm64-only on Apple Silicon. A universal app therefore also needs a universal
  Qt (e.g. the official Qt installer's universal build) — out of scope for this
  scaffolding, which only makes Snapforge's own code + ffmpeg universal-ready.

## What NOT to do

- Don't split `crates/` further until the pain is real. The Phase 2 split (domain / capture / encode / storage / app / core / ffi) is sufficient.
- Don't add `tests/` scaffolding speculatively — create when first test arrives.
- Don't introduce plugin systems, dynamic loading, or microservices — wrong layer for a desktop recorder.
- Don't make folders like `common/`, `util/`, `lib/` — vague buckets become dumping grounds.
- Don't change `--identifier com.snapforge.app` in code signing — TCC grants are pinned to it.

## When to actually start Phase 3

Triggers:
- Concrete plan to ship a non-Qt client (CLI for headless capture, SwiftUI for a thinner macOS UI, etc.).
- A second contributor who'd benefit from a smaller surface than the Qt app.
- Whichever comes first.

Until then the current single-client layout is correct — `clients/qt/` rename costs touching every script + CI path for no real win.
