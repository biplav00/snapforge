# Architecture

Mental model for Snapforge. Audience: future-me + AI coding agents.

## TL;DR

Snapforge = **Rust core** (split across 6 leaf/use-case crates) + **Qt/C++ frontend** (windows, tray, overlays) bridged by a **C ABI FFI** that exposes use cases — not primitives.

```
┌──────────────────────────────────────────────────────────┐
│  Qt/C++ frontend  (qt/src)                               │
│   app · ui/{windows,overlay,annotation,tray}             │
│   controllers (RecordingController, ClickTap)            │
│   capture (RecordingManager) · infra · platform/macos    │
└──────────────────────────────────────────────────────────┘
                          │  C ABI (snapforge_*)
                          ▼
┌──────────────────────────────────────────────────────────┐
│  snapforge-ffi  (thin C wrapper, type translation)       │
└──────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────┐
│  snapforge-app  (use-case layer)                         │
│   screenshot · recording · clicks · AppError             │
└──────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────┐
│  Leaf crates                                             │
│   snapforge-domain   (Rect, CaptureFormat, LastRegion)   │
│   snapforge-capture  (SCK + xcap fallback + clicks)      │
│   snapforge-encode   (image format + ffmpeg record)      │
│   snapforge-storage  (config, history, clipboard)        │
│   snapforge-core     (facade re-exporting the above)     │
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
| [future-direction.md](future-direction.md) | Phase status, remaining debt, next milestone (second client) |

## Invariants worth knowing before changing anything

1. **FFI returns owned pointers; Qt must free them via the matching `snapforge_free_*`.** Allocator mismatch = heap corruption. See `ffi-boundary.md`.
2. **Recording + click handle pointers are tracked in `HANDLE_REGISTRY`.** Any pointer not in the registry is rejected — defends against use-after-free + arbitrary caller pointers.
3. **macOS `.mm` files only live under `qt/src/platform/macos/` (and `qt/src/ui/overlay/ClickIndicatorOverlayMac.mm`).** No Objective-C in `.cpp`. AppKit headers don't compile under plain C++.
4. **TCC grants are pinned by code-signing identifier (`com.snapforge.app`)**. Don't change the identifier; users lose Screen Recording permission.
5. **ffmpeg is bundled in `Contents/MacOS/ffmpeg-aarch64-apple-darwin`**, dylibs in `Contents/Frameworks/`. `packaging/macos/bundle-ffmpeg.sh` rewrites dylib paths to `@executable_path/../Frameworks/`.
6. **FFI is use-case shaped.** Frontends call `snapforge_screenshot`, `snapforge_save_prerendered`, `snapforge_record_*`, `snapforge_clicks_*` — not low-level save/copy/history primitives (those were removed in Phase 2D). Raw capture primitives (`snapforge_capture_fullscreen` / `_region`) remain because Qt needs the bitmap before annotating.
