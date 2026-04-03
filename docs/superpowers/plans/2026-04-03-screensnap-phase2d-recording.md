# ScreenSnap Phase 2d: Screen Recording — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add screen recording — capture frames via CoreGraphics, pipe to system FFmpeg for MP4/GIF encoding, with a floating recording indicator and tray/hotkey integration.

**Architecture:** A new `record` module in `screen-core` handles the frame capture loop in a background thread, piping raw BGRA frames to FFmpeg via stdin. The Tauri layer exposes start/stop commands and manages recording state. A small floating indicator window (Svelte) shows elapsed time and a stop button. The overlay is reused for region selection before recording starts.

**Tech Stack:** Rust (`std::process::Command` for FFmpeg), CoreGraphics (frame capture), Tauri v2 (multi-window), Svelte 5 (indicator UI)

**Prerequisite:** FFmpeg must be installed on the system (`brew install ffmpeg`). The app checks for FFmpeg availability and reports an error if missing.

---

## File Structure

```
New files:
  crates/screen-core/src/record/
  ├── mod.rs              # RecordConfig, RecordingHandle, start/stop API
  └── ffmpeg.rs           # FFmpeg process spawn, frame writing, cleanup

  src-tauri/src/
  └── recording.rs        # Tauri recording state, start/stop commands

  src/lib/recording/
  └── Indicator.svelte    # floating recording indicator (red dot + timer + stop)

  src/recording.html      # Vite entry for indicator window
  src/recording-main.ts   # indicator app mount

Modified files:
  crates/screen-core/src/lib.rs          # add pub mod record
  crates/screen-core/src/config.rs       # add RecordingConfig fields
  crates/screen-core/Cargo.toml          # no new deps needed
  src-tauri/src/main.rs                  # recording state, commands, trigger_recording fn
  src-tauri/src/tray.rs                  # add Record Screen menu item
  src-tauri/src/hotkeys.rs               # add recording hotkey
  src-tauri/src/commands.rs              # add recording commands
  src/lib/overlay/Overlay.svelte         # support recording mode (region select → start recording)
  src/lib/overlay/RegionSelector.svelte  # add "Record" button alongside annotate
  vite.config.ts                         # add recording.html entry
```

---

### Task 1: Recording Config

**Files:**
- Modify: `crates/screen-core/src/config.rs`

- [ ] **Step 1: Add RecordingConfig to config.rs**

Add this struct before `HotkeyBindings`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingFormat {
    Mp4,
    Gif,
}

impl Default for RecordingFormat {
    fn default() -> Self {
        RecordingFormat::Mp4
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingQuality {
    Low,
    Medium,
    High,
}

impl Default for RecordingQuality {
    fn default() -> Self {
        RecordingQuality::Medium
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub format: RecordingFormat,
    pub fps: u32,
    pub quality: RecordingQuality,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            format: RecordingFormat::default(),
            fps: 30,
            quality: RecordingQuality::default(),
        }
    }
}
```

Add to `AppConfig` struct:

```rust
pub recording: RecordingConfig,
```

Add to `AppConfig::default()`:

```rust
recording: RecordingConfig::default(),
```

Add to `HotkeyBindings` struct:

```rust
pub record_screen: String,
```

Add to `HotkeyBindings::default()`:

```rust
record_screen: "CmdOrCtrl+Shift+R".to_string(),
```

Also add a helper method to `AppConfig`:

```rust
pub fn recording_file_path(&self) -> PathBuf {
    let now = chrono::Local::now();
    let ext = match self.recording.format {
        RecordingFormat::Mp4 => "mp4",
        RecordingFormat::Gif => "gif",
    };
    let filename = format!("recording-{}.{}", now.format("%Y-%m-%d-%H-%M-%S"), ext);
    self.save_directory.join(filename)
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/screen-core/src/config.rs
git commit -m "feat: add recording config — format, fps, quality, hotkey"
```

---

### Task 2: FFmpeg Recording Module in screen-core

**Files:**
- Create: `crates/screen-core/src/record/mod.rs`
- Create: `crates/screen-core/src/record/ffmpeg.rs`
- Modify: `crates/screen-core/src/lib.rs`

- [ ] **Step 1: Create record/mod.rs**

```rust
// crates/screen-core/src/record/mod.rs
pub mod ffmpeg;

use crate::config::{RecordingFormat, RecordingQuality};
use crate::types::Rect;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("ffmpeg not found — install with: brew install ffmpeg")]
    FfmpegNotFound,
    #[error("ffmpeg failed to start: {0}")]
    FfmpegSpawnFailed(String),
    #[error("recording write failed: {0}")]
    WriteFailed(String),
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    #[error("recording not active")]
    NotActive,
}

