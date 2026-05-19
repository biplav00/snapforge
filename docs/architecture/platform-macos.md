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

- `CGEventTapCreate(kCGSessionEventTap | kCGHIDEventTap, ..., listenOnly)` on `kCGEventLeftMouseDown | kCGEventRightMouseDown`.
- Handles tap-disable re-enable on timeout / permission revoke.
- Runs on a dedicated CFRunLoop thread spawned by `start()`.
- Records `ClickEvent { x, y, right_click: bool, ... }` into a shared tracker.

`crates/snapforge-app/src/clicks.rs::start_click_tracking` adds a forwarder thread that polls the tracker every ~16ms (~60Hz) and invokes the user-supplied C callback. Exposed to Qt via `snapforge_clicks_start/stop/free_handle`.

Qt-side wrapper: `qt/src/controllers/ClickTap.{h,cpp}` — a thin platform-agnostic `QObject` that re-dispatches the Rust callback onto the Qt main thread via `QMetaObject::invokeMethod` before emitting `clicked(QPoint, bool rightClick)`. The old `qt/src/ClickEventTap.{h,mm}` (and its later `qt/src/platform/macos/` copy) was deleted in Phase 2C.

## ScreenCaptureKit

`crates/snapforge-capture/src/capture/macos.rs` wraps SCK for both still capture and continuous recording frames.

- Fallback path: `CGDisplayCreateImage` for fullscreen captures of full-screen apps (commit `6982101`). SCK sometimes misses these.
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
