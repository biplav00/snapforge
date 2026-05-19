# Modules

Authoritative map of every code location. Update when adding/removing modules.

## Rust crates

### `crates/snapforge-core`

Core logic. No Qt, no UI, no C ABI. Public Rust API.

| Module | Purpose | Key types |
|--------|---------|-----------|
| `capture/mod.rs` | Public capture API | `capture_fullscreen`, `capture_region`, `display_count` |
| `capture/macos.rs` | ScreenCaptureKit impl | (internal) |
| `capture/xcap_impl.rs` | Cross-platform fallback (xcap crate) | (internal) |
| `record/mod.rs` | Recording config + ffmpeg path resolution | `RecordConfig`, `RecordError`, `find_ffmpeg` |
| `record/ffmpeg.rs` | ffmpeg child process orchestration | `RecordingHandle`, `start_recording`, `RecordingHandle::stop/pause/resume` |
| `clicks.rs` | Global mouse-click tracker (CGEventTap on macOS) | `ClickTracker`, `MacOSClickTapHandle` |
| `clipboard.rs` | Image → system clipboard | `copy_image_to_clipboard` |
| `config.rs` | App config persistence (JSON) | `AppConfig`, `RecordingFormat`, `RecordingQuality` |
| `format.rs` | Image encode (PNG/JPEG/WebP) | `save_image`, `FormatError` |
| `history.rs` | Recent captures index | `ScreenshotHistory` |
| `types.rs` | Shared value types | `Rect`, `CaptureFormat` |
| `lib.rs` | Convenience entry points | `screenshot_fullscreen`, `screenshot_region`, `ScreenError` |

### `crates/snapforge-ffi`

C ABI wrapper. Translates Rust types ↔ C types. Owns lifetime registries.

- `src/lib.rs` — every `#[no_mangle] extern "C" fn snapforge_*`. See [ffi-boundary.md](ffi-boundary.md) for the full surface.
- `tests/abi.rs` — ABI smoke tests (run in CI).

**Invariants**

- All exported strings are `*mut c_char`. Caller must `snapforge_free_string`.
- All exported buffers are `*mut u8` + `len`. Caller must `snapforge_free_buffer(ptr, len)`. **Length must match what was returned** — `BUFFER_REGISTRY` panics otherwise.
- Recording handles are opaque `*mut c_void`. Tracked in `HANDLE_REGISTRY`. Unknown pointers are rejected without dereferencing.
- Errors go through `LAST_RECORDING_ERROR` static. Read via `snapforge_last_recording_error()` (returns owned string).

## Qt frontend (`qt/src/`)

Flat directory today (28+ files). Restructure deferred — see [future-direction.md](future-direction.md).

### App entry / lifecycle

| File | Purpose |
|------|---------|
| `main.cpp` | App entry. Holds tray icon, hotkeys, all signal wiring. **God-file** — first target for extraction in Phase 1 of restructure. |
| `Logger.{h,cpp}` | App-wide log buffer surfaced in Preferences → Logs tab |

### Capture surface

| File | Purpose |
|------|---------|
| `OverlayWindow.{h,cpp}` | Region picker — full-screen translucent window with drag-to-select. Also entry for fullscreen capture and recording start. |
| `RecordingManager.{h,cpp}` | Wraps `snapforge_start/stop/pause/resume_recording` FFI. Emits Qt signals (`recordingStarted/Stopped/Paused/Resumed/Error/elapsedChanged`). Reads prefs via `reloadPrefs()`. |

### Windows

| File | Purpose |
|------|---------|
| `PreferencesWindow.{h,cpp}` | Tabbed settings: General, Screenshots, Recording, Hotkeys, Permissions, Logs. Persists via `snapforge_config_load/save`. Emits `configSaved` signal. |
| `HistoryWindow.{h,cpp}` | Browses recent captures via `snapforge_history_list`. |

### Annotation subsystem

Self-contained. Used after a screenshot before save.

| File | Purpose |
|------|---------|
| `Annotation.h` | Shared annotation data types |
| `AnnotationState.{h,cpp}` | Undo/redo stack + selected tool state |
| `AnnotationCanvas.{h,cpp}` | QWidget surface for drawing tools |
| `AnnotationRenderer.{h,cpp}` | Composites annotations onto the source pixmap |
| `AnnotationToolbar.{h,cpp}` | Tool picker + colour/stroke controls |

### Tray icon (in `main.cpp` today)

- Idle icon = 18pt aperture+shutter glyph (rendered procedurally).
- Recording icon = pulsing red dot + `MM:SS` timer. Rebuilt every second by a `QTimer`.
- Context menu rebuilt on state change (`buildNormalMenu` / `buildRecordingMenu`).

### Click visualizer

| File | Purpose |
|------|---------|
| `ClickIndicatorOverlay.{h,cpp}` | Transparent click-through Qt window spanning virtual desktop. Draws ~500ms expanding ring per click (red=left, blue=right). |
| `ClickIndicatorOverlayMac.mm` | macOS-only window-level + Space behaviour (`NSScreenSaverWindowLevel`, ignores mouse, joins all Spaces). |
| `ClickEventTap.{h,mm}` | macOS `CGEventTap` listening for left/right mouse-down globally. **Duplicates `clicks.rs` in core** — see [future-direction.md](future-direction.md). |

### Platform observers (macOS)

| File | Purpose |
|------|---------|
| `SpaceChangeObserver.{h,mm}` | Notifies when active macOS Space changes — used to re-show overlays on the current Space |
| `WorkspaceSleepObserver.{h,mm}` | Notifies on system sleep/wake — used to pause/resume recording cleanly |

## Resources / packaging

| Path | Purpose |
|------|---------|
| `qt/resources/AppIcon.icns` | App icon bundled into `Contents/Resources/` |
| `qt/resources/AppIcon.iconset/` | Source PNGs the .icns is built from |
| `qt/resources/icons/` | Tray/menu PNGs |
| `qt/resources/snapforge.qrc` | Qt resource manifest |
| `qt/Info.plist.in` | CMake-templated Info.plist. Holds `LSUIElement` (accessory app), `NSScreenCaptureUsageDescription`, `NSInputMonitoringUsageDescription` |
| `qt/scripts/build_dmg.sh` | DMG packaging |
| `qt/scripts/bundle-ffmpeg.sh` | Copies ffmpeg binary + Homebrew dylibs into bundle, rewrites install names, ad-hoc signs |
| `qt/scripts/build_iconset.py` | Generates `.iconset/*.png` from source SVG/PNG |

## Top-level

| Path | Purpose |
|------|---------|
| `Cargo.toml`, `Cargo.lock` | Rust workspace |
| `target/` | Rust build output (gitignored) |
| `qt/build/` | CMake build output (gitignored) |
| `docs/` | Architecture docs (this folder) + UI mocks |
