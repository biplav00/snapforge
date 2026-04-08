# Snapforge

Fast, lightweight screenshot and screen recording tool with annotation support.

## Features

- **Screenshot with region selection** -- Lightshot/Flameshot-style transparent overlay for precise area capture
- **12 annotation tools** -- Arrow, Rectangle, Circle, Line, Dotted Line, Freehand, Text, Highlight, Blur, Step Numbers, Color Picker, Measure
- **Draggable annotation toolbar** -- Reposition the toolbar anywhere on screen
- **Screen recording** -- Record to MP4 (H.264) or GIF via FFmpeg
- **System tray integration** -- Runs in the menu bar with no dock icon on macOS
- **Customizable global hotkeys** -- Set your own shortcuts via Preferences
- **Preferences UI with dark mode** -- Configure format, quality, hotkeys, and recording settings
- **Auto-copy to clipboard** -- Annotated screenshots copied automatically
- **Remember last region** -- Re-capture the same area instantly
- **Cross-platform** -- macOS, Windows, Linux
- **Auto-update support** -- Built-in updater via Tauri plugin
- **CLI for scripted captures** -- Automate screenshots and recordings from the terminal

## Tech Stack

| Layer | Technology |
|-------|------------|
| Core | Rust (`snapforge-core`) |
| GUI | Tauri v2 |
| Frontend | Svelte 5, TypeScript, Vite |
| Recording | FFmpeg (piped via stdin) |
| Linting | Biome |
| Testing | Vitest + Cargo test |

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (20+)
- [Cargo Tauri CLI](https://v2.tauri.app/start/): `cargo install tauri-cli --version "^2"`
- [FFmpeg](https://ffmpeg.org/) (for screen recording): `brew install ffmpeg`

### Build from Source

```bash
npm install
cargo tauri build
```

The built application will be in `src-tauri/target/release`.

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
| Save | `Cmd+S` / `Enter` |
| Copy | `Cmd+C` |
| Undo | `Cmd+Z` |
| Redo | `Cmd+Shift+Z` |
| Cancel | `Escape` |

All shortcuts are customizable via Preferences.

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

## Development

```bash
# Install dependencies
npm install

# Run in development mode
cargo tauri dev

# Run frontend dev server only
npm run dev

# Lint
npm run lint

# Auto-fix lint issues
npm run lint:fix

# Format code
npm run format

# Type-check
npm run typecheck

# Run frontend tests
npm run test

# Run all tests (frontend + Rust)
npm run test:all

# Test coverage
npm run test:coverage
npm run test:coverage:rust

# Full check (lint + typecheck)
npm run check
```

## License

MIT
