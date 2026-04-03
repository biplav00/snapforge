# ScreenSnap Phase 2c: System Tray, Global Hotkeys, Preferences — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make ScreenSnap a tray-resident app with global hotkeys for screenshot/last-region capture and a preferences window for customizing settings and keybindings.

**Architecture:** The app starts as a tray icon with no visible window. Global hotkeys and tray menu items trigger screenshot capture by creating an overlay window on-demand (capture screen first, then show overlay). A separate preferences window (different HTML entry point) lets users edit settings. Hotkey bindings are stored in `AppConfig` alongside existing settings.

**Tech Stack:** Tauri v2 (`tray-icon` feature), `tauri-plugin-global-shortcut`, Svelte 5, Vite multi-page build

---

## File Structure

```
New/modified files:

src-tauri/
├── Cargo.toml                    # add tray-icon feature, global-shortcut plugin
├── tauri.conf.json               # empty windows array (tray-only)
├── capabilities/default.json     # add global-shortcut permissions
├── src/
│   ├── main.rs                   # tray setup, hotkey registration, on-demand overlay
│   ├── tray.rs                   # tray menu creation and event handling
│   ├── hotkeys.rs                # global shortcut registration/unregistration
│   └── commands.rs               # add get_config, save_config, open_save_folder commands

crates/screen-core/src/
│   └── config.rs                 # add hotkey_bindings field to AppConfig

src/                              # overlay frontend (existing index.html)
├── preferences.html              # second Vite entry for preferences window
├── preferences-main.ts           # preferences app mount
└── lib/preferences/
    ├── Preferences.svelte        # root preferences component with tabs
    ├── GeneralTab.svelte         # save dir, clipboard, notifications, startup, remember region
    ├── HotkeysTab.svelte         # keybinding table with rebind UI
    └── ScreenshotsTab.svelte     # format, quality, filename pattern

vite.config.ts                    # add preferences.html as second entry
```

---

### Task 1: Add Hotkey Bindings to AppConfig

**Files:**
- Modify: `crates/screen-core/src/config.rs`

- [ ] **Step 1: Add hotkey fields to AppConfig**

Add a `hotkey_bindings` field to the `AppConfig` struct. Add it after `filename_pattern`:

```rust
// Add to AppConfig struct:
pub hotkey_bindings: HotkeyBindings,
```

Add the `HotkeyBindings` struct before `AppConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBindings {
    pub screenshot: String,
    pub capture_last_region: String,
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            screenshot: "CmdOrCtrl+Shift+S".to_string(),
            capture_last_region: "CmdOrCtrl+Shift+L".to_string(),
        }
    }
}
```

Update `AppConfig::default()` to include `hotkey_bindings: HotkeyBindings::default()`.

- [ ] **Step 2: Run tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test -p screen-core`
Expected: all tests pass (serde roundtrip test may need adjustment since the new field is present in defaults)

- [ ] **Step 3: Commit**

```bash
git add crates/screen-core/src/config.rs
git commit -m "feat: add hotkey bindings to AppConfig"
```

---

### Task 2: Tauri Commands for Config Read/Write and Open Folder

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add config commands to commands.rs**

Append these commands after the existing ones:

```rust
/// Get the current app config as JSON.
#[tauri::command]
pub fn get_config() -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&config).map_err(|e| e.to_string())
}

/// Save the app config from JSON.
#[tauri::command]
pub fn save_config(config_json: String) -> Result<(), String> {
    let config: screen_core::config::AppConfig =
        serde_json::from_str(&config_json).map_err(|e| e.to_string())?;
    config.save().map_err(|e| e.to_string())
}

/// Open the save directory in the system file manager.
#[tauri::command]
pub fn open_save_folder() -> Result<(), String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;
    let dir = &config.save_directory;
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    open::that(dir).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Add `open` crate dependency**

Add to `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
open = "5"
```

- [ ] **Step 3: Register new commands in main.rs**

Add `commands::get_config`, `commands::save_config`, and `commands::open_save_folder` to the `generate_handler!` macro.

- [ ] **Step 4: Verify builds**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap-app`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs src-tauri/Cargo.toml
git commit -m "feat: add Tauri commands for config read/write and open save folder"
```

---

