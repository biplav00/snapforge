# ScreenCaptureKit Migration — Design Spec

**Date:** 2026-04-08
**Scope:** macOS screenshot capture only (recording stays on CG+FFmpeg)
**Target:** macOS 13+

## Problem

The current capture implementation uses `CGWindowListCreateImage` (CoreGraphics), which is:
- Slow (~100-200ms per capture, synchronous CPU compositing)
- Legacy — Apple recommends ScreenCaptureKit as the replacement
- Previously used `CGDisplayCreateImage` which only captured wallpaper (fixed but symptomatic of CG limitations)

## Solution

Replace CoreGraphics capture with ScreenCaptureKit (SCK) in `macos.rs` using raw `objc2` FFI. SCK provides hardware-accelerated GPU capture at ~10-30ms.

## Architecture

### Public API (unchanged)

The `capture/mod.rs` interface stays identical. No changes to `commands.rs`, `main.rs`, frontend, or other platforms:

```rust
capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError>
capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError>
has_permission() -> bool
request_permission() -> bool
display_count() -> usize
```

### Platform dispatch (unchanged)

- macOS → `macos.rs` (SCK, rewritten)
- Windows/Linux → `xcap_impl.rs` (unchanged)

### SCK Capture Flow

1. `SCShareableContent::getCurrentContent()` — enumerate available displays
2. Create `SCContentFilter` for the target display (all on-screen windows)
3. `SCScreenshotManager.captureImage(filter, config)` — single async call → `CGImage`
4. Convert `CGImage` → `RgbaImage` via existing `cg_image_to_rgba()`

### Async Handling

`SCScreenshotManager.captureImage()` uses an Objective-C completion handler. Block the calling Rust thread with `std::sync::mpsc::channel` to keep the public API synchronous.

### Permission Handling

- `has_screen_capture_permission()` — try `SCShareableContent::getCurrentContent()`. If it returns content, permission is granted. Fall back to `CGPreflightScreenCaptureAccess` as secondary check.
- `request_screen_capture_permission()` — stays as `CGRequestScreenCaptureAccess()` (SCK has no equivalent).
- Cache the result in a static `AtomicBool` after first success. Reset only if a capture fails.

### File Changes

**Modified:**
- `crates/snapforge-core/src/capture/macos.rs` — rewrite to use SCK via objc2 FFI. Remove CoreGraphics capture code. Keep `cg_image_to_rgba()` (SCK returns CGImage).
- `crates/snapforge-core/Cargo.toml` — add `objc2-screen-capture-kit` dependency.
- `src-tauri/src/main.rs` — remove `prewarm_overlay()` function and its call in `setup()`. Cache permission check result.

**Unchanged:**
- `crates/snapforge-core/src/capture/mod.rs` — public API identical
- `crates/snapforge-core/src/capture/xcap_impl.rs` — other platforms untouched
- `src-tauri/src/commands.rs` — no changes
- All frontend code — no changes

### Dependencies

**Add:**
- `objc2-screen-capture-kit` — SCK framework bindings (part of the objc2 ecosystem, same as existing `objc2-app-kit` and `objc2-foundation`)

**Keep:**
- `core-graphics = "0.25"` — still needed for `CGImage` type and `cg_image_to_rgba()`
- `objc2`, `objc2-foundation`, `objc2-app-kit` — already present

**No CoreGraphics fallback** — macOS 13+ only, CG capture code removed.

## Medium-Impact Quick Wins (included)

### 1. Cache permission check
After first successful check, store result in `static AtomicBool`. Skip `SCShareableContent` query on subsequent calls. Reset if capture fails.

### 2. Remove prewarm overlay
Delete `prewarm_overlay()` from `main.rs` and its invocation in `setup()`. With SCK's ~20ms capture, the overall trigger-to-overlay time is fast enough without this 500ms hack.

## Out of Scope

- Recording pipeline (stays on CG frames → FFmpeg)
- Windows/Linux capture (stays on xcap)
- Frontend changes
- Overlay-first-then-capture architecture (may revisit later)
