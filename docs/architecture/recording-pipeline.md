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

Built in `crates/snapforge-encode/src/record/ffmpeg.rs` (`ffmpeg_args`). Shape for MP4:

```
ffmpeg -y -nostats -loglevel error \
       -f rawvideo -pix_fmt bgra -s WxH -r FPS -i - -an \
       -c:v libx264 -preset fast -crf {28|23|18} \
       -vf scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,setparams=...bt709... \
       -pix_fmt yuv420p -colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv \
       -g {2*FPS} -movflags +frag_keyframe+empty_moov+default_base_moof \
       -- {output_path}
```

- **Colour**: captured pixels are sRGB (see [Frame source](#frame-source-capturerecordingsource)). RGB→YUV uses the BT.709 matrix (ffmpeg's default is BT.601), and the stream is tagged BT.709/limited range so players decode it correctly. BT.709 primaries are the same as sRGB's, so colours are not shifted.
- **Fragmented MP4**: `empty_moov` + `frag_keyframe` with a keyframe every 2 s means a recording interrupted by a crash or kill is still playable up to the last completed fragment. The file still begins with `ftyp` followed by an (empty) `moov`, so `is_incomplete_mp4` treats such files as complete.
- **GIF** uses a streaming per-frame palette (`palettegen=stats_mode=single` / `paletteuse=new=1`) and no colour tags.

The ffmpeg binary is located (`find_ffmpeg`) and validated (`ffmpeg -version`) once; the successful result is cached for later recordings, so these subprocesses don't run on the caller's thread every time.

Frames are captured with ScreenCaptureKit (sRGB BGRA, downscaled to at most 1920 px on the long side) in the worker thread, cropped to the region, and written to ffmpeg's stdin at a constant frame rate. If the loop stalls, the missed slots are filled with copies of the *previous* frame before the new frame is written, so new content never appears early. The requested region is clipped to the display bounds before scaling; a region entirely off-screen or too small fails the start with `CaptureFailed`.

### Frame source (`capture::RecordingSource`)

The worker creates a `RecordingSource` (in `crates/snapforge-capture/src/capture/macos.rs`) on its own thread:

- **Normal path: `StreamCapture`** (`capture/macos/stream.rs`). It runs one `SCStream` over the whole display (`SCContentFilter initWithDisplay:excludingWindows:@[]`, the same content as screenshots). It is configured with width and height equal to the output size, `kCVPixelFormatType_32BGRA`, `colorSpaceName = kCGColorSpaceSRGB`, `minimumFrameInterval = 1/fps`, `queueDepth = 4` and the cursor shown. SCK pushes `CMSampleBuffer`s to an `SCStreamOutput` object on a private serial dispatch queue. Only `SCFrameStatus.Complete` buffers are used. Idle buffers (screen unchanged) carry no image and are skipped. The `CVPixelBuffer` is locked read-only, copied row by row (it honours `bytesPerRow` padding) into a one-frame mailbox (`FrameSlot`, which recycles buffers), and unlocked. Each output slot, the encode loop takes the newest frame, if one arrived since the last slot. When none has arrived, it repeats the previous frame, which counts as a success. Pacing (`FrameClock`), pause, click compositing and region cropping are unchanged. The worker waits up to 2 s for the first frame. If none arrives, the file starts with a black frame and a warning is logged.
- **Stream failure**: `SCStreamDelegate stream:didStopWithError:` marks the stream dead. Every poll after that fails with `CaptureFailed`, so the usual 3 s failure streak ends the recording with `CaptureFailed`. Each poll checks `CGDisplayIsOnline` first, so a disconnected display still ends the recording immediately with `NoDisplay`.
- **Start/stop**: `startCapture` and `stopCapture` use completion handlers with bounded waits (5 s and 2 s). `stopCapture` is issued on every exit path, including `Drop` and a timed-out start.
- **Fallback: `CaptureContext`**. If the stream can't start (the SCK display lookup or the `addStreamOutput` call fails, or `startCapture` errors or times out), a warning is logged and the worker falls back to the old path. That path takes one `SCScreenshotManager` screenshot per frame, also in sRGB, with a 5 s timeout per frame. A `NoDisplay` error is not retried. The worker logs which source it picked (`[record] capturing via ...`).

**SIGPIPE**: the Rust code ships as a staticlib in the Qt app, where SIGPIPE is not ignored. The worker sets `F_SETNOSIGPIPE` on ffmpeg's stdin pipe, so if ffmpeg dies a write fails with `EPIPE` (turned into an error that carries ffmpeg's stderr, e.g. "No space left on device") instead of killing the app.

## Pause / resume

`snapforge_record_pause` / `snapforge_record_resume` atomically switch the handle between RUNNING and PAUSED (compare-and-swap; they never overwrite STOPPED and report failure once the recording has ended). While paused the worker captures and writes nothing **and freezes its frame clock**, so the output has no gap and no frozen-frame stretch: the video simply continues from the resume point at a constant frame rate. The tray pill swaps the dot for two short bars.

## Liveness, failures and stop

- `RecordingHandle::is_alive()` (FFI `snapforge_record_is_alive`) is true while the worker is RUNNING or PAUSED and false once the worker thread has exited, including after an error. The UI polls it to detect a recording that died on its own; `stop()` then returns the preserved error.
- Capture errors are logged (rate-limited). After 3 s of consecutive capture failures, or immediately if the display is disconnected (`CaptureError::NoDisplay`), the worker finalizes ffmpeg (keeping what was recorded) and ends with `CaptureFailed`.
- `stop()` closes stdin and waits up to 20 s for ffmpeg to finish the file before killing it. If the worker itself is stuck (for example, blocked writing to a hung ffmpeg), `stop()` kills ffmpeg after 25 s and returns an error, so the caller can never hang indefinitely.

## Failure modes

| Where | Symptom | Surfaced to user |
|-------|---------|------------------|
| ffmpeg binary missing | `RecordError::FfmpegNotFound` | Modal "Recording Failed" + tray banner |
| ffmpeg spawn fails (permission, corruption) | `RecordError::FfmpegSpawnFailed` | Same |
| ffmpeg exits / stdin write fails mid-recording | `RecordError::WriteFailed` with ffmpeg's stderr tail | Same (via `is_alive` going false, then stop) |
| ffmpeg hangs on stop | `RecordError::WriteFailed` (killed after deadline) | Same |
| SCK capture fails for 3 s / SCStream died (`didStopWithError`) / display disconnected | `RecordError::CaptureFailed` | Same |
| Region off-screen / invalid size | `RecordError::CaptureFailed` at start | Same |
| Bad config JSON | `LAST_APP_ERROR` set, NULL handle | Modal via `recordingError` signal (read via `snapforge_app_last_error`) |

All paths go through the `recordingError` signal in `RecordingManager`, handled by `RecordingController` (wired in `main.cpp`) which resets tray + pops the deferred `QMessageBox`.

## File output

- **Path**: `~/Pictures/Snapforge/{filename_pattern}.{mp4|gif}` by default. `filename_pattern` is templated by prefs (timestamps, sequence numbers).
- **History**: indexing is now handled inside `snapforge_record_start` via `add_to_history_on_stop:true`. The use-case layer adds the finished file to the index after a successful stop (skipping incomplete mp4s automatically).
- **Clipboard**: file URL + plain path are copied so the user can paste into Finder/Slack/Messages.
- **Incomplete MP4 detection**: `snapforge_is_incomplete_mp4(path)` checks for missing moov atom (happens if app crashed mid-recording). History view flags these.

## Click visualizer integration (when "Show clicks while recording" is enabled)

The Rust encoder draws the clicks itself. With `show_clicks: true`, `start_recording` starts a `ClickTracker` CGEventTap (needs Accessibility permission; without it the tap no-ops and no ripples are drawn). For each captured frame the worker takes a snapshot of recent clicks, converts their global desktop points into scaled-frame pixels of the target display, and composites a ripple (dot plus two expanding rings, about 1 s lifetime) into a scratch copy of the frame before writing it. Catch-up copies reuse the already-composed frame. With clicks off, frames are written straight through with no overlay work.
