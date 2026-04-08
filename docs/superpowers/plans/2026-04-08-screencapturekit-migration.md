# ScreenCaptureKit Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace CoreGraphics screen capture with ScreenCaptureKit for ~5-10x faster screenshots on macOS 13+.

**Architecture:** Rewrite `macos.rs` to use SCK via `objc2-screen-capture-kit` FFI bindings. Keep the same public API surface so nothing above the capture layer changes. Add permission caching and remove the prewarm overlay hack.

**Tech Stack:** Rust, objc2-screen-capture-kit 0.3, ScreenCaptureKit (macOS 13+), existing core-graphics for CGImage conversion.

**Spec:** `docs/superpowers/specs/2026-04-08-screencapturekit-migration-design.md`

---

### File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/snapforge-core/Cargo.toml` | Modify | Add `objc2-screen-capture-kit` dependency |
| `crates/snapforge-core/src/capture/macos.rs` | Rewrite | SCK-based capture, permission checks, display enumeration |
| `src-tauri/src/main.rs` | Modify | Remove `prewarm_overlay()`, add permission caching |

---

### Task 1: Add objc2-screen-capture-kit dependency

**Files:**
- Modify: `crates/snapforge-core/Cargo.toml:15-19`

- [ ] **Step 1: Add the dependency**

In `crates/snapforge-core/Cargo.toml`, replace the `[target.'cfg(target_os = "macos")'.dependencies]` section with:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
core-graphics = "0.25"
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = ["NSData", "NSString", "NSArray", "NSError"] }
objc2-app-kit = { version = "0.3", features = ["NSPasteboard"] }
objc2-screen-capture-kit = { version = "0.3", features = [
    "SCShareableContent",
    "SCStream",
    "SCScreenshotManager",
    "block2",
    "objc2-core-graphics",
    "objc2-core-media",
    "objc2-core-foundation",
] }
block2 = "0.6"
```

Note: `NSArray` and `NSError` features added to `objc2-foundation` because SCK completion handlers return `NSArray<SCDisplay>` and `NSError`.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p snapforge-core 2>&1`
Expected: Compiles successfully (macos.rs still uses old CG code, which is fine — we haven't changed it yet).

- [ ] **Step 3: Commit**

```bash
git add crates/snapforge-core/Cargo.toml
git commit -m "deps: add objc2-screen-capture-kit for SCK capture migration"
```

---

### Task 2: Rewrite macos.rs — SCK display enumeration and permission check

**Files:**
- Modify: `crates/snapforge-core/src/capture/macos.rs`

- [ ] **Step 1: Write the new macos.rs with SCK display enumeration and permission**

Replace the entire contents of `crates/snapforge-core/src/capture/macos.rs` with:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use image::RgbaImage;
use objc2::rc::Retained;
use objc2_foundation::NSArray;
use objc2_screen_capture_kit::SCDisplay;
use objc2_screen_capture_kit::SCShareableContent;

use crate::types::Rect;

use super::CaptureError;

extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// Cached permission state — once granted, skip re-checking.
static PERMISSION_GRANTED: AtomicBool = AtomicBool::new(false);

/// Check if screen recording permission is granted.
/// Uses cached result after first success, falls back to SCK content query.
pub fn has_screen_capture_permission() -> bool {
    if PERMISSION_GRANTED.load(Ordering::Relaxed) {
        return true;
    }
    if unsafe { CGPreflightScreenCaptureAccess() } {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
        return true;
    }
    // Preflight can return stale false. Try fetching SCK content as proof of permission.
    if get_shareable_displays().is_some() {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
        return true;
    }
    false
}

/// Request screen recording permission. Returns true if granted.
pub fn request_screen_capture_permission() -> bool {
    let result = unsafe { CGRequestScreenCaptureAccess() };
    if result {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
    }
    result
}

/// Fetch available displays via SCShareableContent (async → sync bridge).
fn get_shareable_displays() -> Option<Retained<NSArray<SCDisplay>>> {
    let (tx, rx) = mpsc::channel();
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            true,
            true,
            &block2::RcBlock::new(move |content: *mut SCShareableContent, error: *mut objc2_foundation::NSError| {
                if error.is_null() && !content.is_null() {
                    let content = unsafe { &*content };
                    let displays = content.displays();
                    let _ = tx.send(Some(displays));
                } else {
                    let _ = tx.send(None);
                }
            }),
        );
    }
    rx.recv().ok().flatten()
}

