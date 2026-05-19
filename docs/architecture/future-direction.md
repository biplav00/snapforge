# Future direction

Scalable target for when growth hurts. Phases below are ordered by cost. Don't do any of this speculatively — wait for the pain.

## Target layout

```
/
├── Cargo.toml, Cargo.lock
├── crates/
│   ├── snapforge-domain/        Pure types + traits. Zero I/O. Easy to unit-test
│   ├── snapforge-capture/       Screen capture (SCK; later X11/Wayland)
│   ├── snapforge-encode/        ffmpeg wrapper + codec presets
│   ├── snapforge-storage/       History index, config, filesystem
│   ├── snapforge-clicks/        Click tap (extracted from snapforge-core)
│   ├── snapforge-app/           Use-case layer: TakeScreenshot, StartRecording. Orchestrates the above
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

### Phase 1 — boundary moves (cheap, do when restructuring)

| Move | Effort | Buys |
|------|--------|------|
| Rename `qt/` → `clients/qt/` | Low (CI + scripts + docs grep) | Signals frontend pluggability |
| Carve `qt/src/` into `{app, ui/{windows,overlay,annotation,tray}, controllers, infra, platform/macos}` | Low (`git mv` + CMake paths) | No more god-files; clear extension points |
| Move `qt/scripts/` → `packaging/macos/` | Low | Separates dev tooling from app tree |
| Carve `docs/` into `{architecture, design, guides}` | Already partially done | Audience-specific docs |
| Extract tray pill + menu builders from `main.cpp` → `ui/tray/` (**Done** — `ui/tray/TrayIcon.{h,cpp}`) | Medium (~200 LOC moved) | `main.cpp` <100 LOC, only DI |
| Extract recording lifecycle from `main.cpp` → `controllers/RecordingController.{h,cpp}` (**Done**) | Medium | Same |

### Phase 2 — Rust crate splits (**Done**)

| Move | Effort | Buys | Status |
|------|--------|------|--------|
| `snapforge-core` → `snapforge-domain` + `snapforge-capture` + `snapforge-encode` + `snapforge-storage` + `snapforge-app` | High | Per-crate testing, parallel work, swappable platform impls | Done (Phase 2A). `snapforge-core` is now a thin facade kept for back-compat. |
| Convert FFI to use-case surface | Medium | Frontends decoupled from internals | Done (Phases 2B–2D). Use cases: `snapforge_screenshot`, `snapforge_save_prerendered`, `snapforge_record_*`, `snapforge_clicks_*`. Errors flow through a single `snapforge_app_last_error()`. |
| Extract `clicks.rs` → `snapforge-clicks` equivalent. Delete `qt/src/ClickEventTap.{h,mm}`. Wire Qt overlay through `snapforge_clicks_start(callback)` FFI | Low–medium | Removes duplicated CGEventTap impl | Done (Phase 2C). Clicks live in `snapforge-capture` / `snapforge-app`; Qt uses `ClickTap` over the new FFI. |
| Delete deprecated primitive FFI (`snapforge_save_image`, `snapforge_copy_to_clipboard`, `snapforge_history_add`, recording-primitive set, `snapforge_last_recording_error`) | Low | Single error path, no parallel surface to keep in sync | Done (Phase 2D). |

### Phase 3 — second client (defer until needed; layout prep complete)

| Move | Effort | Buys |
|------|--------|------|
| Add `clients/swiftui/` or `clients/cli/` | High | Validates the architecture is real |
| Promote shared concepts (e.g. click ripple compositor) to core when duplicated | Low (per move) | DRY across frontends |

## Known debt

Resolved in Phase 2:
- ~~`ClickEventTap.{h,mm}` duplicates `crates/snapforge-core/src/clicks.rs`~~ — deleted in Phase 2C; clicks live in the Rust capture layer with a thin Qt `ClickTap` wrapper.
- ~~Primitive `snapforge_save_image` / `snapforge_copy_to_clipboard` / `snapforge_history_add` / recording-primitive FFI~~ — deleted in Phase 2D; every Qt callsite now uses the use-case surface.
- ~~`snapforge-core` god-crate~~ — split into domain / capture / encode / storage / app in Phase 2A.

Still outstanding (carry into Phase 3 prep):
- **No tests on the Qt side.** Once controllers exist, they become testable (Qt has QTest). Don't write tests against `main.cpp` lambdas.
- **`ffmpeg` is arm64-only.** Intel build needs a second binary + universal dylibs. Blocks shipping a universal `.dmg`.
- **App is not notarized.** Users see Gatekeeper warning on first launch. Needs an Apple Developer ID + `notarytool` step in `packaging/macos/`.
- **`snapforge-core` facade is still in the dep graph.** It re-exports everything from the leaf crates so older internal call sites keep compiling. Once nothing depends on it the facade can be deleted; until then it's a thin layer.

## What NOT to do

- Don't split `crates/` further until the pain is real. Two crates flat is correct today.
- Don't add `tests/` scaffolding speculatively — create when first test arrives.
- Don't introduce plugin systems, dynamic loading, or microservices — wrong layer for a desktop recorder.
- Don't make folders like `common/`, `util/`, `lib/` — vague buckets become dumping grounds.
- Don't change `--identifier com.snapforge.app` in code signing — TCC grants are pinned to it.

## When to actually start Phase 1

Triggers:
- `main.cpp` crosses 1000 LOC.
- Second person joins the project.
- Adding a third top-level window/overlay.
- Whichever comes first.

Today none of these are true. Document is enough.
