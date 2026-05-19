# Recording pipeline

End-to-end: from hotkey to file on disk.

## Sequence

```
User                Qt main thread          RecordingManager       FFI                 snapforge-app/encode    ffmpeg child
 │                       │                       │                  │                       │                      │
 │── Cmd+Shift+R ───────►│                       │                  │                       │                      │
 │                       │── activateForRecording                   │                       │                      │
 │                       │                       │                  │                       │                      │
 │── drag region ────────►│                       │                  │                       │                      │
 │                       │── startRecording(disp, rect, dir) ──────►│                       │                      │
 │                       │                       │  build JSON      │                       │                      │
 │                       │                       │──────────────────►snapforge_record_start                          │
 │                       │                       │                  │── parse JSON ─────────►record::ffmpeg::start  │
 │                       │                       │                  │                       │── find_ffmpeg() ────► │
 │                       │                       │                  │                       │── spawn(child) ─────►│
 │                       │                       │                  │                       │── spawn worker thread│
 │                       │                       │                  │◄──── RecordingHandle ─│                      │
 │                       │                       │◄── opaque handle─│                       │                      │
 │                       │◄── recordingStarted ──│                  │                       │                      │
 │                       │── show tray pill ─────│                  │                       │                      │
 │                       │── (pref) start click overlay + tap       │                       │                      │
 │                       │                       │                  │                       │  ┌──── frames ─────► │
 │                       │                       │                  │                       │  │  via stdin        │
 │                       │                       │                  │                       │  │                   │
 │── Stop (Cmd+Shift+R)─►│── stopRecording ─────►│── snapforge_record_stop ────────────────► RecordingHandle::stop │
 │                       │                       │                  │                       │── close stdin ─────► │
 │                       │                       │                  │                       │── wait for child ──► │
 │                       │                       │                  │                       │                      │
 │                       │                       │◄── 0/-1 ─────────│                       │                      │
 │                       │◄── recordingStopped(path) ──             │                       │                      │
 │                       │── tray banner + copy URL to clipboard    │                       │                      │
 │                       │── hide click overlay + stop tap          │                       │                      │
```

## Inputs (RecordConfig JSON)

Built in `qt/src/capture/RecordingManager.cpp` from prefs + user selection:

```json
{
  "display": 0,
  "region": { "x": 100, "y": 100, "width": 800, "height": 600 },
  "output_path": "/Users/.../Snapforge/2026-05-19_14-32-01.mp4",
  "format": "mp4",       // or "gif"
  "fps": 30,             // 10/15/24/30/60
  "quality": "medium",   // low/medium/high
  "ffmpeg_path": null,   // null = use bundled
  "add_to_history_on_stop": true
}
```

The `add_to_history_on_stop` flag tells `snapforge-app::recording` to add the finished file to the history index inside Rust on a successful stop — Qt no longer calls a separate `snapforge_history_add` (that primitive was deleted in Phase 2D).

## ffmpeg invocation

Built in `crates/snapforge-encode/src/record/ffmpeg.rs`. Shape:

```
ffmpeg -y -f rawvideo -pix_fmt rgba -s WxH -r FPS -i pipe:0 \
       -c:v libx264 -preset ultrafast -crf {quality_crf} \
       -pix_fmt yuv420p {output_path}
```

(Exact args depend on format/quality — see `ffmpeg.rs`.)

Frames are read from ScreenCaptureKit in the capture worker thread and written to ffmpeg stdin at the configured FPS. ffmpeg handles encoding + container muxing.

## Pause / resume

`snapforge_record_pause` / `snapforge_record_resume` flip a flag on the `RecordingHandle`. The capture worker skips frame-writes while paused (timer continues; no frames sent → ffmpeg sees a gap). Tray pill swaps dot → two short bars.

## Failure modes

| Where | Symptom | Surfaced to user |
|-------|---------|------------------|
| ffmpeg binary missing | `RecordError::FfmpegNotFound` | Modal "Recording Failed" + tray banner |
| ffmpeg spawn fails (permission, corruption) | `RecordError::FfmpegSpawnFailed` | Same |
| Stdin pipe write fails mid-recording | `RecordError::WriteFailed` | Same |
| SCK capture fails | `RecordError::CaptureFailed` | Same |
| Bad config JSON | `LAST_APP_ERROR` set, NULL handle | Modal via `recordingError` signal (read via `snapforge_app_last_error`) |

All paths go through the `recordingError` signal in `RecordingManager`, handled by `RecordingController` (wired in `main.cpp`) which resets tray + pops the deferred `QMessageBox`.

## File output

- **Path**: `~/Pictures/Snapforge/{filename_pattern}.{mp4|gif}` by default. `filename_pattern` is templated by prefs (timestamps, sequence numbers).
- **History**: indexing is now handled inside `snapforge_record_start` via `add_to_history_on_stop:true`. The use-case layer adds the finished file to the index after a successful stop (skipping incomplete mp4s automatically).
- **Clipboard**: file URL + plain path are copied so the user can paste into Finder/Slack/Messages.
- **Incomplete MP4 detection**: `snapforge_is_incomplete_mp4(path)` checks for missing moov atom (happens if app crashed mid-recording). History view flags these.

## Click visualizer integration (when "Show clicks while recording" is enabled)

- `recordingStarted` → `ClickIndicatorOverlay::showOverlay()` + `ClickTap::start()` (which wraps `snapforge_clicks_start`)
- Each click → ripple drawn into the overlay window
- The overlay window is rendered by the compositor on top of everything → captured by SCK → baked into the recording
- `recordingStopped/Error` → tear down overlay + tap

The overlay is **not** composited into frames at the Rust level — SCK simply sees it on screen and captures it like any other window. That's why the overlay must be at `NSScreenSaverWindowLevel` and visible on all Spaces.
