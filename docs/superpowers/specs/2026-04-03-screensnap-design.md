# ScreenSnap — Design Spec

A lightweight, cross-platform (macOS/Windows) screenshot and screen recording application with overlay-based annotation.

## Goals

- Fast, lightweight utility that lives in the system tray
- Full annotation toolkit on a frozen-screen overlay (no separate editor window)
- Basic screen recording (region/fullscreen → MP4/GIF)
- Three entry points: system tray, global hotkeys (customizable), CLI
- Local-only output: files + clipboard

## Tech Stack

- **Backend / Core:** Rust (standalone crate `screen-core`)
- **GUI Framework:** Tauri v2
- **Frontend:** Svelte (lightweight, fast, minimal bundle)
- **Recording Encoder:** FFmpeg (bundled)
- **Target Platforms:** macOS, Windows

## Architecture

Two-layer design: Rust core library + Tauri shell.

### `screen-core` (Rust Crate)

Standalone library with no UI dependencies. Handles:

- **Screen Capture** — platform-native APIs behind Rust traits
  - macOS: CoreGraphics (`CGWindowListCreateImage`)
  - Windows: Windows Graphics Capture API
- **Screen Recording** — drives FFmpeg for encoding
  - Frame capture loop using the same platform APIs
  - Pipes frames to FFmpeg for MP4 (H.264) or GIF encoding
- **Format Conversion** — PNG, JPG, WebP, GIF, MP4, SVG
  - JPG/WebP: configurable quality (1-100)
  - GIF: palette generation for optimization
  - SVG: exports annotation vectors
- **Clipboard** — platform clipboard APIs for image copy
  - macOS: `NSPasteboard`
  - Windows: `SetClipboardData`
- **Region Persistence** — stores last-used region coordinates (display, x, y, w, h) in config

Platform-specific code is isolated behind traits:

```rust
trait ScreenCapture {
    fn capture_fullscreen(display: usize) -> Result<ImageBuffer>;
    fn capture_region(display: usize, region: Rect) -> Result<ImageBuffer>;
}

trait ScreenRecorder {
    fn start(config: RecordConfig) -> Result<RecordingHandle>;
    fn stop(handle: RecordingHandle) -> Result<PathBuf>;
}

trait Clipboard {
    fn copy_image(image: &ImageBuffer) -> Result<()>;
}
```

### Tauri Shell (GUI Layer)

Calls `screen-core` via Tauri commands. Handles:

- **Overlay Window** — frameless, transparent, always-on-top Tauri window
  - Displays the frozen full-screen capture as background
  - Region selection drawn on top
  - Annotation tools rendered on HTML5 Canvas
- **Annotation Canvas** — HTML5 Canvas in the overlay webview
- **Floating Toolbar** — appears near selected region
- **System Tray** — tray icon with menu
- **Preferences Window** — separate Tauri window for settings
- **Recording Indicator** — small floating widget (red dot + timer + stop)

### CLI

Separate binary in the workspace. Non-interactive commands call `screen-core` directly (no webview). Interactive commands (region select) launch the Tauri overlay.

```bash
screen capture                           # launches overlay for region select
screen capture --fullscreen              # no overlay, captures immediately via screen-core
screen capture --last-region             # no overlay, captures remembered region via screen-core
screen capture --region 100,200,800,600  # no overlay, captures specific coords via screen-core
screen record                            # launches overlay for region select
screen record --fullscreen               # starts recording immediately
screen record --format gif
screen --list-hotkeys                    # show current bindings
```

## Screenshot Workflow

1. **Trigger** — hotkey (`Ctrl+Shift+S`), tray menu, or CLI
2. **Freeze** — `screen-core` captures full-screen image instantly. Tauri displays it as a frozen overlay (fullscreen, always-on-top, frameless). If "remember last region" is enabled and a previous region exists, that region is pre-selected.
3. **Select region** — user draws a rectangle on the frozen overlay. Live dimensions displayed. Corners are draggable to resize. Entire selection is draggable to reposition.
4. **Annotate** — floating toolbar appears near selection. User annotates directly on the overlay.
5. **Confirm** — Enter or "Save" button. Selected region + annotations composited and saved to file / copied to clipboard.
6. **Cancel** — Escape dismisses overlay.

## Annotation Tools

Full toolkit rendered on HTML5 Canvas in the overlay:

| Tool | Description |
|------|-------------|
| Arrow | Directional arrow with configurable head |
| Rectangle | Outline or filled rectangle |
| Circle/Ellipse | Outline or filled circle |
| Line | Straight line |
| Freehand | Free drawing / pen tool |
| Text | Click to place text label, editable inline |
| Blur/Pixelate | Drag over region to obscure sensitive content |
| Highlight | Semi-transparent color overlay |
| Step Numbers | Click to place numbered circles (auto-incrementing 1, 2, 3...) |
| Color Picker | Eyedropper — pick any color from the frozen screen |
| Measurement | Shows pixel distance between two points |

**Toolbar layout** — floating bar positioned below the selected region (or above if near screen bottom):

```
[Arrow] [Rect] [Circle] [Line] [Freehand] [Text] [Blur] [Highlight] [Steps] [Color] [Size] [Undo] [Save] [Copy] [Cancel]
```

**Annotation state:**
- All annotations stored as a vector of typed objects (not rasterized until export)
- Undo/redo stack (Ctrl+Z / Ctrl+Shift+Z)
- Each tool has configurable: color, stroke width, opacity

## Screen Recording Workflow