/// Get the SCDisplay at the given index.
fn get_sc_display(display: usize) -> Result<Retained<SCDisplay>, CaptureError> {
    let displays = get_shareable_displays().ok_or(CaptureError::CaptureFailed)?;
    if display >= displays.len() {
        return Err(CaptureError::NoDisplay(display));
    }
    Ok(displays.objectAtIndex(display))
}

pub fn display_count() -> usize {
    get_shareable_displays().map_or(0, |d| d.len())
}

pub fn capture_fullscreen(_display: usize) -> Result<RgbaImage, CaptureError> {
    // Placeholder — implemented in Task 3
    Err(CaptureError::CaptureFailed)
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
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

fn cg_image_to_rgba(cg_image: &core_graphics::image::CGImage) -> Result<RgbaImage, CaptureError> {
    let width = cg_image.width() as u32;
    let height = cg_image.height() as u32;
    let bytes_per_row = cg_image.bytes_per_row();
    let data = cg_image.data();
    let raw_bytes: &[u8] = &data;

    let expected_pixels = (width * height) as usize;
    let expected_bytes = expected_pixels * 4;

    let last_row_start = (height as usize - 1) * bytes_per_row;
    let min_required = last_row_start + (width as usize) * 4;
    if raw_bytes.len() < min_required {
        return Err(CaptureError::ImageDataFailed);
    }

    let mut rgba_buf = vec![0u8; expected_bytes];

    for y in 0..height as usize {
        let row_start = y * bytes_per_row;
        let dst_row_start = y * (width as usize) * 4;
        let src_row = &raw_bytes[row_start..row_start + (width as usize) * 4];
        let dst_row = &mut rgba_buf[dst_row_start..dst_row_start + (width as usize) * 4];

        // Swap B and R channels: BGRA → RGBA
        for (src, dst) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
            dst[0] = src[2]; // R
            dst[1] = src[1]; // G
            dst[2] = src[0]; // B
            dst[3] = src[3]; // A
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
    fn test_permission_cache() {
        // Reset cache
        PERMISSION_GRANTED.store(false, Ordering::Relaxed);
        let first = has_screen_capture_permission();
        if first {
            // Should be cached now
            assert!(PERMISSION_GRANTED.load(Ordering::Relaxed));
            // Second call should use cache
            assert!(has_screen_capture_permission());
        }
    }

    #[test]
    fn test_invalid_display() {
        let result = capture_fullscreen(99);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p snapforge-core 2>&1`
Expected: Compiles. `capture_fullscreen` returns `CaptureFailed` for now (placeholder).

Note: The `block2::RcBlock::new` callback and `SCShareableContent` method names may need adjustment based on the exact API surface of `objc2-screen-capture-kit 0.3`. If compilation fails on the `getShareableContent...` method name, check the docs with `cargo doc -p objc2-screen-capture-kit --open` and adjust the method name accordingly. The objc2 convention translates Objective-C selectors like `getShareableContentExcludingDesktopWindows:onScreenWindowsOnly:completionHandler:` to underscored Rust names.

- [ ] **Step 3: Run existing tests**

Run: `cargo test -p snapforge-core 2>&1`
Expected: `test_display_count_nonzero` passes (SCK can enumerate displays). `test_invalid_display` passes. Capture tests may fail since `capture_fullscreen` is a placeholder — that's expected.

- [ ] **Step 4: Commit**

```bash
git add crates/snapforge-core/src/capture/macos.rs
git commit -m "feat: rewrite macos capture to SCK — display enum and permission"
```

---

### Task 3: Implement SCK screenshot capture

**Files:**
- Modify: `crates/snapforge-core/src/capture/macos.rs`

- [ ] **Step 1: Replace the placeholder `capture_fullscreen` with SCK capture**

In `macos.rs`, replace the placeholder `capture_fullscreen` function and add the SCK capture helper. Add these imports at the top (merge with existing):

```rust
use objc2_screen_capture_kit::{
    SCContentFilter, SCScreenshotManager, SCStreamConfiguration,
};
```

Then replace the `capture_fullscreen` function:

```rust
pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    let sc_display = get_sc_display(display)?;

    // Create a content filter for this display (all windows)
    let filter = unsafe {
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            &sc_display,
            &NSArray::new(),
        )
    };

    // Configure capture at native resolution
    let config = unsafe {
        let config = SCStreamConfiguration::new();
        let display_id = sc_display.displayID();
        let cg_display = core_graphics::display::CGDisplay::new(display_id);
        config.setWidth(cg_display.pixels_wide() as usize);
        config.setHeight(cg_display.pixels_high() as usize);
        config.setShowsCursor(false);
        config
    };

    // Async capture → sync bridge
    let (tx, rx) = mpsc::channel();
    unsafe {
        SCScreenshotManager::captureSampleBufferWithFilter_configuration_completionHandler(
            &filter,
            &config,
            &block2::RcBlock::new(
                move |sample_buffer: *mut objc2_core_media::CMSampleBuffer,
                      error: *mut objc2_foundation::NSError| {
                    if error.is_null() && !sample_buffer.is_null() {
                        let sb = unsafe { &*sample_buffer };
                        let _ = tx.send(extract_rgba_from_sample_buffer(sb));
                    } else {
                        let _ = tx.send(None);
                    }
                },
            ),
        );
    }

    let result = rx
        .recv()
        .map_err(|_| CaptureError::CaptureFailed)?
        .ok_or(CaptureError::CaptureFailed)?;

    Ok(result)
}
```

- [ ] **Step 2: Add the sample buffer extraction helper**

Add this function to `macos.rs` (after `capture_fullscreen`, before `capture_region`):

```rust
/// Extract RGBA image data from a CMSampleBuffer.
/// SCK provides frames in BGRA format in an IOSurface-backed pixel buffer.
fn extract_rgba_from_sample_buffer(
    sample_buffer: &objc2_core_media::CMSampleBuffer,
) -> Option<RgbaImage> {
    use core_graphics::image::CGImage;

    // Get CGImage from the sample buffer's image buffer
    extern "C" {
        fn CMSampleBufferGetImageBuffer(
            sbuf: *const std::ffi::c_void,
        ) -> *const std::ffi::c_void;
        fn CVPixelBufferLockBaseAddress(
            pixelBuffer: *const std::ffi::c_void,
            lockFlags: u64,
        ) -> i32;
        fn CVPixelBufferUnlockBaseAddress(
            pixelBuffer: *const std::ffi::c_void,
            lockFlags: u64,
        ) -> i32;
        fn CVPixelBufferGetWidth(pixelBuffer: *const std::ffi::c_void) -> usize;
        fn CVPixelBufferGetHeight(pixelBuffer: *const std::ffi::c_void) -> usize;
        fn CVPixelBufferGetBytesPerRow(pixelBuffer: *const std::ffi::c_void) -> usize;
        fn CVPixelBufferGetBaseAddress(
            pixelBuffer: *const std::ffi::c_void,
        ) -> *const u8;
    }

    unsafe {
        let sb_ptr = sample_buffer as *const _ as *const std::ffi::c_void;
        let pixel_buffer = CMSampleBufferGetImageBuffer(sb_ptr);
        if pixel_buffer.is_null() {
            return None;
        }

        CVPixelBufferLockBaseAddress(pixel_buffer, 1); // kCVPixelBufferLock_ReadOnly

        let width = CVPixelBufferGetWidth(pixel_buffer) as u32;
        let height = CVPixelBufferGetHeight(pixel_buffer) as u32;
        let bytes_per_row = CVPixelBufferGetBytesPerRow(pixel_buffer);
        let base = CVPixelBufferGetBaseAddress(pixel_buffer);

        if base.is_null() || width == 0 || height == 0 {
            CVPixelBufferUnlockBaseAddress(pixel_buffer, 1);
            return None;
        }

        let expected_bytes = (width * height * 4) as usize;
        let mut rgba_buf = vec![0u8; expected_bytes];

        for y in 0..height as usize {
            let row_start = y * bytes_per_row;
            let dst_row_start = y * (width as usize) * 4;
            let src_row =
                std::slice::from_raw_parts(base.add(row_start), (width as usize) * 4);
            let dst_row = &mut rgba_buf[dst_row_start..dst_row_start + (width as usize) * 4];

            // BGRA → RGBA
            for (src, dst) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                dst[0] = src[2]; // R
                dst[1] = src[1]; // G
                dst[2] = src[0]; // B
                dst[3] = src[3]; // A
            }
        }

        CVPixelBufferUnlockBaseAddress(pixel_buffer, 1);

        RgbaImage::from_raw(width, height, rgba_buf)
    }
}
```

- [ ] **Step 3: Remove the now-unused `cg_image_to_rgba` function**

The `cg_image_to_rgba` function is no longer called since SCK returns `CMSampleBuffer` instead of `CGImage`. Remove it. Also remove the unused `core_graphics` imports at the top of the file (`CGDisplay`, `CGPoint`, `CGRect`, `CGSize`, and the `window` imports). Keep `core_graphics::display::CGDisplay` since it's still used in `capture_fullscreen` to get pixel dimensions.

Clean up the imports to:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use core_graphics::display::CGDisplay;
use image::RgbaImage;
use objc2::rc::Retained;
use objc2_foundation::NSArray;
use objc2_screen_capture_kit::{
    SCContentFilter, SCDisplay, SCScreenshotManager, SCShareableContent,
    SCStreamConfiguration,
};

use crate::types::Rect;

use super::CaptureError;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p snapforge-core 2>&1`
Expected: Compiles successfully.

If `captureSampleBufferWithFilter_configuration_completionHandler` doesn't exist in the crate, try the alternative CGImage-based method:
- `captureImageWithFilter_configuration_completionHandler` — returns a `CGImage` directly, and we can reuse `cg_image_to_rgba`. This is simpler. If this is available, use it instead and keep `cg_image_to_rgba`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p snapforge-core 2>&1`
Expected: All tests pass, including capture tests (if screen recording permission is granted in the dev environment).

- [ ] **Step 6: Commit**

```bash
git add crates/snapforge-core/src/capture/macos.rs
git commit -m "feat: implement SCK screenshot capture via SCScreenshotManager"
```

---

### Task 4: Remove prewarm overlay

**Files:**
- Modify: `src-tauri/src/main.rs:49-63` (setup block) and `src-tauri/src/main.rs:125-147` (prewarm_overlay function)

- [ ] **Step 1: Remove the `prewarm_overlay` call from setup**

In `src-tauri/src/main.rs`, change the `.setup()` block from:

```rust
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::create_tray(app.handle())?;
            hotkeys::register_hotkeys(app.handle())?;

            // Pre-warm webview to cache JS bundle for instant overlay startup
            prewarm_overlay(app.handle());

            Ok(())
        })
