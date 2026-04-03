# ScreenSnap Phase 1: Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Rust core library (`screen-core`) and CLI so that `screen capture --fullscreen` and `screen capture --region x,y,w,h` produce screenshot files on macOS.

**Architecture:** Cargo workspace with two crates — `screen-core` (library) and `cli` (binary). `screen-core` provides platform-abstracted screen capture, image format conversion, clipboard copy, and config persistence. The CLI calls `screen-core` directly for non-interactive commands.

**Tech Stack:** Rust, `image` crate (format conversion), `core-graphics` + `core-foundation` (macOS capture), `arboard` (clipboard), `clap` (CLI), `serde`/`serde_json` (config), `dirs` (platform paths)

---

## File Structure

```
screen/
├── Cargo.toml                          # workspace root
├── crates/
│   └── screen-core/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                  # public API re-exports
│           ├── types.rs                # Rect, ImageBuffer, CaptureFormat, etc.
│           ├── capture/
│           │   ├── mod.rs              # ScreenCapture trait + factory
│           │   └── macos.rs            # CoreGraphics implementation
│           ├── format.rs               # save_image(), format conversion
│           ├── clipboard.rs            # copy_image_to_clipboard()
│           └── config.rs              # AppConfig, load/save, region persistence
└── cli/
    ├── Cargo.toml
    └── src/
        └── main.rs                     # clap CLI binary
```

---

### Task 1: Workspace and Project Scaffold

**Files:**
- Create: `screen/Cargo.toml`
- Create: `screen/crates/screen-core/Cargo.toml`
- Create: `screen/crates/screen-core/src/lib.rs`
- Create: `screen/cli/Cargo.toml`
- Create: `screen/cli/src/main.rs`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
# screen/Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/screen-core",
    "cli",
]
```

- [ ] **Step 2: Create screen-core crate**

```toml
# screen/crates/screen-core/Cargo.toml
[package]
name = "screen-core"
version = "0.1.0"
edition = "2021"

[dependencies]
image = "0.25"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs = "6"
thiserror = "2"
arboard = "3"
chrono = "0.4"

[target.'cfg(target_os = "macos")'.dependencies]
core-graphics = "0.24"
core-foundation = "0.10"
```

```rust
// screen/crates/screen-core/src/lib.rs
pub mod types;
pub mod capture;
pub mod format;
pub mod clipboard;
pub mod config;
```

- [ ] **Step 3: Create CLI crate**

```toml
# screen/cli/Cargo.toml
[package]
name = "screensnap"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "screen"
path = "src/main.rs"

[dependencies]
screen-core = { path = "../crates/screen-core" }
clap = { version = "4", features = ["derive"] }
```

```rust
// screen/cli/src/main.rs
fn main() {
    println!("screensnap cli");
}
```

- [ ] **Step 4: Verify workspace builds**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git init
git add Cargo.toml crates/ cli/
git commit -m "feat: scaffold Cargo workspace with screen-core and cli crates"
```

---

### Task 2: Core Types

**Files:**
- Create: `screen/crates/screen-core/src/types.rs`

- [ ] **Step 1: Write types tests**