1. **Trigger** — hotkey (`Ctrl+Shift+R`), tray menu, or CLI
2. **Select** — same overlay for region selection (or fullscreen). If "remember last region" enabled, previous region pre-selected.
3. **Start** — overlay dismisses. Small floating indicator appears in corner: red dot + elapsed timer + stop button.
4. **Stop** — click stop, press hotkey again, or press Escape.
5. **Save** — file saved as MP4 or GIF based on preference. Desktop notification with file path.

**Recording settings:**
- Formats: MP4 (H.264, default), GIF
- Frame rates: 15 / 24 / 30 / 60 fps (default: 30)
- Quality presets: Low / Medium / High
- GIF: auto palette optimization, max duration warning threshold
- No audio capture, no webcam, no annotations during recording

## System Tray

```
Screenshot          (Ctrl+Shift+S)
Record Screen       (Ctrl+Shift+R)
Capture Last Region (Ctrl+Shift+L)
─────────────────────────────────
Open Save Folder
─────────────────────────────────
Preferences
About
Quit
```

## Global Hotkeys

All customizable via Preferences:

| Action | Default Binding |
|--------|----------------|
| Screenshot | `Ctrl+Shift+S` |
| Record Screen | `Ctrl+Shift+R` |
| Capture Last Region | `Ctrl+Shift+L` |
| Cancel/Dismiss | `Escape` |

## Preferences Window

Separate Tauri window with tabbed layout:

### General Tab
- Default save location (folder picker)
- Auto-copy to clipboard after capture (toggle, default: on)
- Show notification after capture (toggle, default: on)
- Launch at startup (toggle, default: off)
- Remember last region (toggle, default: off)

### Hotkeys Tab
- Table of all actions with current keybinding
- Click binding → press new combo to rebind
- Conflict detection (warns if combo already assigned)
- "Reset to Defaults" button

### Screenshots Tab
- Default format: PNG / JPG / WebP (default: PNG)
- JPG/WebP quality slider (1-100, default: 90)
- Default filename pattern (e.g., `screenshot-{date}-{time}`)

### Recording Tab
- Default format: MP4 / GIF (default: MP4)
- Frame rate: 15 / 24 / 30 / 60 fps (default: 30)
- Quality preset: Low / Medium / High (default: Medium)
- GIF max duration warning threshold (seconds)

### Annotation Defaults Tab
- Default color (color picker, default: red #FF0000)
- Default stroke width (slider, 1-10px, default: 2px)
- Default font size for text tool (default: 16px)
- Default blur intensity (slider, default: medium)

## Output Formats

| Type | Formats | Notes |
|------|---------|-------|
| Screenshots | PNG, JPG, WebP | Configurable quality for JPG/WebP |
| Screenshots | SVG | Exports annotation vectors |
| Recordings | MP4 | H.264 via FFmpeg |
| Recordings | GIF | Palette-optimized via FFmpeg |

## Configuration Storage

App config stored as JSON in platform-standard location:
- macOS: `~/Library/Application Support/screensnap/config.json`
- Windows: `%APPDATA%/screensnap/config.json`

Config includes: all preferences, hotkey bindings, last region data.

## Project Structure

```
screen/
├── Cargo.toml                    # workspace root
├── crates/
│   └── screen-core/              # standalone Rust library
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── capture/          # screen capture (platform traits)
│           │   ├── mod.rs
│           │   ├── macos.rs
│           │   └── windows.rs
│           ├── record/           # screen recording + FFmpeg
│           │   ├── mod.rs
│           │   ├── encoder.rs
│           │   └── gif.rs
│           ├── format/           # image format conversion
│           │   └── mod.rs
│           ├── clipboard/        # platform clipboard
│           │   ├── mod.rs
│           │   ├── macos.rs
│           │   └── windows.rs
│           └── config/           # config loading/saving, region persistence
│               └── mod.rs
├── src-tauri/                    # Tauri app
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── commands.rs           # Tauri command handlers
│       ├── tray.rs               # system tray setup
│       └── hotkeys.rs            # global hotkey registration
├── src/                          # Svelte frontend
│   ├── App.svelte
│   ├── main.ts
│   ├── lib/
│   │   ├── overlay/              # overlay + region selection
│   │   │   ├── Overlay.svelte
│   │   │   └── RegionSelector.svelte
│   │   ├── annotation/           # canvas annotation tools
│   │   │   ├── Canvas.svelte
│   │   │   ├── Toolbar.svelte
│   │   │   └── tools/            # individual tool implementations
│   │   │       ├── arrow.ts
│   │   │       ├── rect.ts
│   │   │       ├── circle.ts
│   │   │       ├── line.ts
│   │   │       ├── freehand.ts
│   │   │       ├── text.ts
│   │   │       ├── blur.ts
│   │   │       ├── highlight.ts
│   │   │       ├── steps.ts
│   │   │       ├── colorpicker.ts
│   │   │       └── measure.ts
│   │   ├── preferences/          # preferences window
│   │   │   ├── Preferences.svelte
│   │   │   ├── GeneralTab.svelte
│   │   │   ├── HotkeysTab.svelte
│   │   │   ├── ScreenshotsTab.svelte
│   │   │   ├── RecordingTab.svelte
│   │   │   └── AnnotationTab.svelte
│   │   └── recording/            # recording indicator widget
│   │       └── Indicator.svelte
│   └── stores/                   # Svelte stores for state
│       ├── settings.ts
│       └── annotation.ts
└── cli/                          # CLI binary
    ├── Cargo.toml
    └── src/
        └── main.rs               # clap-based CLI, calls screen-core
```

## Non-Goals (for now)

- Cloud upload / sharing links
- Integrations (Slack, Discord, Jira, etc.)
- Audio capture during recording
- Webcam overlay
- Annotations during recording
- Streaming (RTMP)
- Linux support (can be added later — Tauri supports it)
- Built-in editor window (overlay-only editing)
