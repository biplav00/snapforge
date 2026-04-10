# Recording, History, Preferences — Design Spec

## Summary

Add recording, history, and preferences windows to the Qt frontend. Recording uses existing Rust FFI for FFmpeg pipeline. History shows saved screenshots/recordings with thumbnails. Preferences provides tabbed settings UI.

## New FFI Functions Needed

Add to `crates/snapforge-ffi/src/lib.rs` and `snapforge_ffi.h`:

### Recording FFI
```c
// Start recording. config_json is a JSON string with display, region, format, fps, quality, output_path, ffmpeg_path.
// Returns an opaque handle. NULL on error.
void *snapforge_start_recording(const char *config_json);

// Stop recording. Returns 0 on success.
int snapforge_stop_recording(void *handle);

// Check if recording is active.
int snapforge_is_recording(void *handle);
```

### History FFI
```c
// Get history as JSON string. Caller frees via snapforge_free_string.
char *snapforge_history_list(void);

// Add entry to history. path is the file path.
int snapforge_history_add(const char *path);

// Delete single entry by path.
int snapforge_history_delete(const char *path);

// Clear all history.
int snapforge_history_clear(void);

// Get thumbnail for a path. Returns PNG bytes. Caller frees via snapforge_free_buffer.
CapturedImage snapforge_history_thumbnail(const char *path);
```

### Config FFI
```c
// Get config as JSON string. Caller frees via snapforge_free_string.
char *snapforge_config_load(void);

// Save config from JSON string. Returns 0 on success.
int snapforge_config_save(const char *json);
```

## 1. Recording

### Architecture
```
qt/src/
├── RecordingManager.h/cpp   (manages recording lifecycle, FFI calls)
└── (OverlayWindow.cpp)      (modify: add record mode)
```

### Flow
1. User presses Cmd+Shift+R → overlay opens in record mode
2. User draws region → "Record Region" and "Record Fullscreen" buttons appear (like current Svelte)
3. User clicks Record → overlay closes, recording starts via FFI
4. Tray menu changes to show "● Recording..." and "■ Stop"
5. User clicks Stop (or presses Cmd+Shift+R again) → recording stops
6. Saved file added to history

### RecordingManager
- Holds the opaque recording handle from FFI
- `startRecording(config)` → calls `snapforge_start_recording`
- `stopRecording()` → calls `snapforge_stop_recording`, returns output path
- `isRecording()` → checks handle state
- Signals: `recordingStarted()`, `recordingStopped(QString path)`

### Tray Integration
- When recording: tray menu shows "● Recording... (MM:SS)" and "■ Stop Recording"
- Timer updates tray tooltip/menu text every second
- Stop menu item calls `stopRecording()`

### OverlayWindow Changes
- New mode: `RecordSelect` (like Svelte's `record-select`)
- After region drawn in record mode: show "Record Region (Enter)" and "Record Fullscreen" buttons
- Enter starts region recording, closes overlay
- No annotation in record mode

## 2. History

### Architecture
```
qt/src/
├── HistoryWindow.h/cpp      (main history window)
└── (main.cpp)               (modify: tray menu "History" item)
```

### HistoryWindow
- Regular QWidget window (not overlay), 960x640, resizable, min 600x400
- Title: "Snapforge History"

### Layout
- **Top toolbar**: Search input (QLineEdit) + filter dropdown (All/Images/Videos) + sort dropdown (Newest/Oldest/Name)
- **Grid**: QScrollArea with flow layout of thumbnail cards
- **Footer**: Entry count label + "Clear All" button

### Thumbnail Card (200px wide, 16:10 aspect)
- Thumbnail image (loaded from history FFI)
- Filename label
- Timestamp label
- Action buttons: Copy (images only), Show in Folder, Delete
- Click opens preview

### Preview
- Modal overlay within the history window
- Full-size image display
- Action buttons: Copy, Show in Folder, Delete, Close
- Escape closes preview

### Data Flow
- On open: call `snapforge_history_list()` → parse JSON → load thumbnails
- Search/filter/sort computed from the loaded list (client-side)
- Delete: call `snapforge_history_delete()` → remove from UI list
- Clear: call `snapforge_history_clear()` → empty UI list
- Copy: call `snapforge_copy_to_clipboard()` with the file's image data
- Show in Folder: `QDesktopServices::openUrl()` on parent directory

## 3. Preferences

### Architecture
```
qt/src/
├── PreferencesWindow.h/cpp  (tabbed preferences window)
└── (main.cpp)               (modify: tray menu "Preferences" item)
```

### PreferencesWindow
- Regular QWidget, 600x480, resizable
- Title: "Snapforge Preferences"
- QTabWidget with 4 tabs

### General Tab
- Save directory: QLineEdit + browse button (QFileDialog)
- Toggle switches (QCheckBox styled): auto_copy_clipboard, show_notification, remember_last_region
- Launch at startup: QCheckBox

### Screenshots Tab
- Format: radio button group (PNG, JPG, WebP)
- Quality slider: QSlider 1-100 (shown for JPG/WebP)
- Filename pattern: QLineEdit with placeholder hint

### Recording Tab
- Format: radio button group (MP4, GIF)
- FPS: button group (10, 15, 24, 30, 60)
- Quality: button group (Low, Medium, High)

### Hotkeys Tab
- Table (QTableWidget) with columns: Action, Shortcut, Change button
- Sections: Global, Tools, Sizes, Actions
- "Change" button enters recording mode: captures next keypress as new binding
- Conflict detection: warn if binding already used
- Reset to Defaults button

### Data Flow
- On open: call `snapforge_config_load()` → parse JSON → populate UI
- Save button: collect UI state → serialize JSON → `snapforge_config_save()`
- Hotkey changes require `reload_hotkeys()` (new FFI function or signal to main)

## Tray Menu Updates

Current menu:
```
Screenshot (Cmd+Shift+S)
---
Quit
```

New menu:
```
Screenshot (Cmd+Shift+S)
Record (Cmd+Shift+R)
---
History (Cmd+Shift+H)
Preferences
---
Quit
```

When recording:
```
● Recording... (00:15)
■ Stop Recording
---
History (Cmd+Shift+H)
Preferences
---
Quit
```

## What is NOT included
- Click ripple animation during recording (enhancement, later)
- Region outline overlay during recording (enhancement, later)
- Auto-update mechanism
- Launch at startup implementation (platform-specific, later)