```

to:

```rust
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::create_tray(app.handle())?;
            hotkeys::register_hotkeys(app.handle())?;

            Ok(())
        })
```

- [ ] **Step 2: Remove the `prewarm_overlay` function**

Delete the entire `prewarm_overlay` function (lines 125-147):

```rust
/// Pre-warm the webview by creating and immediately closing a throwaway overlay.
/// This caches the JS bundle so subsequent overlays open much faster.
fn prewarm_overlay(app: &AppHandle) {
    // ... entire function
}
```

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cargo check -p snapforge-app 2>&1 && cargo test 2>&1`
Expected: Compiles, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "perf: remove prewarm overlay hack — SCK capture is fast enough"
```

---

### Task 5: Integration testing and cleanup

**Files:**
- Modify: `crates/snapforge-core/src/capture/macos.rs` (update tests)

- [ ] **Step 1: Update the test suite**

Replace the `#[cfg(test)]` module in `macos.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_count_nonzero() {
        assert!(display_count() > 0, "should detect at least one display");
    }

    #[test]
    fn test_permission_cache() {
        PERMISSION_GRANTED.store(false, Ordering::Relaxed);
        let first = has_screen_capture_permission();
        if first {
            assert!(PERMISSION_GRANTED.load(Ordering::Relaxed));
            assert!(has_screen_capture_permission());
        }
    }

    #[test]
    fn test_capture_fullscreen_main() {
        let img = capture_fullscreen(0);
        if let Ok(img) = img {
            assert!(img.width() > 0);
            assert!(img.height() > 0);
        }
    }

    #[test]
    fn test_capture_region_main() {
        let region = crate::types::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
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

    #[test]
    fn test_get_shareable_displays() {
        let displays = get_shareable_displays();
        // Should return Some if permission is granted
        if let Some(displays) = displays {
            assert!(displays.len() > 0);
        }
    }
}
```

- [ ] **Step 2: Run the full test suite**

Run: `cargo test 2>&1`
Expected: All Rust tests pass.

- [ ] **Step 3: Run frontend tests**

Run: `npm test 2>&1`
Expected: All JS tests pass (no frontend changes were made).

- [ ] **Step 4: Build the app to verify end-to-end**

Run: `cargo build -p snapforge-app 2>&1`
Expected: Builds successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/snapforge-core/src/capture/macos.rs
git commit -m "test: update macOS capture tests for SCK migration"
```

---

### Task 6: Build DMG and manual verification

**Files:** None (build + manual test only)

- [ ] **Step 1: Build the DMG**

Run: `npm run build:dmg 2>&1`
Expected: DMG produced in `src-tauri/target/release/bundle/dmg/`.

- [ ] **Step 2: Manual verification checklist**

Install the DMG and verify:
1. App launches without errors
2. First screenshot trigger shows permission dialog (if not already granted)
3. After granting permission, screenshot captures the actual screen with windows (not wallpaper)
4. Region selection works
5. Screenshot overlay appears quickly (~snappy feel)
6. Multiple consecutive screenshots work without repeated permission prompts
7. Recording still works (CG+FFmpeg pipeline unchanged)
