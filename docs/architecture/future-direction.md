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
│   ├── snapforge-app/           Use-case layer: TakeScreenshot, StartRecording, StartClickTracking. Orchestrates the above
│   ├── snapforge-core/          Back-compat facade re-exporting the leaf crates (delete once nothing depends on it)
│   ├── snapforge-ffi/           Thin C ABI. Translates types. No logic
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
| `snapforge-core` → `snapforge-domain` + `snapforge-capture` + `snapforge-encode` + `snapforge-storage` + `snapforge-app` | High | Per-crate testing, parallel work, swappable platform impls | Done (Phase 2A). `snapforge-core` is now a thin facade kept for back-compat. |
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
- ~~`main.cpp` god-file~~ — Phase 1 extractions of `TrayIcon` and `RecordingController` brought it from ~849 LOC down to ~560 LOC of DI + hotkey wiring.
- ~~Inline tray-menu builders and recording-signal slot bodies in `main.cpp`~~ — moved to `ui/tray/TrayIcon` and `controllers/RecordingController`.
- ~~`qt/scripts/` mixed with frontend source~~ — moved to `packaging/macos/`.

Still outstanding:
- **No tests on the Qt side.** Controllers now exist and are testable via QTest; nothing written yet.
- **`ffmpeg` is arm64-only.** Intel build needs a second binary + universal dylibs. Blocks shipping a universal `.dmg`.
- **App is not notarized.** Users see Gatekeeper warning on first launch. Needs an Apple Developer ID + `notarytool` step in `packaging/macos/`.
- **`snapforge-core` facade is still in the dep graph.** It re-exports everything from the leaf crates plus convenience helpers (`screenshot_fullscreen`, `screenshot_region`) so older internal call sites keep compiling. Once nothing depends on it the facade can be deleted; until then it's a thin layer.
- **Phase 3 (second client) is the next milestone.** Layout is ready; no client yet.

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