#[derive(Debug, Clone)]
pub struct RecordConfig {
    pub display: usize,
    pub region: Option<Rect>,
    pub output_path: PathBuf,
    pub format: RecordingFormat,
    pub fps: u32,
    pub quality: RecordingQuality,
}

/// Check if FFmpeg is available on the system.
pub fn check_ffmpeg() -> Result<(), RecordError> {
    match std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        _ => Err(RecordError::FfmpegNotFound),
    }
}
```

- [ ] **Step 2: Create record/ffmpeg.rs**

```rust
// crates/screen-core/src/record/ffmpeg.rs
use super::{RecordConfig, RecordError};
use crate::capture;
use crate::config::{RecordingFormat, RecordingQuality};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct RecordingHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Result<(), RecordError>>>,
}

impl RecordingHandle {
    /// Stop the recording and wait for FFmpeg to finish.
    pub fn stop(mut self) -> Result<(), RecordError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| RecordError::WriteFailed("thread panicked".into()))?
        } else {
            Ok(())
        }
    }

    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

/// Start recording. Returns a handle that must be stopped to finalize the output file.
pub fn start_recording(config: RecordConfig) -> Result<RecordingHandle, RecordError> {
    super::check_ffmpeg()?;

    // Determine frame dimensions by capturing one test frame
    let test_frame = if let Some(region) = &config.region {
        capture::capture_region(config.display, *region)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?
    } else {
        capture::capture_fullscreen(config.display)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?
    };

    let width = test_frame.width();
    let height = test_frame.height();

    let mut child = spawn_ffmpeg(&config, width, height)?;
    let mut stdin = child.stdin.take()
        .ok_or_else(|| RecordError::FfmpegSpawnFailed("no stdin".into()))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);

    let thread = std::thread::spawn(move || -> Result<(), RecordError> {
        let mut writer = std::io::BufWriter::with_capacity(
            (width * height * 4) as usize,
            &mut stdin,
        );

        // Write the test frame as the first frame
        writer.write_all(test_frame.as_raw())
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        let start = Instant::now();
        let mut next_frame_time = start + frame_interval;

        while !stop_clone.load(Ordering::SeqCst) {
            // Sleep until next frame time
            let now = Instant::now();
            if now < next_frame_time {
                std::thread::sleep(next_frame_time - now);
            }
            next_frame_time += frame_interval;

            // Capture frame
            let frame = if let Some(region) = &config.region {
                capture::capture_region(config.display, *region)
            } else {
                capture::capture_fullscreen(config.display)
            };

            match frame {
                Ok(img) => {
                    if writer.write_all(img.as_raw()).is_err() {
                        break; // FFmpeg process died
                    }
                }
                Err(_) => {
                    // Skip frame on capture failure
                    continue;
                }
            }
        }

        // Flush and close stdin to signal FFmpeg to finish
        drop(writer);
        drop(stdin);

        // Wait for FFmpeg to finish encoding
        let status = child.wait()
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        if !status.success() {
            return Err(RecordError::WriteFailed(
                format!("ffmpeg exited with code: {:?}", status.code()),
            ));
        }

        Ok(())
    });

    Ok(RecordingHandle {
        stop_flag,
        thread: Some(thread),
    })
}

