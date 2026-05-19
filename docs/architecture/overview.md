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
              │  main.cpp + window controllers                      │
              │   (currently inline lambdas — see future-direction) │
              └────────────────────┬────────────────────────────────┘
                                   │ extern "C" snapforge_*
              ┌────────────────────▼────────────────────────────────┐
              │  snapforge-ffi (translation only)                   │
              │   JSON in, opaque handle out, error via TLS-ish     │
              │   global `LAST_RECORDING_ERROR`                     │
              └────────────────────┬────────────────────────────────┘
                                   │ snapforge_core::*
              ┌────────────────────▼────────────────────────────────┐
              │  snapforge-core                                     │
              │   capture · record · clicks · format · history      │
              │   config · clipboard · types                        │
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
- **ffmpeg = child process** spawned by `snapforge-core::record::ffmpeg`. Frames piped over stdin.
- **Capture worker = dedicated thread** inside `RecordingHandle`. Pulls frames from ScreenCaptureKit, pushes to ffmpeg stdin.
- **Click event tap = dedicated CFRunLoop thread** (Rust impl in `clicks.rs::macos_tap::start`).
- **GUI work always on main thread.** FFI callbacks dispatch back via `dispatch_async(dispatch_get_main_queue())` or Qt queued connections.

## Data flow — screenshot

```
hotkey ─► OverlayWindow.activate()
       ─► user drags region
       ─► main.cpp builds path + format from PreferencesWindow
       ─► snapforge_capture_region(display, rect) ──► CapturedImage (RGBA buf)
       ─► snapforge_save_image(buf, path, format, quality) ──► file on disk
       ─► snapforge_copy_to_clipboard(buf)  (optional)
       ─► snapforge_history_add(path)
       ─► snapforge_free_buffer(buf, len)
       ─► tray banner "Screenshot saved"
```

## Data flow — recording

See [recording-pipeline.md](recording-pipeline.md) for the full sequence.

## Threading rules

| Code | Thread |
|------|--------|
| Anything touching `QWidget` | Main only |
| ScreenCaptureKit callbacks | SCK worker → bridged to record worker |
| ffmpeg stdin writes | Record worker |
| `snapforge_capture_*` FFI calls | Caller thread (Qt main usually) — blocking, fast |
| `snapforge_start_recording` | Caller thread — blocks until ffmpeg spawned |
| Click tap callback | CFRunLoop thread — must `dispatch_async` to main before touching Qt |

## Why Qt + Rust split

| Why Rust core | Why Qt frontend |
|---------------|-----------------|
| Memory-safe pixel/frame handling | Mature cross-platform UI (future Linux/Win port) |
| Easy to unit-test capture/encode | Native menu bar / NSStatusItem integration via QSystemTrayIcon |
| Single source of truth across clients | C++/Obj-C++ interop with AppKit when needed |
| Already shipped — keep it | Already shipped — keep it |