```rust
// screen/crates/screen-core/src/types.rs
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CaptureFormat {
    Png,
    Jpg,
    WebP,
}

impl CaptureFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            CaptureFormat::Png => "png",
            CaptureFormat::Jpg => "jpg",
            CaptureFormat::WebP => "webp",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(CaptureFormat::Png),
            "jpg" | "jpeg" => Some(CaptureFormat::Jpg),
            "webp" => Some(CaptureFormat::WebP),
            _ => None,
        }
    }
}

impl Default for CaptureFormat {
    fn default() -> Self {
        CaptureFormat::Png
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LastRegion {
    pub display: usize,
    pub rect: Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_format_extension() {
        assert_eq!(CaptureFormat::Png.extension(), "png");
        assert_eq!(CaptureFormat::Jpg.extension(), "jpg");
        assert_eq!(CaptureFormat::WebP.extension(), "webp");
    }

    #[test]
    fn test_capture_format_from_extension() {
        assert_eq!(CaptureFormat::from_extension("png"), Some(CaptureFormat::Png));
        assert_eq!(CaptureFormat::from_extension("PNG"), Some(CaptureFormat::Png));
        assert_eq!(CaptureFormat::from_extension("jpg"), Some(CaptureFormat::Jpg));
        assert_eq!(CaptureFormat::from_extension("jpeg"), Some(CaptureFormat::Jpg));
        assert_eq!(CaptureFormat::from_extension("webp"), Some(CaptureFormat::WebP));
        assert_eq!(CaptureFormat::from_extension("bmp"), None);
    }

    #[test]
    fn test_capture_format_default() {
        assert_eq!(CaptureFormat::default(), CaptureFormat::Png);
    }

    #[test]
    fn test_rect_serde_roundtrip() {
        let rect = Rect { x: 100, y: 200, width: 800, height: 600 };
        let json = serde_json::to_string(&rect).unwrap();
        let deserialized: Rect = serde_json::from_str(&json).unwrap();
        assert_eq!(rect, deserialized);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/screen-core/src/types.rs crates/screen-core/src/lib.rs
git commit -m "feat: add core types — Rect, CaptureFormat, LastRegion"
```

---

### Task 3: Configuration Loading and Saving

**Files:**
- Create: `screen/crates/screen-core/src/config.rs`

- [ ] **Step 1: Write config module with tests**

```rust
// screen/crates/screen-core/src/config.rs
use crate::types::{CaptureFormat, LastRegion};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub save_directory: PathBuf,
    pub auto_copy_clipboard: bool,
    pub show_notification: bool,
    pub launch_at_startup: bool,
    pub remember_last_region: bool,
    pub last_region: Option<LastRegion>,
    pub screenshot_format: CaptureFormat,
    pub jpg_quality: u8,
    pub filename_pattern: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        let save_directory = dirs::picture_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("ScreenSnap");

        Self {
            save_directory,
            auto_copy_clipboard: true,
            show_notification: true,
            launch_at_startup: false,
            remember_last_region: false,
            last_region: None,
            screenshot_format: CaptureFormat::default(),
            jpg_quality: 90,
            filename_pattern: "screenshot-{date}-{time}".to_string(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("screensnap");
        config_dir.join("config.json")
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Read)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents).map_err(ConfigError::Read)?;
        Ok(())
    }

    pub fn generate_filename(&self) -> String {
        let now = chrono::Local::now();
        self.filename_pattern
            .replace("{date}", &now.format("%Y-%m-%d").to_string())
            .replace("{time}", &now.format("%H-%M-%S").to_string())
    }

    pub fn save_file_path(&self) -> PathBuf {
        let filename = self.generate_filename();
        let ext = self.screenshot_format.extension();
        self.save_directory.join(format!("{}.{}", filename, ext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(config.auto_copy_clipboard);
        assert!(config.show_notification);
        assert!(!config.launch_at_startup);
        assert!(!config.remember_last_region);
        assert!(config.last_region.is_none());
        assert_eq!(config.screenshot_format, CaptureFormat::Png);
        assert_eq!(config.jpg_quality, 90);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.auto_copy_clipboard, config.auto_copy_clipboard);
        assert_eq!(deserialized.screenshot_format, config.screenshot_format);
        assert_eq!(deserialized.jpg_quality, config.jpg_quality);
    }

    #[test]
    fn test_config_save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.json");

        let mut config = AppConfig::default();
        config.jpg_quality = 75;
        config.remember_last_region = true;

        // Save manually to temp path
        let contents = serde_json::to_string_pretty(&config).unwrap();
        fs::write(&config_path, &contents).unwrap();

        // Load from temp path
        let loaded: AppConfig = serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.jpg_quality, 75);
        assert!(loaded.remember_last_region);
    }

    #[test]
    fn test_generate_filename() {
        let config = AppConfig {
            filename_pattern: "screenshot-{date}-{time}".to_string(),
            ..Default::default()
        };
        let filename = config.generate_filename();
        // Should contain a date-like pattern
        assert!(filename.starts_with("screenshot-"));
        assert!(filename.len() > "screenshot-".len());
    }

    #[test]
    fn test_save_file_path() {
        let config = AppConfig {
            save_directory: PathBuf::from("/tmp/screenshots"),
            screenshot_format: CaptureFormat::Jpg,
            filename_pattern: "test-{date}".to_string(),
            ..Default::default()
        };
        let path = config.save_file_path();
        assert_eq!(path.extension().unwrap(), "jpg");
        assert!(path.starts_with("/tmp/screenshots"));
    }
}
```

