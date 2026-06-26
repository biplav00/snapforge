# Overview

## Layers

```
              ┌─────────────────────────────────────────────────────┐
   USER  ───► │  Qt UI                                              │
              │   region overlay · annotation · prefs · history     │
              │   tray pill · click ripple overlay                  │
              └────────────────────┬────────────────────────────────┘
                                   │ Qt signals/slots
              ┌────────────────────▼────────────────────────────────┐
              │  qt controllers + RecordingManager                  │
              │   RecordingController, ClickTap, TrayIcon           │
              │   (main.cpp = ~560 LOC of DI + hotkey wiring only)  │
              └────────────────────┬────────────────────────────────┘
                                   │ extern "C" snapforge_*
              ┌────────────────────▼────────────────────────────────┐
              │  snapforge-ffi (translation only)                   │
              │   JSON in, opaque handle out; errors via            │
              │   snapforge_app_last_error()                        │
              └────────────────────┬────────────────────────────────┘
                                   │ snapforge_app::*
              ┌────────────────────▼────────────────────────────────┐
              │  snapforge-app  (use cases)                         │
              │   screenshot · recording · clicks · AppError        │
              └────────────────────┬────────────────────────────────┘
                                   │
              ┌────────────────────▼────────────────────────────────┐
              │  Leaf crates                                        │
              │   domain · capture · encode · storage               │
              └────────────────────┬────────────────────────────────┘
                                   │ system calls
              ┌────────────────────▼────────────────────────────────┐
              │  macOS frameworks + bundled ffmpeg child process    │
              │   ScreenCaptureKit · CoreGraphics · AppKit          │
              │   Carbon (hotkeys) · ApplicationServices            │
              └─────────────────────────────────────────────────────┘
```

## Process model

- **One process** total. Qt event loop on the main thread.
- **ffmpeg = child process** spawned by `snapforge-encode::record::ffmpeg`. Frames piped over stdin.
- **Capture worker = dedicated thread** inside the encode-layer `RecordingHandle`. Pulls frames from ScreenCaptureKit, pushes to ffmpeg stdin.
- **Click event tap = dedicated CFRunLoop thread** (Rust impl in `snapforge-capture::clicks`) plus a **forwarder thread** in `snapforge-app::clicks` that polls the tracker at ~60Hz (16ms sleep) and invokes the user-supplied callback.
- **GUI work always on main thread.** FFI click callbacks fire on the forwarder thread; the Qt-side `ClickTap` re-dispatches via `QMetaObject::invokeMethod` (QueuedConnection semantics) before emitting `clicked()`.

## Data flow — screenshot

```
hotkey ─► OverlayWindow.activate()
       ─► snapforge_capture_region(display, rect) ──► CapturedImage (RGBA buf, raw backdrop)
       ─► user drags region + annotates on top in Qt (AnnotationCanvas / Renderer)
       ─► main.cpp builds path + format from PreferencesWindow
       ─► snapforge_save_prerendered(composited_rgba, len, w, h, req_json)
              req_json: {output_path, format, quality, copy_to_clipboard, add_to_history}
            └─► encodes file, optionally copies to clipboard, indexes history
       ─► snapforge_free_buffer(buf, len)
       ─► tray banner "Screenshot saved"
```

Plain fullscreen / region screenshots that never enter the annotation flow can use `snapforge_screenshot(req_json)` instead — it captures, saves, copies, and indexes in one FFI call. Cmd+C-from-region uses `snapforge_save_prerendered` with `output_path` omitted (clipboard-only).

## Data flow — recording

See [recording-pipeline.md](recording-pipeline.md) for the full sequence.

## Threading rules

| Code | Thread |
|------|--------|
| Anything touching `QWidget` | Main only |
| ScreenCaptureKit callbacks | SCK worker → bridged to record worker |
| ffmpeg stdin writes | Record worker |
| `snapforge_capture_*` FFI calls | Caller thread (Qt main usually) — blocking, fast |
| `snapforge_record_start` | Caller thread — blocks until ffmpeg spawned |
| `snapforge_clicks_*` callback | Rust forwarder thread (~60Hz poll) — Qt wrapper re-dispatches to main |

## Why Qt + Rust split

| Why Rust core | Why Qt frontend |
|---------------|-----------------|
| Memory-safe pixel/frame handling | Mature cross-platform UI (future Linux/Win port) |
| Easy to unit-test capture/encode | Native menu bar / NSStatusItem integration via QSystemTrayIcon |
| Single source of truth across clients | C++/Obj-C++ interop with AppKit when needed |
| Already shipped — keep it | Already shipped — keep it |
