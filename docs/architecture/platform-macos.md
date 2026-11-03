# Platform: macOS

Every macOS-specific touch point. If/when porting to Linux/Windows, this file becomes `platform-{os}.md`.

## Frameworks linked

From `qt/CMakeLists.txt`:

| Framework | Used by |
|-----------|---------|
| `ScreenCaptureKit` | `crates/snapforge-capture/src/capture/macos.rs` |
| `CoreGraphics` | Capture fallback, click locations |
| `CoreMedia` | SCK frame timing |
| `CoreFoundation` | CFRunLoop for event tap |
| `Foundation` | NSString interop |
| `AppKit` | NSWindow level/space tweaks, NSStatusItem (via Qt), observers |
| `IOKit` | Display info |
| `IOSurface` | Zero-copy frame buffers from SCK |
| `Security` | Keychain (if/when used) |
| `Carbon` | `RegisterEventHotKey` for global hotkeys (Cmd+Shift+S/R/H/F) |

## TCC permissions

| Permission | Plist key | Triggered by | Used for |
|------------|-----------|--------------|----------|
| Screen Recording | `NSScreenCaptureUsageDescription` | First `snapforge_capture_*`, `snapforge_screenshot`, or `snapforge_record_start` call | Capture frames, list windows |
| Input Monitoring | `NSInputMonitoringUsageDescription` | First `CGEventTapCreate` call | Click visualizer + (future) global click logging |

**Pinning grants**: `qt/CMakeLists.txt` ad-hoc-signs the bundle with `--identifier com.snapforge.app`. If this identifier changes, macOS treats it as a different app and the user must re-grant Screen Recording. **Do not change.**

## Global hotkeys

Registered in `qt/src/app/main.cpp` via Carbon `RegisterEventHotKey`:

| Hotkey | Virtual key | Action |
|--------|-------------|--------|
| Cmd+Shift+S | `0x01` | Region screenshot (opens OverlayWindow) |
| Cmd+Shift+R | `0x0F` | Toggle recording |
| Cmd+Shift+H | `0x04` | Show history |
| Cmd+Shift+F | `0x03` | Fullscreen capture |

Carbon hotkeys do **not** require Accessibility — they go through a private system path. This is intentional; we don't want to ask for that grant just for hotkeys.

## Click event tap (single impl, Rust-side)

Phase 2C consolidated the previously-duplicated CGEventTap. The tap now lives in `crates/snapforge-capture/src/clicks.rs::macos_tap`:

- `CGEventTapCreate(kCGHIDEventTap, kCGTailAppendEventTap, kCGEventTapOptionListenOnly, ...)` on `kCGEventLeftMouseDown | kCGEventRightMouseDown`.
- Handles tap-disable re-enable on timeout / permission revoke. Re-enables are rate-limited per burst: at most 5 within a burst of disables, and the budget refills after 60 s without a disable. Isolated timeouts in a long session never exhaust it, while a revoked-permission loop stops after 5 attempts.
- Teardown removes the run-loop source and invalidates the mach port before releasing them.
- Runs on a dedicated CFRunLoop thread spawned by `start()`.
- Records `ClickEvent { x, y, right_click: bool, ... }` into a shared tracker. `x`/`y` are global desktop **points** (CoreGraphics global space), not physical pixels.

`crates/snapforge-app/src/clicks.rs::start_click_tracking` adds a forwarder thread that polls the tracker every ~16ms (~60Hz) and invokes the user-supplied C callback. Exposed to Qt via `snapforge_clicks_start/stop/free_handle`.

Qt-side wrapper: `qt/src/controllers/ClickTap.{h,cpp}` — a thin platform-agnostic `QObject` that re-dispatches the Rust callback onto the Qt main thread via `QMetaObject::invokeMethod` before emitting `clicked(QPoint, bool rightClick)`. The old `qt/src/ClickEventTap.{h,mm}` (and its later `qt/src/platform/macos/` copy) was deleted in Phase 2C.

## ScreenCaptureKit

`crates/snapforge-capture/src/capture/macos.rs` wraps SCK (and the CoreGraphics fallback) for still capture and the recording frame source. The `SCStream` recorder is in `crates/snapforge-capture/src/capture/macos/stream.rs`.