### Task 3: System Tray Module

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/Cargo.toml` (add `tray-icon` feature)

- [ ] **Step 1: Add tray-icon feature to Cargo.toml**

Change the tauri dependency:

```toml
tauri = { version = "2", features = ["macos-private-api", "tray-icon"] }
```

- [ ] **Step 2: Create tray.rs**

```rust
// src-tauri/src/tray.rs
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let screenshot_item = MenuItem::with_id(app, "screenshot", "Screenshot", true, None::<&str>)?;
    let last_region_item = MenuItem::with_id(app, "last_region", "Capture Last Region", true, None::<&str>)?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let open_folder_item = MenuItem::with_id(app, "open_folder", "Open Save Folder", true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let preferences_item = MenuItem::with_id(app, "preferences", "Preferences", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &screenshot_item,
            &last_region_item,
            &separator1,
            &open_folder_item,
            &separator2,
            &preferences_item,
            &quit_item,
        ],
    )?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .menu_on_left_click(true)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "screenshot" => {
                    crate::trigger_screenshot(app);
                }
                "last_region" => {
                    crate::trigger_last_region(app);
                }
                "open_folder" => {
                    let _ = crate::commands::open_save_folder();
                }
                "preferences" => {
                    crate::open_preferences(app);
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
```

- [ ] **Step 3: Update main.rs with tray, on-demand overlay, and preferences window**

Replace the entire `src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod tray;
mod hotkeys;

use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Holds a pre-captured screenshot taken before the overlay window appears.
pub struct PreCapturedScreen(pub Mutex<Option<String>>);

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(PreCapturedScreen(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            commands::get_pre_captured_screen,
            commands::capture_screen,
            commands::save_region,
            commands::save_fullscreen,
            commands::save_composited_image,
            commands::copy_composited_image,
            commands::get_config,
            commands::save_config,
            commands::open_save_folder,
        ])
        .setup(|app| {
            // Hide dock icon on macOS — tray-only app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::create_tray(app.handle())?;
            hotkeys::register_hotkeys(app.handle())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Capture screen and open overlay window.
pub fn trigger_screenshot(app: &AppHandle) {
    // Capture before opening window
    let pre_captured = commands::capture_screen(0).ok();
    if let Ok(mut guard) = app.state::<PreCapturedScreen>().0.lock() {
        *guard = pre_captured;
    }

    // Close existing overlay if any
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }

    let _ = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html".into()))
        .title("ScreenSnap Overlay")
        .fullscreen(true)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .build();
}

/// Capture last region directly (no overlay).
pub fn trigger_last_region(app: &AppHandle) {
    match commands::save_last_region() {
        Ok(path) => {
            eprintln!("Saved last region to: {}", path);
        }
        Err(e) => {
            eprintln!("Last region capture failed: {}", e);
        }
    }
    let _ = app; // used for potential notification later
}

/// Open preferences window (or focus if already open).
pub fn open_preferences(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("preferences") {
        let _ = window.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(
        app,
        "preferences",
        WebviewUrl::App("preferences.html".into()),
    )
    .title("ScreenSnap Preferences")
    .inner_size(600.0, 480.0)
    .resizable(true)
    .build();
}
```

- [ ] **Step 4: Add save_last_region command to commands.rs**

Append to commands.rs:

```rust
/// Save a screenshot of the last remembered region.
pub fn save_last_region() -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let last = config.last_region
        .ok_or_else(|| "No last region saved".to_string())?;

    let save_path = config.save_file_path();
    let path = screen_core::screenshot_region(
        last.display,
        last.rect,
        &save_path,
        config.screenshot_format,
        config.jpg_quality,
        config.auto_copy_clipboard,
    )
    .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}
```

- [ ] **Step 5: Create placeholder hotkeys.rs**

```rust
// src-tauri/src/hotkeys.rs
use tauri::AppHandle;

pub fn register_hotkeys(_app: &AppHandle) -> tauri::Result<()> {
    // Will be implemented in Task 4
    Ok(())
}
```

- [ ] **Step 6: Verify builds**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap-app`
Expected: compiles with no errors

- [ ] **Step 7: Commit**

```bash
git add src-tauri/
git commit -m "feat: add system tray with menu and on-demand overlay window"
```

---

### Task 4: Global Hotkeys Module

**Files:**
- Modify: `src-tauri/src/hotkeys.rs`
- Modify: `src-tauri/Cargo.toml` (add global-shortcut plugin)

- [ ] **Step 1: Add global-shortcut plugin dependency**

Add to `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
tauri-plugin-global-shortcut = "2"
```

- [ ] **Step 2: Register the plugin in main.rs**

Add after `.plugin(tauri_plugin_shell::init())`:

```rust
.plugin(tauri_plugin_global_shortcut::Builder::new().build())
```

- [ ] **Step 3: Implement hotkeys.rs**

```rust
// src-tauri/src/hotkeys.rs
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

pub fn register_hotkeys(app: &AppHandle) -> tauri::Result<()> {
    let config = screen_core::config::AppConfig::load().unwrap_or_default();

    // Register screenshot hotkey
    if let Ok(shortcut) = config.hotkey_bindings.screenshot.parse::<Shortcut>() {
        let app_handle = app.clone();
        let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                crate::trigger_screenshot(&app_handle);
            }
        });
    }

    // Register last-region hotkey
    if let Ok(shortcut) = config.hotkey_bindings.capture_last_region.parse::<Shortcut>() {
        let app_handle = app.clone();
        let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                crate::trigger_last_region(&app_handle);
            }
        });
    }

    Ok(())
}

/// Unregister all hotkeys and re-register from current config.
pub fn reload_hotkeys(app: &AppHandle) -> tauri::Result<()> {
    let _ = app.global_shortcut().unregister_all();
    register_hotkeys(app)
}
```

- [ ] **Step 4: Add a Tauri command to reload hotkeys after preferences change**

Append to `commands.rs`:

```rust
/// Reload global hotkeys from config (call after saving preferences).
#[tauri::command]
pub fn reload_hotkeys(app: tauri::AppHandle) -> Result<(), String> {
    crate::hotkeys::reload_hotkeys(&app).map_err(|e| e.to_string())
}
```

Register it in `main.rs` by adding `commands::reload_hotkeys` to the `generate_handler!` macro.

- [ ] **Step 5: Verify builds**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap-app`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add src-tauri/
git commit -m "feat: add global hotkeys — screenshot and last-region capture"
```

---

### Task 5: Preferences Frontend — Vite Multi-Page Setup

**Files:**
- Create: `preferences.html`
- Create: `src/preferences-main.ts`
- Modify: `vite.config.ts` (add second entry)

- [ ] **Step 1: Create preferences.html**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ScreenSnap Preferences</title>
    <link rel="stylesheet" href="/src/styles/global.css" />
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/preferences-main.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: Create src/preferences-main.ts**

```ts
// src/preferences-main.ts
import Preferences from "./lib/preferences/Preferences.svelte";
import { mount } from "svelte";

const app = mount(Preferences, {
  target: document.getElementById("app")!,
});

export default app;
```

- [ ] **Step 3: Update vite.config.ts for multi-page**

```ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        preferences: resolve(__dirname, "preferences.html"),
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
  },
});
```

- [ ] **Step 4: Create placeholder Preferences.svelte**

```svelte
<!-- src/lib/preferences/Preferences.svelte -->
<script lang="ts">
</script>

<main>
  <h1>ScreenSnap Preferences</h1>
  <p>Loading...</p>
</main>

<style>
  main {
    padding: 24px;
    font-family: system-ui, sans-serif;
    color: #333;
  }
</style>
```

- [ ] **Step 5: Verify Vite builds both pages**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with `dist/index.html` and `dist/preferences.html`

- [ ] **Step 6: Commit**

```bash
git add preferences.html src/preferences-main.ts src/lib/preferences/ vite.config.ts
git commit -m "feat: scaffold preferences window with Vite multi-page build"
```

---

### Task 6: Preferences UI — General, Hotkeys, Screenshots Tabs

**Files:**
- Modify: `src/lib/preferences/Preferences.svelte`
- Create: `src/lib/preferences/GeneralTab.svelte`
- Create: `src/lib/preferences/HotkeysTab.svelte`
- Create: `src/lib/preferences/ScreenshotsTab.svelte`

- [ ] **Step 1: Create GeneralTab.svelte**

```svelte
<!-- src/lib/preferences/GeneralTab.svelte -->
<script lang="ts">
  interface Props {
    config: Record<string, unknown>;
    onChange: (key: string, value: unknown) => void;
  }

  let { config, onChange }: Props = $props();
</script>

<div class="tab-content">
  <div class="field">
    <label>Save Directory</label>
    <input
      type="text"
      value={config.save_directory as string}
      onchange={(e) => onChange("save_directory", (e.target as HTMLInputElement).value)}
    />
  </div>

  <div class="field">
    <label>
      <input
        type="checkbox"
        checked={config.auto_copy_clipboard as boolean}
        onchange={(e) => onChange("auto_copy_clipboard", (e.target as HTMLInputElement).checked)}
      />
      Auto-copy to clipboard after capture
    </label>
  </div>

  <div class="field">
    <label>
      <input
        type="checkbox"
        checked={config.show_notification as boolean}
        onchange={(e) => onChange("show_notification", (e.target as HTMLInputElement).checked)}
      />
      Show notification after capture
    </label>
  </div>

  <div class="field">
    <label>
      <input
        type="checkbox"
        checked={config.remember_last_region as boolean}
        onchange={(e) => onChange("remember_last_region", (e.target as HTMLInputElement).checked)}
      />
      Remember last region
    </label>
  </div>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field label {
    font-size: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  input[type="text"] {
    padding: 6px 10px;
    border: 1px solid #ccc;
    border-radius: 6px;
    font-size: 14px;
    font-family: monospace;
  }

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
  }
</style>
```

- [ ] **Step 2: Create HotkeysTab.svelte**

```svelte
<!-- src/lib/preferences/HotkeysTab.svelte -->
<script lang="ts">
  interface HotkeyBindings {
    screenshot: string;
    capture_last_region: string;
  }

  interface Props {
    bindings: HotkeyBindings;
    onChangeBinding: (action: string, newBinding: string) => void;
  }

  let { bindings, onChangeBinding }: Props = $props();

  let recording = $state<string | null>(null);
  let recordedKeys = $state("");

  const ACTIONS = [
    { id: "screenshot", label: "Screenshot" },
    { id: "capture_last_region", label: "Capture Last Region" },
  ];

  function startRecording(action: string) {
    recording = action;
    recordedKeys = "";
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    // Ignore lone modifier keys
    if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;

    const parts: string[] = [];
    if (e.ctrlKey || e.metaKey) parts.push("CmdOrCtrl");
    if (e.shiftKey) parts.push("Shift");
    if (e.altKey) parts.push("Alt");

    // Map key to a readable name
    let key = e.key.length === 1 ? e.key.toUpperCase() : e.code;
    // Convert "KeyA" to "A", "Digit1" to "1" etc.
    if (key.startsWith("Key")) key = key.slice(3);
    if (key.startsWith("Digit")) key = key.slice(5);

    parts.push(key);
    const combo = parts.join("+");

    recordedKeys = combo;
    onChangeBinding(recording, combo);
    recording = null;
  }

  function resetDefaults() {
    onChangeBinding("screenshot", "CmdOrCtrl+Shift+S");
    onChangeBinding("capture_last_region", "CmdOrCtrl+Shift+L");
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="tab-content">
  <table class="hotkey-table">
    <thead>
      <tr>
        <th>Action</th>
        <th>Shortcut</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each ACTIONS as action}
        <tr>
          <td>{action.label}</td>
          <td class="binding">
            {#if recording === action.id}
              <span class="recording">Press keys...</span>
            {:else}
              <code>{bindings[action.id as keyof HotkeyBindings]}</code>
            {/if}
          </td>
          <td>
            <button class="rebind-btn" onclick={() => startRecording(action.id)}>
              {recording === action.id ? "..." : "Rebind"}
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  <button class="reset-btn" onclick={resetDefaults}>Reset to Defaults</button>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .hotkey-table {
    width: 100%;
    border-collapse: collapse;
  }

  .hotkey-table th {
    text-align: left;
    padding: 8px;
    border-bottom: 1px solid #ddd;
    font-size: 13px;
    color: #666;
  }

  .hotkey-table td {
    padding: 8px;
    border-bottom: 1px solid #eee;
    font-size: 14px;
  }

  .binding code {
    background: #f0f0f0;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 13px;
  }

  .recording {
    color: #4a9eff;
    font-style: italic;
  }

  .rebind-btn {
    padding: 4px 12px;
    border: 1px solid #ccc;
    border-radius: 4px;
    background: white;
    cursor: pointer;
    font-size: 12px;
  }

  .rebind-btn:hover {
    background: #f0f0f0;
  }

  .reset-btn {
    align-self: flex-start;
    padding: 6px 16px;
    border: 1px solid #ccc;
    border-radius: 6px;
    background: white;
    cursor: pointer;
    font-size: 13px;
  }

  .reset-btn:hover {
    background: #f0f0f0;
  }
</style>
```

- [ ] **Step 3: Create ScreenshotsTab.svelte**

```svelte
<!-- src/lib/preferences/ScreenshotsTab.svelte -->
<script lang="ts">
  interface Props {
    config: Record<string, unknown>;
    onChange: (key: string, value: unknown) => void;
  }

  let { config, onChange }: Props = $props();

  const FORMATS = [
    { value: "Png", label: "PNG" },
    { value: "Jpg", label: "JPG" },
    { value: "WebP", label: "WebP" },
  ];
</script>

<div class="tab-content">
  <div class="field">
    <label>Default Format</label>
    <div class="radio-group">
      {#each FORMATS as fmt}
        <label class="radio-label">
          <input
            type="radio"
            name="format"
            value={fmt.value}
            checked={config.screenshot_format === fmt.value}
            onchange={() => onChange("screenshot_format", fmt.value)}
          />
          {fmt.label}
        </label>
      {/each}
    </div>
  </div>

  <div class="field">
    <label>JPG/WebP Quality: {config.jpg_quality}</label>
    <input
      type="range"
      min="1"
      max="100"
      value={config.jpg_quality as number}
      oninput={(e) => onChange("jpg_quality", parseInt((e.target as HTMLInputElement).value))}
    />
  </div>

  <div class="field">
    <label>Filename Pattern</label>
    <input
      type="text"
      value={config.filename_pattern as string}
      onchange={(e) => onChange("filename_pattern", (e.target as HTMLInputElement).value)}
    />
    <small class="hint">Use {"{date}"} and {"{time}"} as placeholders</small>
  </div>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field label {
    font-size: 14px;
  }

  .radio-group {
    display: flex;
    gap: 16px;
  }

  .radio-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 14px;
    cursor: pointer;
  }

  input[type="text"] {
    padding: 6px 10px;
    border: 1px solid #ccc;
    border-radius: 6px;
    font-size: 14px;
    font-family: monospace;
  }

  input[type="range"] {
    width: 100%;
  }

  .hint {
    color: #888;
    font-size: 12px;
  }
</style>
```

- [ ] **Step 4: Create Preferences.svelte with tabs and config load/save**

```svelte
<!-- src/lib/preferences/Preferences.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import GeneralTab from "./GeneralTab.svelte";
  import HotkeysTab from "./HotkeysTab.svelte";
  import ScreenshotsTab from "./ScreenshotsTab.svelte";

  let activeTab = $state("general");
  let config = $state<Record<string, unknown> | null>(null);
  let saving = $state(false);
  let statusMessage = $state("");

  const TABS = [
    { id: "general", label: "General" },
    { id: "hotkeys", label: "Hotkeys" },
    { id: "screenshots", label: "Screenshots" },
  ];

  async function loadConfig() {
    try {
      const json = await invoke<string>("get_config");
      config = JSON.parse(json);
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }

  function updateConfig(key: string, value: unknown) {
    if (!config) return;
    config = { ...config, [key]: value };
  }

  function updateHotkeyBinding(action: string, binding: string) {
    if (!config) return;
    const bindings = { ...(config.hotkey_bindings as Record<string, string>) };
    bindings[action] = binding;
    config = { ...config, hotkey_bindings: bindings };
  }

  async function saveConfig() {
    if (!config || saving) return;
    saving = true;
    statusMessage = "";
    try {
      await invoke("save_config", { configJson: JSON.stringify(config) });
      await invoke("reload_hotkeys");
      statusMessage = "Settings saved!";
      setTimeout(() => { statusMessage = ""; }, 2000);
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
    saving = false;
  }

  loadConfig();
</script>

<main>
  <h1>Preferences</h1>

  {#if !config}
    <p>Loading...</p>
  {:else}
    <nav class="tabs">
      {#each TABS as tab}
        <button
          class="tab-btn"
          class:active={activeTab === tab.id}
          onclick={() => (activeTab = tab.id)}
        >
          {tab.label}
        </button>
      {/each}
    </nav>

    <div class="tab-panel">
      {#if activeTab === "general"}
        <GeneralTab {config} onChange={updateConfig} />
      {:else if activeTab === "hotkeys"}
        <HotkeysTab
          bindings={config.hotkey_bindings as { screenshot: string; capture_last_region: string }}
          onChangeBinding={updateHotkeyBinding}
        />
      {:else if activeTab === "screenshots"}
        <ScreenshotsTab {config} onChange={updateConfig} />
      {/if}
    </div>

    <div class="footer">
      <button class="save-btn" onclick={saveConfig} disabled={saving}>
        {saving ? "Saving..." : "Save"}
      </button>
      {#if statusMessage}
        <span class="status">{statusMessage}</span>
      {/if}
    </div>
  {/if}
</main>

<style>
  main {
    padding: 24px;
    font-family: system-ui, sans-serif;
    color: #333;
    max-width: 560px;
  }

  h1 {
    font-size: 20px;
    margin-bottom: 16px;
  }

  .tabs {
    display: flex;
    gap: 0;
    border-bottom: 1px solid #ddd;
    margin-bottom: 20px;
  }

  .tab-btn {
    padding: 8px 20px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    font-size: 14px;
    cursor: pointer;
    color: #666;
  }

  .tab-btn:hover {
    color: #333;
  }

  .tab-btn.active {
    color: #4a9eff;
    border-bottom-color: #4a9eff;
  }

  .tab-panel {
    min-height: 200px;
  }

  .footer {
    margin-top: 24px;
    padding-top: 16px;
    border-top: 1px solid #eee;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .save-btn {
    padding: 8px 24px;
    background: #4a9eff;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 14px;
    cursor: pointer;
  }

  .save-btn:hover {
    background: #3a8eef;
  }

  .save-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .status {
    font-size: 13px;
    color: #44aa44;
  }
</style>
```

- [ ] **Step 5: Verify Vite build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with both `dist/index.html` and `dist/preferences.html`

- [ ] **Step 6: Commit**

```bash
git add src/lib/preferences/ src/preferences-main.ts
git commit -m "feat: add preferences UI — General, Hotkeys, Screenshots tabs"
```

---

### Task 7: Override global.css for Preferences Window

**Files:**
- Create: `src/styles/preferences.css`
- Modify: `preferences.html`

The global.css sets `background: transparent` which is needed for the overlay but makes the preferences window invisible.

- [ ] **Step 1: Create preferences.css**

```css
/* src/styles/preferences.css */
*, *::before, *::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  width: 100%;
  height: 100%;
  overflow: auto;
  background: white;
  font-family: system-ui, sans-serif;
}

#app {
  width: 100%;
  height: 100%;
}
```

- [ ] **Step 2: Update preferences.html to use preferences.css instead of global.css**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ScreenSnap Preferences</title>
    <link rel="stylesheet" href="/src/styles/preferences.css" />
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/preferences-main.ts"></script>
  </body>
</html>
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 4: Commit**

```bash
git add src/styles/preferences.css preferences.html
git commit -m "feat: add preferences stylesheet with opaque white background"
```

---

### Task 8: End-to-End Verification

- [ ] **Step 1: Run full build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build && cargo build -p screensnap-app && cargo test`
Expected: frontend builds (2 pages), Tauri compiles, all tests pass

- [ ] **Step 2: Manual E2E test**

Run: `cargo tauri dev`

Verify:
1. App starts with NO visible window — only a tray icon appears
2. No dock icon on macOS (tray-only)
3. Click tray icon → menu shows: Screenshot, Capture Last Region, Open Save Folder, Preferences, Quit
4. Click "Screenshot" → screen captured, overlay appears, draw region, annotate, save
5. Press the global hotkey (Cmd+Shift+S) → same as clicking Screenshot
6. Click "Open Save Folder" → Finder opens the save directory
7. Click "Preferences" → preferences window opens with 3 tabs
8. General tab: toggle settings, save → config file updated
9. Hotkeys tab: click Rebind, press new combo → shows updated binding, save → hotkeys updated
10. Screenshots tab: change format/quality → save → next capture uses new settings
11. Click "Quit" → app exits

- [ ] **Step 3: Fix any issues found during testing**

- [ ] **Step 4: Run all tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test`
Expected: all tests pass

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: Phase 2c complete — system tray, global hotkeys, preferences window"
```

---

## Phase 2c Summary

After completing all 8 tasks:

- Tray-resident app (no dock icon on macOS)
- System tray menu: Screenshot, Capture Last Region, Open Save Folder, Preferences, Quit
- Global hotkeys: Cmd+Shift+S (screenshot), Cmd+Shift+L (last region) — customizable
- Overlay window created on-demand (screen captured before window opens)
- Preferences window with 3 tabs:
  - General: save directory, clipboard, notifications, remember region
  - Hotkeys: rebind shortcuts with key recording, reset to defaults
  - Screenshots: format, quality slider, filename pattern
- Config saved to `~/Library/Application Support/screensnap/config.json`
- Hotkeys reload after saving preferences

**Next phases:**
- Phase 2b+: Additional annotation tools (Circle, Blur, Highlight, Steps, etc.)
- Phase 2d: Screen recording (FFmpeg integration)
