# Snapforge Qt Native Frontend — Design Spec

## Summary

Replace the Tauri/Svelte frontend with a native C++ Qt frontend while keeping the Rust `snapforge-core` backend as a statically linked library via C FFI. Single process, single binary, native UI speed.

## Motivation

The current Tauri-based app has ~800-1000ms hotkey-to-overlay latency due to WebView initialization. Native screenshot tools (Flameshot, Lightshot) achieve ~30-80ms. This migration targets ~50-80ms by eliminating the WebView entirely.

## Architecture

```
┌─────────────────────────────────────────┐
│           C++ Qt Application            │
│                                         │
│  main.cpp            → QApplication     │
│  TrayIcon            → QSystemTrayIcon  │
│  OverlayWindow       → QWidget          │
│    └─ RegionSelector → QPainter         │
│    └─ AnnotationLayer→ QPainter         │
│    └─ Toolbar        → QWidget          │
│  HistoryWindow       → QWidget          │
│  PreferencesWindow   → QWidget          │
│                                         │
│  ┌───────────── C FFI ────────────────┐ │
│  │  libsnapforge_ffi.a                │ │
│  │  (extern "C" wrappers over core)   │ │
│  └────────────────────────────────────┘ │
│                                         │
│  libsnapforge_core.a  (Rust static lib) │
└─────────────────────────────────────────┘
```

### Components

**Kept as-is (Rust — `snapforge-core`):**
- Screen capture (ScreenCaptureKit on macOS, xcap on others)
- Recording pipeline (FFmpeg stdin piping, BGRA frames)
- Click tracking (CGEvent tap)
- Image encoding (PNG/JPG/WebP via `image` crate)
- Clipboard (arboard)
- Config (JSON file read/write)
- History (JSON-based history with thumbnails)
- Format conversion and saving

**New: C FFI crate (`crates/snapforge-ffi/`)**

Thin `extern "C"` wrapper exposing core functionality:

```rust
// Capture
snapforge_capture_fullscreen(display: u32, out_buf: *mut u8, out_len: *mut usize, out_w: *mut u32, out_h: *mut u32) -> i32
snapforge_capture_region(display: u32, x: i32, y: i32, w: u32, h: u32, out_buf: *mut u8, ...) -> i32
snapforge_free_buffer(buf: *mut u8, len: usize)

// Save
snapforge_save_image(rgba_buf: *const u8, w: u32, h: u32, path: *const c_char, format: u32, quality: u8) -> i32

// Clipboard
snapforge_copy_to_clipboard(rgba_buf: *const u8, w: u32, h: u32) -> i32

// Recording
snapforge_start_recording(config_json: *const c_char) -> *mut RecordingHandle
snapforge_stop_recording(handle: *mut RecordingHandle) -> i32

// Config
snapforge_config_load(out_json: *mut *mut c_char) -> i32
snapforge_config_save(json: *const c_char) -> i32
snapforge_free_string(s: *mut c_char)

// History
snapforge_history_list(out_json: *mut *mut c_char) -> i32
snapforge_history_add(path: *const c_char) -> i32
snapforge_history_delete(id: *const c_char) -> i32
snapforge_history_clear() -> i32

// Permission
snapforge_has_permission() -> i32
snapforge_request_permission() -> i32

// Display info
snapforge_display_count() -> u32
snapforge_display_scale_factor() -> f64
```

Return convention: `0` = success, negative = error code. Buffer ownership via `snapforge_free_buffer` / `snapforge_free_string`.

**New: C++ Qt app (`qt/`)**