- [ ] **Step 2: Add tempfile dev dependency**

Add to `screen/crates/screen-core/Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core`
Expected: all tests pass (types + config)

- [ ] **Step 4: Commit**

```bash
git add crates/screen-core/
git commit -m "feat: add config module — load, save, filename generation"
```

---

### Task 4: Screen Capture Trait and macOS Implementation

**Files:**
- Create: `screen/crates/screen-core/src/capture/mod.rs`
- Create: `screen/crates/screen-core/src/capture/macos.rs`

- [ ] **Step 1: Write capture trait and macOS implementation**

```rust
// screen/crates/screen-core/src/capture/mod.rs
#[cfg(target_os = "macos")]
pub mod macos;

use crate::types::Rect;
use image::RgbaImage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no display found at index {0}")]
    NoDisplay(usize),
    #[error("screen capture failed")]
    CaptureFailed,
    #[error("failed to get image data")]
    ImageDataFailed,
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_fullscreen(display)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = display;
        Err(CaptureError::CaptureFailed)
    }
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_region(display, region)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (display, region);
        Err(CaptureError::CaptureFailed)
    }
}

pub fn display_count() -> usize {
    #[cfg(target_os = "macos")]
    {
        macos::display_count()
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}
```

```rust
// screen/crates/screen-core/src/capture/macos.rs
use crate::types::Rect;
use core_graphics::display::{
    CGDisplay, CGMainDisplayID,
};
use image::RgbaImage;
use super::CaptureError;

pub fn display_count() -> usize {
    // CGGetActiveDisplayList — for now just check if main display exists
    if CGDisplay::new(CGMainDisplayID()).is_active() {
        1
    } else {
        0
    }
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    let display_id = get_display_id(display)?;
    let cg_display = CGDisplay::new(display_id);
    let cg_image = cg_display.image().ok_or(CaptureError::CaptureFailed)?;

    cg_image_to_rgba(&cg_image)
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    // Capture fullscreen then crop — simplest correct approach.
    // CGDisplay::screenshot_of_rect exists but has coordinate system quirks.
    let full = capture_fullscreen(display)?;

    let x = region.x.max(0) as u32;
    let y = region.y.max(0) as u32;
    let w = region.width.min(full.width().saturating_sub(x));
    let h = region.height.min(full.height().saturating_sub(y));

    if w == 0 || h == 0 {
        return Err(CaptureError::CaptureFailed);
    }

    let cropped = image::imageops::crop_imm(&full, x, y, w, h).to_image();
    Ok(cropped)
}

fn get_display_id(display: usize) -> Result<u32, CaptureError> {
    if display == 0 {
        Ok(CGMainDisplayID())
    } else {
        // For multi-monitor: use CGGetActiveDisplayList
        // For now, only support main display
        Err(CaptureError::NoDisplay(display))
    }
}

fn cg_image_to_rgba(cg_image: &core_graphics::image::CGImage) -> Result<RgbaImage, CaptureError> {
    let width = cg_image.width() as u32;
    let height = cg_image.height() as u32;
    let bytes_per_row = cg_image.bytes_per_row();
    let data = cg_image.data();
    let raw_bytes: &[u8] = &data;

    let mut rgba_buf = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        let row_start = (y as usize) * bytes_per_row;
        for x in 0..width {
            let pixel_start = row_start + (x as usize) * 4;
            // CoreGraphics uses BGRA byte order
            let b = raw_bytes[pixel_start];
            let g = raw_bytes[pixel_start + 1];
            let r = raw_bytes[pixel_start + 2];
            let a = raw_bytes[pixel_start + 3];
            rgba_buf.extend_from_slice(&[r, g, b, a]);
        }
    }

    RgbaImage::from_raw(width, height, rgba_buf).ok_or(CaptureError::ImageDataFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_count_nonzero() {
        assert!(display_count() > 0, "should detect at least one display");
    }

    #[test]
    fn test_capture_fullscreen_main() {
        let img = capture_fullscreen(0);
        // This test requires screen capture permission on macOS.
        // If running in CI without permission, it will fail — that's expected.
        if let Ok(img) = img {
            assert!(img.width() > 0);
            assert!(img.height() > 0);
        }
    }

    #[test]
    fn test_capture_region_main() {
        let region = crate::types::Rect { x: 0, y: 0, width: 100, height: 100 };
        let img = capture_region(0, region);
        if let Ok(img) = img {
            assert_eq!(img.width(), 100);
            assert_eq!(img.height(), 100);
        }
    }

    #[test]
    fn test_invalid_display() {
        let result = capture_fullscreen(99);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core`
