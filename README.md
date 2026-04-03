# Snapforge

A fast, lightweight screenshot and screen recording tool with annotation support. Built with Rust, Tauri v2, and Svelte.

## Features

- **Screenshot capture** — Lightshot-style transparent overlay for region selection
- **11 annotation tools** — Arrow, Rectangle, Circle, Line, Freehand, Text, Highlight, Blur, Step Numbers, Color Picker, Measurement
- **Screen recording** — MP4 (H.264) and GIF output via FFmpeg
- **System tray** — Lives in the menu bar, no dock icon
- **Global hotkeys** — Customizable keyboard shortcuts
- **Preferences** — Format, quality, hotkeys, recording settings with dark mode support
- **Clipboard support** — Copy annotated screenshots directly

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (20+)
- [Cargo Tauri CLI](https://v2.tauri.app/start/): `cargo install tauri-cli --version "^2"`
- [FFmpeg](https://ffmpeg.org/) (for screen recording): `brew install ffmpeg`

### Development

```bash
# Install dependencies
npm install

# Run in development mode
cargo tauri dev
```

### Build

```bash
# Build the app
cargo tauri build
```

## CLI Usage

```bash
# Screenshot
snapforge capture --fullscreen
snapforge capture --region 100,200,800,600
snapforge capture --last-region

# Screen recording
snapforge record --fullscreen
snapforge record --region 0,0,1920,1080 --format gif --fps 15
```

## Keyboard Shortcuts

### Global (system-wide)

| Action | Default |
|--------|---------|
| Screenshot | `Cmd+Shift+S` |
| Capture Last Region | `Cmd+Shift+L` |
| Record Screen | `Cmd+Shift+R` |

### Annotation Tools

| Tool | Key |
|------|-----|
| Arrow | `A` |
| Rectangle | `R` |
| Circle | `C` |
| Line | `L` |
| Freehand | `F` |
| Text | `T` |
| Highlight | `H` |
| Blur | `B` |
| Step Numbers | `N` |
| Color Picker | `I` |
| Measurement | `M` |

### Overlay Actions

| Action | Key |
|--------|-----|
| Save | `Cmd+S` or `Enter` |
| Copy | `Cmd+C` |
| Undo | `Cmd+Z` |
| Redo | `Cmd+Shift+Z` |
| Cancel | `Escape` |

All shortcuts are customizable via Preferences.

## Tech Stack

- **Core:** Rust (`snapforge-core`)
- **GUI:** Tauri v2
- **Frontend:** Svelte 5, TypeScript, Vite
- **Recording:** FFmpeg (piped via stdin)
- **Platforms:** macOS, Windows, Linux

## Project Structure

```
├── crates/snapforge-core/    # Core library (capture, format, clipboard, config, recording)
├── cli/                      # CLI binary
├── src-tauri/                # Tauri app (tray, hotkeys, commands)
├── src/                      # Svelte frontend
│   ├── lib/overlay/          # Screen overlay + region selector
│   ├── lib/annotation/       # Annotation tools + canvas
│   ├── lib/preferences/      # Settings UI
│   └── lib/recording/        # Recording indicator
└── scripts/                  # Build scripts
```

## License

ISC
