# Architecture

Mental model for Snapforge. Audience: future-me + AI coding agents.

## TL;DR

Snapforge = **Rust core** (capture, encode, storage) + **Qt/C++ frontend** (windows, tray, overlays) bridged by a **C ABI FFI**.

```
┌──────────────────────────────────────────────────────────┐
│  Qt/C++ frontend  (qt/src)                               │
│   ├─ tray pill + menu                                    │
│   ├─ region overlay + annotation                         │
│   ├─ preferences + history windows                       │
│   └─ click indicator overlay                             │
└──────────────────────────────────────────────────────────┘
                          │  C ABI (snapforge_*)
                          ▼
┌──────────────────────────────────────────────────────────┐
│  snapforge-ffi  (thin wrapper, type translation)         │
└──────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────┐
│  snapforge-core                                          │
│   ├─ capture (ScreenCaptureKit on macOS)                 │
│   ├─ record  (ffmpeg child process)                      │
│   ├─ clicks  (CGEventTap)                                │
│   ├─ format, history, config, clipboard                  │
│   └─ types                                               │
└──────────────────────────────────────────────────────────┘
```

## Docs

| File | Purpose |
|------|---------|
| [overview.md](overview.md) | Layers, threading, data-flow diagram |
| [modules.md](modules.md) | Every crate + Qt source group: purpose, key files, invariants |
| [ffi-boundary.md](ffi-boundary.md) | Every exported FFI function, ownership rules, error contract |
| [platform-macos.md](platform-macos.md) | ScreenCaptureKit, CGEventTap, TCC perms, code signing, plist keys |
| [recording-pipeline.md](recording-pipeline.md) | Recording end-to-end: config → capture → ffmpeg → file → tray |
| [future-direction.md](future-direction.md) | Phase 2/3 scalable target (deferred crate splits, clients/ layout) |

## Invariants worth knowing before changing anything

1. **FFI returns owned pointers; Qt must free them via the matching `snapforge_free_*`.** Allocator mismatch = heap corruption. See `ffi-boundary.md`.
2. **Recording handle pointers are tracked in a registry.** Any pointer not in the registry is rejected — defends against use-after-free + arbitrary caller pointers.
3. **macOS .mm files only in `qt/src/*.mm`.** No Objective-C in `.cpp`. AppKit headers don't compile under plain C++.
4. **TCC grants are pinned by code-signing identifier (`com.snapforge.app`)**. Don't change the identifier; users lose Screen Recording permission.
5. **ffmpeg is bundled in `Contents/MacOS/ffmpeg-aarch64-apple-darwin`**, dylibs in `Contents/Frameworks/`. `qt/scripts/bundle-ffmpeg.sh` rewrites dylib paths to `@executable_path/../Frameworks/`.
6. **Known duplication**: `crates/snapforge-core/src/clicks.rs` (Rust CGEventTap) and `qt/src/ClickEventTap.mm` (Qt CGEventTap) implement the same thing. Qt one is currently wired into the click visualizer; Rust one is unused. See [future-direction.md](future-direction.md).