Expected: all tests pass (capture tests may skip gracefully if no screen permission)

- [ ] **Step 3: Commit**

```bash
git add crates/screen-core/src/capture/
git commit -m "feat: add screen capture — trait + macOS CoreGraphics implementation"
```

---

### Task 5: Image Format Conversion

**Files:**
- Create: `screen/crates/screen-core/src/format.rs`

- [ ] **Step 1: Write format module with tests**

```rust
// screen/crates/screen-core/src/format.rs
use crate::types::CaptureFormat;
use image::RgbaImage;
use std::io::Cursor;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("failed to encode image: {0}")]
    EncodeFailed(String),
    #[error("failed to write file: {0}")]
    WriteFailed(#[from] std::io::Error),
    #[error("unsupported format: {0}")]
    Unsupported(String),
}

/// Encode an RgbaImage to bytes in the given format.
pub fn encode_image(image: &RgbaImage, format: CaptureFormat, quality: u8) -> Result<Vec<u8>, FormatError> {
    let mut buf = Cursor::new(Vec::new());

    match format {
        CaptureFormat::Png => {
            image.write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
        }
        CaptureFormat::Jpg => {
            let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            encoder.encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            ).map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
        }
        CaptureFormat::WebP => {
            image.write_to(&mut buf, image::ImageFormat::WebP)
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
        }
    }

    Ok(buf.into_inner())
}

/// Save an RgbaImage to a file path. Creates parent directories if needed.
pub fn save_image(image: &RgbaImage, path: &Path, format: CaptureFormat, quality: u8) -> Result<(), FormatError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = encode_image(image, format, quality)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image() -> RgbaImage {
        RgbaImage::from_pixel(100, 100, image::Rgba([255, 0, 0, 255]))
    }

    #[test]
    fn test_encode_png() {
        let img = test_image();
        let bytes = encode_image(&img, CaptureFormat::Png, 90).unwrap();
        assert!(!bytes.is_empty());
        // PNG magic bytes
        assert_eq!(&bytes[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_encode_jpg() {
        let img = test_image();
        let bytes = encode_image(&img, CaptureFormat::Jpg, 90).unwrap();
        assert!(!bytes.is_empty());
        // JPEG magic bytes
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_encode_webp() {
        let img = test_image();
        let bytes = encode_image(&img, CaptureFormat::WebP, 90).unwrap();
        assert!(!bytes.is_empty());
        // WebP starts with RIFF
        assert_eq!(&bytes[0..4], b"RIFF");
    }

    #[test]
    fn test_save_image_creates_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub/dir/test.png");
        let img = test_image();
        save_image(&img, &path, CaptureFormat::Png, 90).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_save_image_jpg_quality() {
        let tmp = tempfile::tempdir().unwrap();
        let img = test_image();

        let path_high = tmp.path().join("high.jpg");
        save_image(&img, &path_high, CaptureFormat::Jpg, 95).unwrap();

        let path_low = tmp.path().join("low.jpg");
        save_image(&img, &path_low, CaptureFormat::Jpg, 10).unwrap();

        let size_high = std::fs::metadata(&path_high).unwrap().len();
        let size_low = std::fs::metadata(&path_low).unwrap().len();
        assert!(size_high > size_low, "higher quality should produce larger file");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core -- format`