| Component | Qt class | Notes |
|---|---|---|
| App entry | `QApplication` | Single instance, dock-less on macOS |
| System tray | `QSystemTrayIcon` | Menu: Screenshot, Record, History, Preferences, Quit |
| Global hotkey | Platform-native or QHotkey lib | Cmd+Shift+S (screenshot), Cmd+Shift+R (record) |
| Overlay | `QWidget` (frameless, transparent, fullscreen, always-on-top) | Pre-created, hidden, shown on hotkey |
| Region selector | `QPainter` on overlay | Crosshair cursor, rubber band, resize handles, dimension label, marching ants |
| Dim mask | `QPainter` with clipping path | Dark overlay with cutout for selected region |
| Annotation canvas | `QPainter` on overlay | Arrow, rect, circle, line, freehand, text, highlight, blur (QGraphicsBlurEffect), steps, measure |
| Toolbar | Floating `QWidget` | Positioned below region, tool buttons + save/copy/cancel |
| History | `QWidget` + `QListView` | Thumbnail grid, search, filters |
| Preferences | `QWidget` + `QTabWidget` | General, Screenshots, Recording, Hotkeys tabs |
| Recording indicator | Tray menu change (● Recording / ■ Stop) | No separate window needed |
| Region outline | `QWidget` (frameless, transparent, click-through) | Dashed border during recording |

### Key design decisions

1. **Pre-warm overlay**: The overlay QWidget is created at app startup and kept hidden. On hotkey, it's shown (not created). This is the single biggest speed win — avoids widget construction on every invocation.

2. **Direct pixel buffer**: Capture returns raw RGBA bytes via FFI. Qt converts to QImage directly — no base64, no serialization, no IPC.

3. **Single process**: No IPC overhead. Qt calls Rust functions as regular C function calls through the static library.

4. **Pre-capture before show**: Same strategy as current app — capture the screen before showing the overlay, use the captured image as the overlay background.

5. **QPainter for everything**: Region selection, annotations, dim mask — all painted directly on the overlay widget. No separate rendering layers or canvases.

## Prototype scope

Minimal proof-of-concept to validate speed and approach:

1. **C FFI crate** — expose `capture_fullscreen`, `save_image`, `copy_to_clipboard`, `has_permission`, `request_permission`, `display_count`, `display_scale_factor`, `free_buffer`
2. **Qt app** — system tray + global hotkey (Cmd+Shift+S)
3. **Overlay** — pre-warmed transparent fullscreen QWidget
4. **Region selection** — QPainter: crosshair, rubber band, dim mask, resize handles, dimension label
5. **Save/copy** — capture region via FFI, save to file or clipboard
6. **No annotations, no recording, no history, no preferences** — just the core screenshot flow

### Success criteria
- Hotkey-to-overlay visible: < 100ms
- Region selection feels instant (no frame drops)
- Save/copy works correctly with proper DPI scaling

## Build system

```
qt/
├── CMakeLists.txt        # Qt + link to libsnapforge_ffi.a
├── src/
│   ├── main.cpp
│   ├── TrayIcon.h/cpp
│   ├── OverlayWindow.h/cpp
│   └── snapforge_ffi.h   # C header for FFI functions
└── resources/
    └── icons/
```

Build steps:
```bash
# 1. Build Rust FFI lib
cd crates/snapforge-ffi && cargo build --release
# Output: target/release/libsnapforge_ffi.a

# 2. Build Qt app
cd qt && cmake -B build && cmake --build build
# Links against libsnapforge_ffi.a
```

## Migration path (after prototype)

Once the prototype validates speed:

1. Add annotation layer (QPainter — arrow, rect, circle, line, freehand, text, highlight, blur, steps, measure, colorpicker)
2. Add toolbar (floating widget with tool buttons)
3. Add recording support (FFI for start/stop, region outline widget)
4. Add history window
5. Add preferences window
6. Add keyboard shortcuts (all existing shortcuts)
7. Remove Tauri/Svelte code
8. Update build/release pipeline

## Risks

- **Qt licensing**: Qt is LGPL — dynamic linking required for commercial use without buying a license. Since this is a personal project, not a concern.
- **macOS quirks**: Transparent fullscreen widgets need `NSWindow` manipulation (same as current Tauri approach). Qt provides `winId()` to access the native window handle.
- **Global hotkeys**: Qt doesn't have built-in global hotkeys. Need platform-native code or a library like QHotkey.
- **Click-through overlay during recording**: Requires `setIgnoresMouseEvents:` on macOS — same NSWindow FFI as current code.
