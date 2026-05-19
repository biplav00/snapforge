# Platform: macOS

Every macOS-specific touch point. If/when porting to Linux/Windows, this file becomes `platform-{os}.md`.

## Frameworks linked

From `qt/CMakeLists.txt`:

| Framework | Used by |
|-----------|---------|
| `ScreenCaptureKit` | `snapforge-core/src/capture/macos.rs` |
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

Registered in `qt/src/main.cpp` via Carbon `RegisterEventHotKey`:

| Hotkey | Virtual key | Action |
|--------|-------------|--------|
| Cmd+Shift+S | `0x01` | Region screenshot (opens OverlayWindow) |
| Cmd+Shift+R | `0x0F` | Toggle recording |
| Cmd+Shift+H | `0x04` | Show history |
| Cmd+Shift+F | `0x03` | Fullscreen capture |

Carbon hotkeys do **not** require Accessibility — they go through a private system path. This is intentional; we don't want to ask for that grant just for hotkeys.

## Click event tap (two implementations — duplication)

| Impl | File | Threading | Used by |
|------|------|-----------|---------|
| Rust | `crates/snapforge-core/src/clicks.rs::macos_tap` | Dedicated CFRunLoop thread spawned by `start()` | (currently unused) |
| C++ | `qt/src/ClickEventTap.mm` | Main CFRunLoop, dispatches to main queue | Click visualizer (`ClickIndicatorOverlay`) |

Both call `CGEventTapCreate(kCGSessionEventTap | kCGHIDEventTap, ..., listenOnly)` on `kCGEventLeftMouseDown | kCGEventRightMouseDown`. Both handle tap-disable re-enable on timeout / permission revoke.

**To consolidate**: route the click visualizer through the Rust impl via a new FFI (e.g. `snapforge_clicks_start(callback)`), delete `ClickEventTap.{h,mm}`. See [future-direction.md](future-direction.md).

## ScreenCaptureKit

`crates/snapforge-core/src/capture/macos.rs` wraps SCK for both still capture and continuous recording frames.

- Fallback path: `CGDisplayCreateImage` for fullscreen captures of full-screen apps (commit `6982101`). SCK sometimes misses these.
- xcap-based fallback exists in `capture/xcap_impl.rs` for non-macOS or feature-flagged builds.

## Recording → ffmpeg child process

- Binary location at runtime: `Snapforge.app/Contents/MacOS/ffmpeg-aarch64-apple-darwin` (single arm64 build).
- Dylibs at runtime: `Snapforge.app/Contents/Frameworks/lib{avcodec,avformat,avutil,swresample,swscale,avfilter,avdevice,x264,x265,vpx,dav1d,SvtAv1Enc,webp,mp3lame,opus}.*.dylib`.
- `qt/scripts/bundle-ffmpeg.sh` (POST_BUILD step) copies binaries from Homebrew, rewrites `LC_LOAD_DYLIB` entries from `/opt/homebrew/...` → `@executable_path/../Frameworks/...`, then re-ad-hoc-signs.
- `find_ffmpeg()` in `record/mod.rs` resolves the binary at start time: explicit path → sidecar adjacent to executable → `binaries/` subdir → system PATH. Non-system paths log a warning (defends against config-driven RCE).

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

Both `.mm` files use ARC (set via `set_source_files_properties(... COMPILE_FLAGS "-fobjc-arc")` in CMakeLists). `ClickEventTap.mm` and `ClickIndicatorOverlayMac.mm` do **not** use ARC because they manage CF types manually (CF objects are not ARC-managed regardless).

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
