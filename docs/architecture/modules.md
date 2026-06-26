# Modules

Authoritative map of every code location. Update when adding/removing modules.

## Rust crates

### Rust crate layout (post Phase 2)

The single old `snapforge-core` crate was split into focused crates plus a use-case layer. The back-compat facade has since been deleted (it was a pass-through whose only consumer was the FFI); code now depends on the leaf crates directly or on `snapforge-app`.

| Crate | Purpose | Key items |
|-------|---------|-----------|
| `crates/snapforge-domain` | Value types shared by every layer (no I/O). | `Rect`, `CaptureFormat`, `LastRegion` |
| `crates/snapforge-capture` | Screen capture backends + global click tap. | `capture::capture_fullscreen`, `capture::capture_region`, `display_count`, `has/request_permission`, `display_at_point`, `display_scale_factor`, `clicks::macos_tap` (CGEventTap on its own CFRunLoop thread; tracks left + right via the `right_click: bool` field) |
| `crates/snapforge-encode` | Image and video encoding. | `format::save_image`, `format::encode_image`, `record::ffmpeg::{start_recording, RecordingHandle}`, `record::RecordConfig` |
| `crates/snapforge-storage` | Persistent I/O — config, clipboard, history. | `clipboard::copy_image_to_clipboard`, `config::AppConfig`, `history::ScreenshotHistory`, `history::is_incomplete_mp4` |
| `crates/snapforge-app` | High-level use cases — orchestrate the leaf crates into single end-to-end operations. **This is what `snapforge-ffi` wraps.** | `screenshot::{take_screenshot, save_prerendered}`, `recording::{start_recording, stop_recording, pause_recording, resume_recording, RecordingHandle}`, `clicks::start_click_tracking` (forwarder thread blocks on the capture tracker's event-sink channel — no polling), `AppError`. The `*Request` DTOs (`ScreenshotRequest`, `SavePrerenderedRequest`, `RecordingRequest`) are the **canonical serde schema** the FFI deserializes JSON straight into — one definition of every field, default, and enum spelling. |

### `crates/snapforge-ffi`

C ABI wrapper. Translates Rust types ↔ C types. Owns lifetime registries.

- `src/lib.rs` — every `#[no_mangle] extern "C" fn snapforge_*`. See [ffi-boundary.md](ffi-boundary.md) for the full surface.
- `tests/abi.rs` — ABI smoke tests (run in CI).

**Invariants**

- All exported strings are `*mut c_char`. Caller must `snapforge_free_string`.
- All exported buffers are `*mut u8` + `len`. Caller must `snapforge_free_buffer(ptr, len)`. **Length must match what was returned** — `BUFFER_REGISTRY` rejects the free and leaks rather than corrupt the heap.
- Opaque handles (recording, clicks) are `*mut c_void`. Tracked in `HANDLE_REGISTRY`. Unknown pointers are rejected without dereferencing. In-band magic word catches use-after-free.
- All use-case errors flow through one `LAST_APP_ERROR` static. Read via `snapforge_app_last_error()` (returns owned string). The legacy per-domain `LAST_RECORDING_ERROR` was removed alongside the recording primitives in Phase 2D.

## Qt frontend (`qt/src/`)

Grouped subfolders. Restructure Phase 1 + Phase 1 part 2 (tray + recording controller extraction) complete.

### App entry / lifecycle (`src/app/`, `src/infra/`)

| File | Purpose |
|------|---------|
| `app/main.cpp` | App entry, DI, hotkey registration, top-level object lifetimes. ~560 LOC — no more icon drawing, menu builders, or recording-signal slot bodies inline (down from ~849 LOC pre-Phase-1). |
| `infra/Logger.{h,cpp}` | App-wide log buffer surfaced in Preferences → Logs tab |
| `infra/SnapforgeClient.{h,cpp}` | The Qt-side **FFI seam adapter** — the only TU that `#include`s `snapforge_ffi.h`. Exposes typed `sf::` calls (`takeScreenshot`, `recordStart`, `clicksStart`, `configLoadJson`, …) that hide JSON assembly, `snapforge_app_last_error()` retrieval, and string/buffer freeing. `SnapforgeClientFake.cpp` (+ `SnapforgeClientTesting.h`) is a second adapter swapped in at **link time** for tests — no real FFI, display, or TCC grant. Adoption is **complete** — every Qt consumer (`main.cpp`, `OverlayWindow`, `RecordingManager`, `PreferencesWindow`, `HistoryWindow`, `ClickTap`, `Shortcuts`, `AnnotationCanvas`) goes through `sf::`, and `SnapforgeClient.cpp` is the only translation unit that `#include`s `snapforge_ffi.h`. Fake-backed tests: `tst_clicktap`, `tst_recordingmanager`, `tst_preferences`; real-adapter tests: `tst_snapforgeclient`, `tst_smoke`. See `CONTEXT.md`. |

### Capture surface (`src/capture/`, `src/ui/overlay/`)

| File | Purpose |
|------|---------|
| `ui/overlay/OverlayWindow.{h,cpp}` | Region picker — full-screen translucent window with drag-to-select. Also entry for fullscreen capture and recording start. |
| `capture/RecordingManager.{h,cpp}` | Wraps the `snapforge_record_*` use-case FFI (start/stop/pause/resume/free_handle). Emits Qt signals (`recordingStarted/Stopped/Paused/Resumed/Error/elapsedChanged`). Reads prefs via `reloadPrefs()`. Passes `add_to_history_on_stop=true` so the Rust side indexes the finished file. |

### Windows (`src/ui/windows/`)

| File | Purpose |
|------|---------|
| `ui/windows/PreferencesWindow.{h,cpp}` | Tabbed settings: General, Screenshots, Recording, Hotkeys, Permissions, Logs. Persists via `snapforge_config_load/save`. Emits `configSaved` signal. |
| `ui/windows/HistoryWindow.{h,cpp}` | Browses recent captures via `snapforge_history_list`. |

### Annotation subsystem (`src/ui/annotation/`)

Self-contained. Used after a screenshot before save.

| File | Purpose |
|------|---------|
| `ui/annotation/Annotation.h` | Shared annotation data types |
| `ui/annotation/AnnotationState.{h,cpp}` | Undo/redo stack + selected tool state |
| `ui/annotation/AnnotationCanvas.{h,cpp}` | QWidget surface for drawing tools |
| `ui/annotation/AnnotationRenderer.{h,cpp}` | Composites annotations onto the source pixmap |
| `ui/annotation/AnnotationToolbar.{h,cpp}` | Tool picker + colour/stroke controls |

### Tray icon (`src/ui/tray/`)

| File | Purpose |
|------|---------|
| `ui/tray/TrayIcon.{h,cpp}` | Owns `QSystemTrayIcon`, idle + recording-pill icon factories, pulse `QTimer`, context menu and its idle/recording layouts. Public slots: `enterRecordingState`, `leaveRecordingState`, `updateElapsed`, `setPaused`, `showMessage`. Emits `actionScreenshot/Fullscreen/RecordToggle/History/Preferences/Quit/PauseRecording/ResumeRecording/StopRecording` so main can wire menu items to callers without TrayIcon knowing about them. |

- Idle icon = 18pt aperture+shutter glyph (rendered procedurally).
- Recording icon = pulsing red dot + `MM:SS` timer. Rebuilt every second by the internal `QTimer`.

### Controllers (`src/controllers/`)

| File | Purpose |
|------|---------|
| `controllers/RecordingController.{h,cpp}` | Wires `RecordingManager`'s `recordingStarted/Stopped/Paused/Resumed/Error/elapsedChanged` signals to tray state, click overlay + click-tap toggling, clipboard-copy-on-stop of the finished file URL, and the deferred `QMessageBox` error modal. Constructed in main with refs to `RecordingManager`, `TrayIcon`, `ClickIndicatorOverlay`, `ClickTap` (all platforms), and `PreferencesWindow`. |
| `controllers/ClickTap.{h,cpp}` | Platform-agnostic global mouse-down listener. Calls the clicks use case through `SnapforgeClient` (`sf::clicksStart/Stop/FreeHandle`) rather than raw FFI — so it's unit-testable against the fake (`tst_clicktap`) — the platform tap lives in `snapforge-capture::clicks`. Re-dispatches the FFI callback (Rust-owned thread) to the Qt main thread via `QMetaObject::invokeMethod`. Emits `clicked(QPoint, bool rightClick)`. Replaces the previous macOS-only `ClickEventTap.{h,mm}` (Phase 2C). |

### Click visualizer (`src/ui/overlay/`)

| File | Purpose |
|------|---------|
| `ui/overlay/ClickIndicatorOverlay.{h,cpp}` | Transparent click-through Qt window spanning virtual desktop. Draws ~500ms expanding ring per click (red=left, blue=right). |
| `ui/overlay/ClickIndicatorOverlayMac.mm` | macOS-only window-level + Space behaviour (`NSScreenSaverWindowLevel`, ignores mouse, joins all Spaces). Manages CF types manually — **not** built with ARC. |

The click tap that feeds this overlay is `controllers/ClickTap.{h,cpp}` (listed above). The macOS `CGEventTap` now lives in `snapforge-capture::clicks`; the previous Qt-side `ClickEventTap.{h,mm}` was deleted in Phase 2C.

### Platform observers (`src/platform/macos/`)

| File | Purpose |
|------|---------|
| `platform/macos/SpaceChangeObserver.{h,mm}` | Notifies when active macOS Space changes — used to re-show overlays on the current Space |
| `platform/macos/WorkspaceSleepObserver.{h,mm}` | Notifies on system sleep/wake — used to pause/resume recording cleanly |

## Resources / packaging

| Path | Purpose |
|------|---------|
| `qt/resources/AppIcon.icns` | App icon bundled into `Contents/Resources/` |
| `qt/resources/AppIcon.iconset/` | Source PNGs the .icns is built from |
| `qt/resources/icons/` | Tray/menu PNGs |
| `qt/resources/snapforge.qrc` | Qt resource manifest |
| `qt/Info.plist.in` | CMake-templated Info.plist. Holds `LSUIElement` (accessory app), `NSScreenCaptureUsageDescription`, `NSInputMonitoringUsageDescription` |
| `packaging/macos/build_dmg.sh` | DMG packaging |
| `packaging/macos/bundle-ffmpeg.sh` | Copies ffmpeg binary + Homebrew dylibs into bundle, rewrites install names, ad-hoc signs |
| `packaging/macos/build_iconset.py` | Generates `.iconset/*.png` from source SVG/PNG |

## Top-level

| Path | Purpose |
|------|---------|
| `Cargo.toml`, `Cargo.lock` | Rust workspace |
| `target/` | Rust build output (gitignored) |
| `qt/build/` | CMake build output (gitignored) |
| `docs/` | Architecture docs (this folder) + UI mocks |