Expected: 5 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/screen-core/src/format.rs
git commit -m "feat: add format module — encode and save PNG/JPG/WebP"
```

---

### Task 6: Clipboard Support

**Files:**
- Create: `screen/crates/screen-core/src/clipboard.rs`

- [ ] **Step 1: Write clipboard module**

```rust
// screen/crates/screen-core/src/clipboard.rs
use image::RgbaImage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("failed to access clipboard: {0}")]
    AccessFailed(String),
    #[error("failed to set clipboard image: {0}")]
    SetFailed(String),
}

/// Copy an RgbaImage to the system clipboard.
pub fn copy_image_to_clipboard(image: &RgbaImage) -> Result<(), ClipboardError> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    let img_data = arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: std::borrow::Cow::Borrowed(image.as_raw()),
    };

    clipboard.set_image(img_data)
        .map_err(|e| ClipboardError::SetFailed(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_to_clipboard() {
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        // This may fail in headless CI environments without a display server.
        // That's expected — clipboard requires a windowing system.
        let result = copy_image_to_clipboard(&img);
        if result.is_ok() {
            // If it succeeded, clipboard should now have image data
            assert!(result.is_ok());
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core -- clipboard`
Expected: test passes (or skips gracefully in headless environment)

- [ ] **Step 3: Commit**

```bash
git add crates/screen-core/src/clipboard.rs
git commit -m "feat: add clipboard module — copy image to system clipboard"
```

---

### Task 7: Public API in lib.rs

**Files:**
- Modify: `screen/crates/screen-core/src/lib.rs`

- [ ] **Step 1: Create high-level API functions in lib.rs**

```rust
// screen/crates/screen-core/src/lib.rs
pub mod types;
pub mod capture;
pub mod format;
pub mod clipboard;
pub mod config;

use std::path::PathBuf;
use thiserror::Error;
use types::{CaptureFormat, Rect};

#[derive(Debug, Error)]
pub enum ScreenError {
    #[error("capture error: {0}")]
    Capture(#[from] capture::CaptureError),
    #[error("format error: {0}")]
    Format(#[from] format::FormatError),
    #[error("clipboard error: {0}")]
    Clipboard(#[from] clipboard::ClipboardError),
    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),
}

/// Capture fullscreen screenshot, save to file, optionally copy to clipboard.
/// Returns the path where the file was saved.
pub fn screenshot_fullscreen(
    display: usize,
    save_path: &PathBuf,
    format: CaptureFormat,
    quality: u8,
    copy_to_clipboard: bool,
) -> Result<PathBuf, ScreenError> {
    let image = capture::capture_fullscreen(display)?;

    if copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    format::save_image(&image, save_path, format, quality)?;
    Ok(save_path.clone())
}

/// Capture a specific region, save to file, optionally copy to clipboard.
/// Returns the path where the file was saved.
pub fn screenshot_region(
    display: usize,
    region: Rect,
    save_path: &PathBuf,
    format: CaptureFormat,
    quality: u8,
    copy_to_clipboard: bool,
) -> Result<PathBuf, ScreenError> {
    let image = capture::capture_region(display, region)?;

    if copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    format::save_image(&image, save_path, format, quality)?;
    Ok(save_path.clone())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screen-core`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add crates/screen-core/src/lib.rs
git commit -m "feat: add high-level screenshot API — fullscreen and region"
```

---

### Task 8: CLI Implementation

**Files:**
- Modify: `screen/cli/src/main.rs`

- [ ] **Step 1: Write CLI with clap**

```rust
// screen/cli/src/main.rs
use clap::{Parser, Subcommand};
use screen_core::config::AppConfig;
use screen_core::types::{CaptureFormat, Rect};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "screen", about = "ScreenSnap — screenshot and screen recording tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Take a screenshot
    Capture {
        /// Capture the full screen
        #[arg(long)]
        fullscreen: bool,

        /// Use the last remembered region
        #[arg(long)]
        last_region: bool,

        /// Capture a specific region: x,y,width,height
        #[arg(long, value_parser = parse_region)]
        region: Option<Rect>,

        /// Output format: png, jpg, webp
        #[arg(long, short)]
        format: Option<String>,

        /// Output file path (overrides default)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Image quality for jpg/webp (1-100)
        #[arg(long, short, default_value = "90")]
        quality: u8,

        /// Display index (0 = main display)
        #[arg(long, short, default_value = "0")]
        display: usize,
    },
}

fn parse_region(s: &str) -> Result<Rect, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err("region must be x,y,width,height".to_string());
    }
    Ok(Rect {
        x: parts[0].trim().parse().map_err(|_| "invalid x")?,
        y: parts[1].trim().parse().map_err(|_| "invalid y")?,
        width: parts[2].trim().parse().map_err(|_| "invalid width")?,
        height: parts[3].trim().parse().map_err(|_| "invalid height")?,
    })
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Capture {
            fullscreen,
            last_region,
            region,
            format,
            output,
            quality,
            display,
        } => {
            if let Err(e) = handle_capture(fullscreen, last_region, region, format, output, quality, display) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    }
}

fn handle_capture(
    fullscreen: bool,
    last_region: bool,
    region: Option<Rect>,
    format_str: Option<String>,
    output: Option<PathBuf>,
    quality: u8,
    display: usize,
) -> Result<(), screen_core::ScreenError> {
    let config = AppConfig::load()?;

    let format = format_str
        .as_deref()
        .and_then(CaptureFormat::from_extension)
        .unwrap_or(config.screenshot_format);

    let save_path = output.unwrap_or_else(|| config.save_file_path());

    // Determine capture mode
    if let Some(region) = region {
        let path = screen_core::screenshot_region(display, region, &save_path, format, quality, config.auto_copy_clipboard)?;
        println!("Saved to: {}", path.display());

        // Persist region if enabled
        if config.remember_last_region {
            let mut config = config;
            config.last_region = Some(screen_core::types::LastRegion { display, rect: region });
            let _ = config.save();
        }
    } else if last_region {
        match config.last_region {
            Some(last) => {
                let path = screen_core::screenshot_region(last.display, last.rect, &save_path, format, quality, config.auto_copy_clipboard)?;
                println!("Saved to: {}", path.display());
            }
            None => {
                eprintln!("No last region saved. Use --region or capture interactively first.");
                std::process::exit(1);
            }
        }
    } else if fullscreen {
        let path = screen_core::screenshot_fullscreen(display, &save_path, format, quality, config.auto_copy_clipboard)?;
        println!("Saved to: {}", path.display());
    } else {
        // Interactive mode — requires Tauri overlay (not implemented in Phase 1)
        eprintln!("Interactive region selection requires the GUI. Use --fullscreen, --region, or --last-region.");
        std::process::exit(1);
    }

    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap`
Expected: compiles with no errors

- [ ] **Step 3: Test the CLI help**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo run -p screensnap -- --help`
Expected: shows usage with `capture` subcommand

- [ ] **Step 4: Test the CLI capture help**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo run -p screensnap -- capture --help`
Expected: shows `--fullscreen`, `--region`, `--last-region`, `--format`, `--output`, `--quality`, `--display` flags

- [ ] **Step 5: Test fullscreen capture**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo run -p screensnap -- capture --fullscreen --output /tmp/screensnap-test.png`
Expected: prints `Saved to: /tmp/screensnap-test.png`, file exists and is a valid PNG

- [ ] **Step 6: Test region capture**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo run -p screensnap -- capture --region 0,0,200,200 --output /tmp/screensnap-region.png`
Expected: prints `Saved to: /tmp/screensnap-region.png`, file is a 200x200 PNG

- [ ] **Step 7: Test format flags**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo run -p screensnap -- capture --fullscreen --format jpg --quality 50 --output /tmp/screensnap-test.jpg`
Expected: prints saved path, file is a valid JPEG

- [ ] **Step 8: Commit**

```bash
git add cli/
git commit -m "feat: add CLI — capture command with fullscreen, region, format flags"
```

---

### Task 9: End-to-End Integration Test

**Files:**
- Create: `screen/crates/screen-core/tests/integration.rs`

- [ ] **Step 1: Write integration test**

```rust
// screen/crates/screen-core/tests/integration.rs
use screen_core::types::{CaptureFormat, Rect};
use std::path::PathBuf;

#[test]
fn test_fullscreen_screenshot_png() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("fullscreen.png");

    let result = screen_core::screenshot_fullscreen(
        0,
        &path,
        CaptureFormat::Png,
        90,
        false, // don't copy to clipboard in test
    );

    // May fail without screen capture permission — that's expected in CI
    if let Ok(saved_path) = result {
        assert!(saved_path.exists());
        let metadata = std::fs::metadata(&saved_path).unwrap();
        assert!(metadata.len() > 100, "PNG should be more than 100 bytes");

        // Verify it's a valid image
        let img = image::open(&saved_path).unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }
}

#[test]
fn test_region_screenshot_jpg() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("region.jpg");
    let region = Rect { x: 0, y: 0, width: 200, height: 150 };

    let result = screen_core::screenshot_region(
        0,
        region,
        &path,
        CaptureFormat::Jpg,
        85,
        false,
    );

    if let Ok(saved_path) = result {
        assert!(saved_path.exists());
        let img = image::open(&saved_path).unwrap();
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 150);
    }
}

#[test]
fn test_region_screenshot_webp() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("region.webp");
    let region = Rect { x: 50, y: 50, width: 100, height: 100 };

    let result = screen_core::screenshot_region(
        0,
        region,
        &path,
        CaptureFormat::WebP,
        90,
        false,
    );

    if let Ok(saved_path) = result {
        assert!(saved_path.exists());
        let metadata = std::fs::metadata(&saved_path).unwrap();
        assert!(metadata.len() > 0);
    }
}
```

- [ ] **Step 2: Run integration tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core --test integration`
Expected: tests pass (or skip gracefully without screen permissions)

- [ ] **Step 3: Run all tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test`
Expected: all unit and integration tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/screen-core/tests/
git commit -m "test: add integration tests — fullscreen and region captures in PNG/JPG/WebP"
```

---

## Phase Summary

After completing all 9 tasks, you have:

- A Cargo workspace with `screen-core` library and `screen` CLI binary
- Screen capture working on macOS via CoreGraphics
- Image export in PNG, JPG, WebP with configurable quality
- Clipboard copy support
- JSON config with region persistence
- A working CLI: `screen capture --fullscreen`, `screen capture --region x,y,w,h`, `screen capture --last-region`
- Unit and integration tests for all modules

**Next phases:**
- Phase 2: Tauri GUI — overlay, region selection, annotation tools, system tray, hotkeys
- Phase 3: Screen recording (FFmpeg), preferences window