fn spawn_ffmpeg(config: &RecordConfig, width: u32, height: u32) -> Result<Child, RecordError> {
    let crf = match config.quality {
        RecordingQuality::Low => "28",
        RecordingQuality::Medium => "23",
        RecordingQuality::High => "18",
    };

    let size_arg = format!("{}x{}", width, height);
    let fps_arg = config.fps.to_string();
    let output_path = config.output_path.to_string_lossy().to_string();

    // Ensure output directory exists
    if let Some(parent) = config.output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut args: Vec<String> = vec![
        "-y".into(),
        "-f".into(), "rawvideo".into(),
        "-pix_fmt".into(), "rgba".into(),
        "-s".into(), size_arg,
        "-r".into(), fps_arg,
        "-i".into(), "-".into(),
        "-an".into(),
    ];

    match config.format {
        RecordingFormat::Mp4 => {
            args.extend([
                "-c:v".into(), "libx264".into(),
                "-preset".into(), "fast".into(),
                "-crf".into(), crf.into(),
                "-pix_fmt".into(), "yuv420p".into(),
            ]);
        }
        RecordingFormat::Gif => {
            args.extend([
                "-vf".into(),
                "split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".into(),
                "-loop".into(), "0".into(),
            ]);
        }
    }

    args.push(output_path);

    Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| RecordError::FfmpegSpawnFailed(e.to_string()))
}
```

- [ ] **Step 3: Add pub mod record to lib.rs**

Add to `crates/screen-core/src/lib.rs` after `pub mod config;`:

```rust
pub mod record;
```

Add to the `ScreenError` enum:

```rust
#[error("recording error: {0}")]
Recording(#[from] record::RecordError),
```

- [ ] **Step 4: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screen-core`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add crates/screen-core/src/record/ crates/screen-core/src/lib.rs
git commit -m "feat: add recording module — FFmpeg frame piping with start/stop API"
```

---

### Task 3: Tauri Recording State and Commands

**Files:**
- Create: `src-tauri/src/recording.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create recording.rs**

```rust
// src-tauri/src/recording.rs
use screen_core::record::ffmpeg::RecordingHandle;
use std::sync::Mutex;

/// Global recording state managed by Tauri.
pub struct RecordingState {
    pub handle: Mutex<Option<RecordingHandle>>,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.handle.lock().map(|h| h.is_some()).unwrap_or(false)
    }
}
```

- [ ] **Step 2: Add recording commands to commands.rs**

Append to `src-tauri/src/commands.rs`:

```rust
/// Check if FFmpeg is installed.
#[tauri::command]
pub fn check_ffmpeg() -> Result<(), String> {
    screen_core::record::check_ffmpeg().map_err(|e| e.to_string())
}

/// Start recording. Returns the output file path.
#[tauri::command]
pub fn start_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
    display: usize,
    region_x: Option<i32>,
    region_y: Option<i32>,
    region_w: Option<u32>,
    region_h: Option<u32>,
) -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let region = match (region_x, region_y, region_w, region_h) {
        (Some(x), Some(y), Some(w), Some(h)) => {
            Some(screen_core::types::Rect { x, y, width: w, height: h })
        }
        _ => None,
    };

    let output_path = config.recording_file_path();
    let record_config = screen_core::record::RecordConfig {
        display,
        region,
        output_path: output_path.clone(),
        format: config.recording.format,
        fps: config.recording.fps,
        quality: config.recording.quality,
    };

    let handle = screen_core::record::ffmpeg::start_recording(record_config)
        .map_err(|e| e.to_string())?;

    let mut guard = state.handle.lock().map_err(|e| e.to_string())?;
    *guard = Some(handle);

    Ok(output_path.display().to_string())
}

/// Stop recording.
#[tauri::command]
pub fn stop_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
) -> Result<(), String> {
    let mut guard = state.handle.lock().map_err(|e| e.to_string())?;
    if let Some(handle) = guard.take() {
        handle.stop().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Check if currently recording.
#[tauri::command]
pub fn is_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
) -> bool {
    state.is_recording()
}
```

- [ ] **Step 3: Update main.rs**

Add `mod recording;` after `mod tray;`.

Add `.manage(recording::RecordingState::new())` after the existing `.manage(PreCapturedScreen(...))`.

Add to `generate_handler!`:
```rust
commands::check_ffmpeg,
commands::start_recording,
commands::stop_recording,
commands::is_recording,
```

Add a `trigger_recording` function:

```rust
/// Open overlay in recording mode for region selection, then start recording.
pub fn trigger_recording(app: &AppHandle) {
    // For now, start fullscreen recording directly.
    // Region selection via overlay will be added in the UI task.
    let state = app.state::<recording::RecordingState>();
    if state.is_recording() {
        // If already recording, stop
        if let Ok(mut guard) = state.handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.stop();
            }
        }
        // Close indicator window
        if let Some(window) = app.get_webview_window("recording-indicator") {
            let _ = window.close();
        }
    } else {
        // Start fullscreen recording
        let config = screen_core::config::AppConfig::load().unwrap_or_default();
        let output_path = config.recording_file_path();
        let record_config = screen_core::record::RecordConfig {
            display: 0,
            region: None,
            output_path,
            format: config.recording.format,
            fps: config.recording.fps,
            quality: config.recording.quality,
        };

        match screen_core::record::ffmpeg::start_recording(record_config) {
            Ok(handle) => {
                if let Ok(mut guard) = state.handle.lock() {
                    *guard = Some(handle);
                }
                // Open indicator window
                let _ = WebviewWindowBuilder::new(
                    app,
                    "recording-indicator",
                    WebviewUrl::App("recording.html".into()),
                )
                .title("Recording")
                .inner_size(200.0, 50.0)
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .transparent(true)
                .build();
            }
            Err(e) => {
                eprintln!("Recording failed: {}", e);
            }
        }
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap-app`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/recording.rs src-tauri/src/commands.rs src-tauri/src/main.rs
git commit -m "feat: add Tauri recording state and start/stop commands"
```

---

### Task 4: Tray Menu and Hotkey for Recording

**Files:**
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/hotkeys.rs`

- [ ] **Step 1: Add Record Screen item to tray.rs**

Read `src-tauri/src/tray.rs`. Add a new menu item after `last_region_item`:

```rust
let record_item = MenuItem::with_id(app, "record", "Record Screen", true, None::<&str>)?;
```

Add `&record_item` to the `Menu::with_items` call (after `&last_region_item`).

Add a handler in the `on_menu_event` match:

```rust
"record" => {
    crate::trigger_recording(app);
}
```

- [ ] **Step 2: Add recording hotkey to hotkeys.rs**

Read `src-tauri/src/hotkeys.rs`. Add after the last-region hotkey registration block:

```rust
// Register record-screen hotkey
if let Ok(shortcut) = config.hotkey_bindings.record_screen.parse::<Shortcut>() {
    let app_handle = app.clone();
    let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            crate::trigger_recording(&app_handle);
        }
    });
}
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap-app`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tray.rs src-tauri/src/hotkeys.rs
git commit -m "feat: add Record Screen to tray menu and global hotkey"
```

---

### Task 5: Recording Indicator UI

**Files:**
- Create: `recording.html`
- Create: `src/recording-main.ts`
- Create: `src/lib/recording/Indicator.svelte`
- Modify: `vite.config.ts`

- [ ] **Step 1: Create recording.html**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Recording</title>
    <style>
      * { margin: 0; padding: 0; box-sizing: border-box; }
      html, body { background: transparent; overflow: hidden; }
      #app { width: 100%; height: 100%; }
    </style>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/recording-main.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: Create src/recording-main.ts**

```ts
import Indicator from "./lib/recording/Indicator.svelte";
import { mount } from "svelte";

const app = mount(Indicator, {
  target: document.getElementById("app")!,
});

export default app;
```

- [ ] **Step 3: Create Indicator.svelte**

```svelte
<!-- src/lib/recording/Indicator.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

  let elapsed = $state(0);
  let interval: ReturnType<typeof setInterval> | null = null;

  const appWindow = getCurrentWebviewWindow();

  function formatTime(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }

  async function stopRecording() {
    try {
      await invoke("stop_recording");
    } catch (e) {
      console.error("Failed to stop:", e);
    }
    if (interval) clearInterval(interval);
    appWindow.close();
  }

  // Start timer
  interval = setInterval(() => {
    elapsed += 1;
  }, 1000);
</script>

<div class="indicator">
  <div class="dot"></div>
  <span class="time">{formatTime(elapsed)}</span>
  <button class="stop-btn" onclick={stopRecording}>■ Stop</button>
</div>

<style>
  .indicator {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(20, 20, 20, 0.9);
    backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 60, 60, 0.4);
    border-radius: 20px;
    padding: 6px 14px;
    font-family: system-ui, sans-serif;
    cursor: default;
    user-select: none;
    -webkit-app-region: drag;
  }

  .dot {
    width: 10px;
    height: 10px;
    background: #ff3333;
    border-radius: 50%;
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .time {
    color: white;
    font-size: 14px;
    font-variant-numeric: tabular-nums;
    min-width: 40px;
  }

  .stop-btn {
    background: rgba(255, 60, 60, 0.2);
    color: #ff6666;
    border: 1px solid rgba(255, 60, 60, 0.3);
    border-radius: 12px;
    padding: 3px 10px;
    font-size: 12px;
    cursor: pointer;
    font-family: system-ui, sans-serif;
    -webkit-app-region: no-drag;
  }

  .stop-btn:hover {
    background: rgba(255, 60, 60, 0.4);
    color: white;
  }
</style>
```

- [ ] **Step 4: Update vite.config.ts**

Read the current file and add the recording entry to the rollup inputs:

```ts
input: {
  main: resolve(__dirname, "index.html"),
  preferences: resolve(__dirname, "preferences.html"),
  recording: resolve(__dirname, "recording.html"),
},
```

- [ ] **Step 5: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with `dist/recording.html` present

- [ ] **Step 6: Commit**

```bash
git add recording.html src/recording-main.ts src/lib/recording/ vite.config.ts
git commit -m "feat: add recording indicator UI — red dot, timer, stop button"
```

---

### Task 6: CLI Recording Support

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add Record subcommand to CLI**

Read `cli/src/main.rs`. Add a new `Record` variant to the `Commands` enum:

```rust
/// Record the screen
Record {
    /// Record the full screen
    #[arg(long)]
    fullscreen: bool,

    /// Record a specific region: x,y,width,height
    #[arg(long, value_parser = parse_region)]
    region: Option<Rect>,

    /// Output format: mp4, gif
    #[arg(long, short, default_value = "mp4")]
    format: String,

    /// Frame rate
    #[arg(long, default_value = "30")]
    fps: u32,

    /// Output file path
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Display index
    #[arg(long, short, default_value = "0")]
    display: usize,
},
```

Add the match arm in `main()`:

```rust
Commands::Record {
    fullscreen,
    region,
    format,
    fps,
    output,
    display,
} => {
    if let Err(e) = handle_record(fullscreen, region, format, fps, output, display) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
```

Add the handler function:

```rust
fn handle_record(
    fullscreen: bool,
    region: Option<Rect>,
    format_str: String,
    fps: u32,
    output: Option<PathBuf>,
    display: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    screen_core::record::check_ffmpeg()?;

    let config = screen_core::config::AppConfig::load()?;
    let recording_format = match format_str.to_lowercase().as_str() {
        "gif" => screen_core::config::RecordingFormat::Gif,
        _ => screen_core::config::RecordingFormat::Mp4,
    };

    let output_path = output.unwrap_or_else(|| config.recording_file_path());

    let record_region = if let Some(r) = region {
        let dpr = 1; // CLI doesn't have DPR context
        Some(Rect {
            x: r.x * dpr,
            y: r.y * dpr,
            width: r.width * dpr as u32,
            height: r.height * dpr as u32,
        })
    } else if fullscreen {
        None
    } else {
        eprintln!("Specify --fullscreen or --region for CLI recording.");
        std::process::exit(1);
    };

    let record_config = screen_core::record::RecordConfig {
        display,
        region: record_region,
        output_path: output_path.clone(),
        format: recording_format,
        fps,
        quality: config.recording.quality,
    };

    println!("Recording to: {} (press Ctrl+C to stop)", output_path.display());

    let handle = screen_core::record::ffmpeg::start_recording(record_config)?;

    // Wait for Ctrl+C
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    println!("\nStopping recording...");
    handle.stop()?;
    println!("Saved to: {}", output_path.display());

    Ok(())
}
```

- [ ] **Step 2: Add ctrlc dependency to cli/Cargo.toml**

```toml
ctrlc = "3"
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add cli/
git commit -m "feat: add CLI recording — screen record --fullscreen/--region with Ctrl+C stop"
```

---

### Task 7: E2E Verification

- [ ] **Step 1: Install FFmpeg**

Run: `brew install ffmpeg` (if not already installed)
Verify: `ffmpeg -version`

- [ ] **Step 2: Full build and tests**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build && cargo build -p screensnap-app && cargo test`
Expected: all pass

- [ ] **Step 3: Test CLI recording**

Run: `cargo run -p screensnap -- record --fullscreen --output /tmp/test-recording.mp4`
Wait 3 seconds, press Ctrl+C.
Expected: `Saved to: /tmp/test-recording.mp4`, file is a valid MP4

Run: `cargo run -p screensnap -- record --fullscreen --format gif --fps 10 --output /tmp/test-recording.gif`
Wait 3 seconds, press Ctrl+C.
Expected: `Saved to: /tmp/test-recording.gif`, file is a valid GIF

- [ ] **Step 4: Test Tauri recording**

Run: `cargo tauri dev`
Click tray → "Record Screen" → indicator appears → click Stop → file saved
Or press Cmd+Shift+R → recording starts → press Cmd+Shift+R again → recording stops

- [ ] **Step 5: Fix any issues**

- [ ] **Step 6: Final commit**

```bash
git add -A
git commit -m "feat: Phase 2d complete — screen recording with FFmpeg, indicator, CLI support"
```

---

## Phase 2d Summary

After completing all 7 tasks:

- **screen-core recording module**: Frame capture loop → FFmpeg stdin pipe → MP4/GIF output
- **FFmpeg integration**: Uses system FFmpeg, checks availability, supports H.264 MP4 and palette-optimized GIF
- **Recording config**: Format (MP4/GIF), FPS (15/24/30/60), quality (Low/Medium/High)
- **Tauri integration**: Start/stop commands, recording state management, floating indicator window
- **Recording indicator**: Red pulsing dot, elapsed timer, stop button, draggable
- **Tray menu**: "Record Screen" item, toggles recording on/off
- **Global hotkey**: Cmd+Shift+R toggles recording
- **CLI**: `screen record --fullscreen`, `screen record --region x,y,w,h`, `--format mp4/gif`, `--fps`, Ctrl+C to stop

**Prerequisite:** `brew install ffmpeg`