- **Display index**: an index into `CGGetActiveDisplayList` with mirror displays removed. Index 0 is the main (menu-bar) display. This lookup is synchronous, needs no Screen Recording grant, and is never cached, so it can't go stale after a topology change. `display_at_point` maps through `CGGetDisplaysWithPoint` into the same list. The `SCDisplay` object is looked up through `SCShareableContent` only inside the SCK paths. The timeout is 3 s for screenshots and 6 s for recording setup.
- **Screenshots**: SCK `SCScreenshotManager captureImageWithFilter:configuration:completionHandler:` is the primary path. It uses a full-display filter with no excluded windows, so it captures the menu bar, the Dock and every window, the same content the CG path captured. The config asks for native pixel size, BGRA, sRGB and no cursor, and the capture has a 3 s timeout. The deprecated `CGDisplayCreateImage` is only a fallback, used when SCK fails (timeout, error, no Screen Recording grant) or returns an all-black frame. SCK can return black for some fullscreen-Space apps (commit `6982101`). The first non-black image wins. If both images are black, the black image is returned rather than an error, because the screen may really be black. Without the grant (`CGPreflightScreenCaptureAccess` false), SCK is skipped entirely, so no TCC prompt is raised and the CG path behaves as before. Qt shows the region overlay only after the capture completes, so the overlay never appears in the shot.
- **Recording**: `RecordingSource` uses `StreamCapture` (one `SCStream`, frames pushed to an `SCStreamOutput` delegate) and falls back to `CaptureContext` (one `SCScreenshotManager` screenshot per frame) if the stream can't start. Both return `CaptureError::NoDisplay` once the display goes offline (`CGDisplayIsOnline`). See `recording-pipeline.md`.
- **Colour management**: all captured pixels are sRGB. The SCK paths (screenshots, the stream, and the per-frame fallback) set `SCStreamConfiguration.colorSpaceName = kCGColorSpaceSRGB`. A CGImage whose colour space isn't named `kCGColorSpaceSRGB` is redrawn by CoreGraphics into an sRGB bitmap context (`PremultipliedFirst | ByteOrder32Little`, so BGRA in memory, alpha forced to 255) before its bytes are read. This covers `CGDisplayCreateImage` output, which is in the display's profile, often Display P3. Before this change, Display P3 bytes were saved untagged and colours shifted. MP4s are tagged BT.709, whose primaries match sRGB.
- **OS guard**: the minimum OS is macOS 14 (`LSMinimumSystemVersion`). The SCK paths still check `NSProcessInfo isOperatingSystemAtLeastVersion` at runtime: 12.3 for `SCShareableContent`, and 14.0 for `SCScreenshotManager` and `SCStream` capture. On an older system they return `CaptureError::CaptureFailed` instead of hitting objc2's missing-class panic.
- **Frameworks**: the Rust staticlib needs ScreenCaptureKit, CoreGraphics, CoreMedia, **CoreVideo** (the `CVPixelBuffer*` calls in the stream callback), CoreFoundation, Foundation, AppKit, IOKit, IOSurface, Security and Carbon. These are listed in `qt/CMakeLists.txt` for the app and the test targets.
- **Pixel size**: `CGDisplayBounds × (mode pixelWidth / mode width)`, so rotated displays report rotated dimensions.
- **Pixel format**: the CGImage converters read the image's bytes directly only when the image is sRGB and 32 bpp / 8 bpc little-endian alpha-first (BGRA in memory). Any other image, whether it has a different layout or a non-sRGB colour space, goes through the sRGB redraw described above. `NoneSkipFirst` images get alpha forced to 255. The stream path accepts only non-planar `32BGRA` pixel buffers.
- **Permission**: `has_permission` is a live `CGPreflightScreenCaptureAccess` (never cached, never an SCK probe). `request_permission` prompts at most once per process. A session latch stops SCK queries after a known denial and clears when preflight turns true.
- xcap-based fallback exists in `crates/snapforge-capture/src/capture/xcap_impl.rs` for non-macOS or feature-flagged builds.

## Recording → ffmpeg child process

- Binary location at runtime: `Snapforge.app/Contents/MacOS/ffmpeg-aarch64-apple-darwin` (single arm64 build).
- Dylibs at runtime: `Snapforge.app/Contents/Frameworks/lib{avcodec,avformat,avutil,swresample,swscale,avfilter,avdevice,x264,x265,vpx,dav1d,SvtAv1Enc,webp,mp3lame,opus}.*.dylib`.
- `packaging/macos/bundle-ffmpeg.sh` (POST_BUILD step) copies binaries from Homebrew, rewrites `LC_LOAD_DYLIB` entries from `/opt/homebrew/...` → `@executable_path/../Frameworks/...`, then re-ad-hoc-signs.
- `find_ffmpeg()` in `crates/snapforge-encode/src/record/mod.rs` resolves the binary at start time: explicit path → sidecar adjacent to executable → `binaries/` subdir → system PATH. Non-system paths log a warning (defends against config-driven RCE).

## Code signing

- **Ad-hoc only** (`codesign --force --deep --sign - --identifier com.snapforge.app`).
- Pinned identifier keeps TCC grant stable across rebuilds.
- Not notarized — users see Gatekeeper warning. Notarization is a future task.

## Window level / Space behaviour

- **Region picker (`OverlayWindow`)**: standard Qt frameless window, raised to front.
- **Click indicator (`ClickIndicatorOverlay`)**: `NSScreenSaverWindowLevel`, collection behaviour = `CanJoinAllSpaces | Stationary | FullScreenAuxiliary | IgnoresCycle`, `ignoresMouseEvents:YES`. Configured in `ClickIndicatorOverlayMac.mm`.
- **Tray icon (`QSystemTrayIcon`)**: Qt manages NSStatusItem. We set `isMask=false` because the recording pill is multi-coloured (red dot + white text); a mask would collapse it to a single tint.

## Observers

| Observer | Source | Purpose |
|----------|--------|---------|
| `SpaceChangeObserver` | `NSWorkspaceActiveSpaceDidChangeNotification` | Re-show region overlay on the current Space |
| `WorkspaceSleepObserver` | `NSWorkspaceWillSleepNotification` / `DidWakeNotification` | Pause/resume recording cleanly across sleep |

Both observer `.mm` files are built with ARC (set via `set_source_files_properties(... COMPILE_FLAGS "-fobjc-arc")` in `qt/CMakeLists.txt`). `ClickIndicatorOverlayMac.mm` does **not** use ARC because it manages CF types manually (CF objects are not ARC-managed regardless).

## Bundle layout (release)

```
Snapforge.app/
├── Contents/
│   ├── Info.plist                    (from Info.plist.in)
│   ├── MacOS/
│   │   ├── Snapforge                 (main binary)
│   │   └── ffmpeg-aarch64-apple-darwin
│   ├── Frameworks/
│   │   ├── QtCore.framework/         (via macdeployqt)
│   │   ├── QtGui.framework/
│   │   ├── QtWidgets.framework/
│   │   ├── libavcodec.62.*.dylib     (via bundle-ffmpeg.sh)
│   │   ├── libx264.165.dylib
│   │   └── ...
│   └── Resources/
│       └── AppIcon.icns
```
