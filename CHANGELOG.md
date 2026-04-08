# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-12-01

### Added

- Screen capture with macOS CoreGraphics implementation
- Interactive region selection via Tauri overlay window
- Fullscreen and region screenshot modes
- Screen recording with FFmpeg frame piping (start/stop API)
- Recording indicator UI with red dot, timer, and stop button
- CLI support for screenshots and recording (`--fullscreen`, `--region`)
- Annotation toolkit with multiple tools:
  - Arrow, rectangle, line, and freehand drawing
  - Text annotations
  - Circle, highlight, and step number tools
  - Blur/pixelate and measurement tools
  - Color picker (eyedropper)
- Floating annotation toolbar with color, size, undo/redo, and action controls
- Annotation compositing for save and clipboard copy
- System tray integration with menu actions
- Global hotkeys for screenshot and last-region capture
- Preferences UI with General, Hotkeys, Screenshots, and Recording tabs
- Configurable hotkey bindings via AppConfig
- Custom UI controls (toggle switches, radio dots, styled slider)
- Custom app icon (viewfinder with crosshair and snap bolt)
- Transparent template tray icon for macOS light/dark themes
- Edge-to-edge overlay without macOS fullscreen Space
- Lightshot-style transparent overlay for region selection
- Configuration module with load, save, and filename generation
- Support for multiple capture formats and clipboard output
- Cross-platform Cargo workspace with `screen-core` and `cli` crates
- Svelte + Vite frontend scaffolding for Tauri
- CI/CD pipeline with Biome linting, semantic release, and Dependabot
- Husky pre-commit hooks with lint-staged
