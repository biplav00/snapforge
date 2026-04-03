# ScreenSnap Phase 2a: Tauri Shell + Overlay + Region Selection — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Tauri v2 GUI shell with a transparent fullscreen overlay for interactive region selection, so that `screen capture` (no flags) launches the overlay, the user draws a region, and the screenshot is saved/copied.

**Architecture:** New workspace member `src-tauri/` (Rust Tauri app) + `src/` (Svelte frontend via Vite). The Tauri app depends on `screen-core` for capture/save. On trigger, Rust captures a fullscreen image, encodes it as base64 PNG, sends it to the Svelte overlay webview which displays it as background. User draws a region on canvas, sends coordinates back to Rust, which crops and saves.

**Tech Stack:** Tauri v2 (~2.10), Svelte 5 (vanilla, not SvelteKit), Vite, TypeScript

---

## File Structure

```
screen/
├── Cargo.toml                          # workspace root (add src-tauri member)
├── src-tauri/
│   ├── Cargo.toml                      # tauri app crate, depends on screen-core
│   ├── build.rs                        # tauri build script
│   ├── tauri.conf.json                 # tauri config (windows, app metadata)
│   ├── capabilities/
│   │   └── default.json                # permissions for windows
│   ├── icons/                          # app icons (placeholder)
│   └── src/
│       ├── main.rs                     # tauri app entry point
│       └── commands.rs                 # tauri command handlers (capture, save)
├── src/                                # svelte frontend
│   ├── main.ts                         # svelte app mount
│   ├── App.svelte                      # root component (routes by window label)
│   ├── lib/
│   │   └── overlay/
│   │       ├── Overlay.svelte          # fullscreen overlay with frozen screenshot
│   │       └── RegionSelector.svelte   # canvas-based region draw/resize/move
│   └── styles/
│       └── global.css                  # global styles (transparent body, reset)
├── package.json                        # npm deps (svelte, vite, tauri api)
├── vite.config.ts                      # vite config for svelte
├── svelte.config.js                    # svelte compiler config
├── tsconfig.json                       # typescript config
├── index.html                          # vite entry HTML
└── cli/                                # existing CLI (unchanged)
```

---

### Task 1: Install Tooling and Scaffold Frontend

**Files:**
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `svelte.config.js`
- Create: `tsconfig.json`
- Create: `index.html`
- Create: `src/main.ts`
- Create: `src/App.svelte`
- Create: `src/styles/global.css`

- [ ] **Step 1: Install cargo-tauri CLI**

Run: `cargo install tauri-cli --version "^2"`
Expected: `cargo tauri --version` prints `tauri-cli 2.x.x`

- [ ] **Step 2: Initialize npm project and install dependencies**

Run from project root:
```bash
cd /Users/biplav00/Documents/personal/screen
npm init -y
npm install -D @sveltejs/vite-plugin-svelte svelte vite typescript
npm install @tauri-apps/api @tauri-apps/plugin-shell
```

- [ ] **Step 3: Create vite.config.ts**

```ts
// screen/vite.config.ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
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

- [ ] **Step 4: Create svelte.config.js**

```js
// screen/svelte.config.js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
};
```

- [ ] **Step 5: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ESNext",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "resolveJsonModule": true,
    "allowJs": true,
    "checkJs": true,
    "isolatedModules": true,
    "moduleResolution": "bundler",
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true
  },
  "include": ["src/**/*.ts", "src/**/*.svelte"]
}
```

- [ ] **Step 6: Create index.html**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ScreenSnap</title>
    <link rel="stylesheet" href="/src/styles/global.css" />
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 7: Create src/styles/global.css**

```css
/* screen/src/styles/global.css */
*, *::before, *::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: transparent;
  user-select: none;
  -webkit-user-select: none;
}

#app {
  width: 100%;
  height: 100%;
}
```

- [ ] **Step 8: Create src/main.ts**

```ts
// screen/src/main.ts
import App from "./App.svelte";
import { mount } from "svelte";

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
```

- [ ] **Step 9: Create src/App.svelte**

```svelte
<!-- screen/src/App.svelte -->
<script lang="ts">
  // Placeholder — will route to Overlay or Preferences based on window label
</script>

<main>
  <p>ScreenSnap loading...</p>
</main>

<style>
  main {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-family: system-ui, sans-serif;
  }
</style>
```

- [ ] **Step 10: Update package.json scripts**

Replace the `scripts` section in `package.json` with:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  }
}
```

- [ ] **Step 11: Verify Vite builds**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds to `dist/` with no errors

- [ ] **Step 12: Commit**

```bash
git add package.json package-lock.json vite.config.ts svelte.config.js tsconfig.json index.html src/
git commit -m "feat: scaffold Svelte + Vite frontend for Tauri"
```

---

### Task 2: Scaffold Tauri App Crate

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/commands.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create src-tauri/Cargo.toml**

```toml
# screen/src-tauri/Cargo.toml
[package]
name = "screensnap-app"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["macos-private-api"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
screen-core = { path = "../crates/screen-core" }
base64 = "0.22"
image = "0.25"
```

- [ ] **Step 2: Create src-tauri/build.rs**

```rust
// screen/src-tauri/build.rs
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 3: Create src-tauri/tauri.conf.json**

```json
{
  "$schema": "https://raw.githubusercontent.com/nicoswan/nicoswan-tauri/refs/heads/main/packages/api/schema.json",
  "productName": "ScreenSnap",
  "version": "0.1.0",
  "identifier": "com.screensnap.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "macOSPrivateApi": true,
    "windows": []
  }
}
```

Note: We start with no windows defined — the overlay window will be created dynamically from Rust when a capture is triggered.

- [ ] **Step 4: Create src-tauri/capabilities/default.json**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for ScreenSnap",
  "windows": ["*"],
  "permissions": [
    "core:default",
    "core:window:default",
    "core:window:allow-create",
    "core:window:allow-close",
    "core:window:allow-set-focus",
    "core:webview:default",
    "core:webview:allow-create-webview-window",
    "shell:allow-open"
  ]
}
```

- [ ] **Step 5: Create src-tauri/src/commands.rs**

```rust
// screen/src-tauri/src/commands.rs
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Capture fullscreen and return base64-encoded PNG for the overlay background.
#[tauri::command]
pub fn capture_screen(display: usize) -> Result<String, String> {
    let image = screen_core::capture::capture_fullscreen(display)
        .map_err(|e| e.to_string())?;

    let bytes = screen_core::format::encode_image(
        &image,
        screen_core::types::CaptureFormat::Png,
        90,
    )
    .map_err(|e| e.to_string())?;

    Ok(STANDARD.encode(&bytes))
}

/// Save a region of the captured screen to disk.
#[tauri::command]
pub fn save_region(
    display: usize,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let region = screen_core::types::Rect { x, y, width, height };
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    let format = config.screenshot_format;
    let quality = config.jpg_quality;

    let path = screen_core::screenshot_region(
        display,
        region,
        &save_path,
        format,
        quality,
        config.auto_copy_clipboard,
    )
    .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}

/// Save a fullscreen capture to disk (no region selection).
#[tauri::command]
pub fn save_fullscreen(display: usize) -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    let format = config.screenshot_format;
    let quality = config.jpg_quality;

    let path = screen_core::screenshot_fullscreen(
        display,
        &save_path,
        format,
        quality,
        config.auto_copy_clipboard,
    )
    .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}
```

- [ ] **Step 6: Create src-tauri/src/main.rs**

```rust
// screen/src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::capture_screen,
            commands::save_region,
            commands::save_fullscreen,
        ])
        .setup(|app| {
            // Create the overlay window on startup for now.
            // Later this will be triggered by hotkey/tray.
            let _overlay = tauri::WebviewWindowBuilder::new(
                app,
                "overlay",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("ScreenSnap Overlay")
            .fullscreen(true)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: Add src-tauri to workspace**

Add `"src-tauri"` to the workspace members in the root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/screen-core",
    "cli",
    "src-tauri",
]
```

- [ ] **Step 8: Create placeholder icon**

Run:
```bash
mkdir -p /Users/biplav00/Documents/personal/screen/src-tauri/icons
cd /Users/biplav00/Documents/personal/screen && npx @anthropic-ai/create-tauri-icons --input "" --output src-tauri/icons 2>/dev/null || true
```

If that fails, create a minimal 32x32 PNG icon manually:
```bash
python3 -c "
from PIL import Image
img = Image.new('RGBA', (32, 32), (0, 120, 255, 255))
img.save('src-tauri/icons/icon.png')
" 2>/dev/null || convert -size 32x32 xc:'#0078FF' src-tauri/icons/icon.png 2>/dev/null || true
```

The icons are optional for development — Tauri will use a default if none exist.

- [ ] **Step 9: Verify Tauri app compiles**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo build -p screensnap-app`
Expected: compiles with no errors (may take a while for first Tauri build)

- [ ] **Step 10: Verify Tauri dev runs**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo tauri dev`
Expected: Vite dev server starts, Tauri window appears showing "ScreenSnap loading..."

Press Ctrl+C to stop.

- [ ] **Step 11: Commit**

```bash
git add src-tauri/ Cargo.toml
git commit -m "feat: scaffold Tauri v2 app with overlay window and capture commands"
```

---

### Task 3: Overlay Component — Display Frozen Screenshot

**Files:**
- Modify: `src/App.svelte`
- Create: `src/lib/overlay/Overlay.svelte`

- [ ] **Step 1: Create Overlay.svelte**

```svelte
<!-- screen/src/lib/overlay/Overlay.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import RegionSelector from "./RegionSelector.svelte";

  let screenshotBase64 = $state("");
  let loading = $state(true);
  let error = $state("");
  let regionSelected = $state(false);

  const appWindow = getCurrentWebviewWindow();

  async function captureScreen() {
    try {
      screenshotBase64 = await invoke<string>("capture_screen", { display: 0 });
      loading = false;
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  function handleCancel() {
    appWindow.close();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      handleCancel();
    }
  }

  async function handleRegionSaved(path: string) {
    regionSelected = true;
    // Small delay to show success before closing
    setTimeout(() => appWindow.close(), 500);
  }

  captureScreen();
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  {#if loading}
    <div class="loading">Capturing screen...</div>
  {:else if error}
    <div class="error">{error}</div>
  {:else if regionSelected}
    <div class="success">Screenshot saved!</div>
  {:else}
    <img
      src="data:image/png;base64,{screenshotBase64}"
      alt="Screen capture"
      class="screenshot"
      draggable="false"
    />
    <RegionSelector
      onSave={handleRegionSaved}
      onCancel={handleCancel}
    />
  {/if}
</div>

<style>
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .screenshot {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
  }

  .loading, .error, .success {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    color: white;
    font-size: 24px;
    font-family: system-ui, sans-serif;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .error {
    color: #ff4444;
  }

  .success {
    color: #44ff44;
  }
</style>
```

- [ ] **Step 2: Update App.svelte to show Overlay**

```svelte
<!-- screen/src/App.svelte -->
<script lang="ts">
  import Overlay from "./lib/overlay/Overlay.svelte";
</script>

<Overlay />
```

- [ ] **Step 3: Verify overlay displays captured screenshot**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo tauri dev`
Expected: fullscreen transparent window appears showing the frozen screenshot of your screen. Press Escape to close.

- [ ] **Step 4: Commit**

```bash
git add src/App.svelte src/lib/overlay/Overlay.svelte
git commit -m "feat: add overlay component — captures and displays frozen screenshot"
```

---

### Task 4: Region Selector — Draw, Resize, Move

**Files:**
- Create: `src/lib/overlay/RegionSelector.svelte`

This is the core interaction component. The user draws a rectangle on the frozen screenshot, can resize via corner handles, and can drag to reposition.

- [ ] **Step 1: Create RegionSelector.svelte**

```svelte
<!-- screen/src/lib/overlay/RegionSelector.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface Props {
    onSave: (path: string) => void;
    onCancel: () => void;
  }

  let { onSave, onCancel }: Props = $props();

  // Region state
  let startX = $state(0);
  let startY = $state(0);
  let endX = $state(0);
  let endY = $state(0);
  let drawing = $state(false);
  let hasRegion = $state(false);

  // Drag/resize state
  let dragging = $state(false);
  let resizing = $state<string | null>(null);
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  // Saving state
  let saving = $state(false);

  const HANDLE_SIZE = 8;

  // Computed region bounds (normalized so width/height are always positive)
  let regionX = $derived(Math.min(startX, endX));
  let regionY = $derived(Math.min(startY, endY));
  let regionW = $derived(Math.abs(endX - startX));
  let regionH = $derived(Math.abs(endY - startY));

  // Dimension label
  let dimensionLabel = $derived(`${regionW} × ${regionH}`);

  function handleMouseDown(e: MouseEvent) {
    if (saving) return;

    // Check if clicking on a resize handle
    if (hasRegion) {
      const handle = getHandleAt(e.clientX, e.clientY);
      if (handle) {
        resizing = handle;
        return;
      }

      // Check if clicking inside region to drag
      if (isInsideRegion(e.clientX, e.clientY)) {
        dragging = true;
        dragOffsetX = e.clientX - regionX;
        dragOffsetY = e.clientY - regionY;
        return;
      }
    }

    // Start new region
    startX = e.clientX;
    startY = e.clientY;
    endX = e.clientX;
    endY = e.clientY;
    drawing = true;
    hasRegion = false;
  }

  function handleMouseMove(e: MouseEvent) {
    if (drawing) {
      endX = e.clientX;
      endY = e.clientY;
    } else if (dragging) {
      const newX = e.clientX - dragOffsetX;
      const newY = e.clientY - dragOffsetY;
      const dx = newX - regionX;
      const dy = newY - regionY;
      startX += dx;
      startY += dy;
      endX += dx;
      endY += dy;
    } else if (resizing) {
      applyResize(resizing, e.clientX, e.clientY);
    }
  }

  function handleMouseUp(_e: MouseEvent) {
    if (drawing) {
      drawing = false;
      if (regionW > 5 && regionH > 5) {
        hasRegion = true;
      }
    }
    dragging = false;
    resizing = null;
  }

  function isInsideRegion(x: number, y: number): boolean {
    return x >= regionX && x <= regionX + regionW &&
           y >= regionY && y <= regionY + regionH;
  }

  function getHandleAt(x: number, y: number): string | null {
    const handles = getHandlePositions();
    for (const [name, hx, hy] of handles) {
      if (Math.abs(x - hx) <= HANDLE_SIZE && Math.abs(y - hy) <= HANDLE_SIZE) {
        return name;
      }
    }
    return null;
  }

  function getHandlePositions(): [string, number, number][] {
    return [
      ["nw", regionX, regionY],
      ["ne", regionX + regionW, regionY],
      ["sw", regionX, regionY + regionH],
      ["se", regionX + regionW, regionY + regionH],
      ["n", regionX + regionW / 2, regionY],
      ["s", regionX + regionW / 2, regionY + regionH],
      ["w", regionX, regionY + regionH / 2],
      ["e", regionX + regionW, regionY + regionH / 2],
    ];
  }

  function applyResize(handle: string, mx: number, my: number) {
    if (handle.includes("n")) {
      if (startY < endY) startY = my; else endY = my;
    }
    if (handle.includes("s")) {
      if (startY < endY) endY = my; else startY = my;
    }
    if (handle.includes("w")) {
      if (startX < endX) startX = mx; else endX = mx;
    }
    if (handle.includes("e")) {
      if (startX < endX) endX = mx; else startX = mx;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && hasRegion && !saving) {
      saveRegion();
    }
  }

  async function saveRegion() {
    if (!hasRegion || saving) return;
    saving = true;

    try {
      // Account for device pixel ratio — coordinates from CSS pixels to physical pixels
      const dpr = window.devicePixelRatio || 1;
      const path = await invoke<string>("save_region", {
        display: 0,
        x: Math.round(regionX * dpr),
        y: Math.round(regionY * dpr),
        width: Math.round(regionW * dpr),
        height: Math.round(regionH * dpr),
      });
      onSave(path);
    } catch (err) {
      console.error("Failed to save:", err);
      saving = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="region-selector"
  onmousedown={handleMouseDown}
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
>
  <!-- Dark overlay with cutout for selected region -->
  {#if hasRegion || drawing}
    <svg class="dim-overlay" viewBox="0 0 {window.innerWidth} {window.innerHeight}">
      <defs>
        <mask id="region-mask">
          <rect width="100%" height="100%" fill="white" />
          <rect x={regionX} y={regionY} width={regionW} height={regionH} fill="black" />
        </mask>
      </defs>
      <rect width="100%" height="100%" fill="rgba(0,0,0,0.5)" mask="url(#region-mask)" />
    </svg>

    <!-- Region border -->
    <div
      class="region-border"
      style="left:{regionX}px;top:{regionY}px;width:{regionW}px;height:{regionH}px"
    ></div>

    <!-- Dimension label -->
    <div
      class="dimension-label"
      style="left:{regionX + regionW / 2}px;top:{regionY - 28}px"
    >
      {dimensionLabel}
    </div>
  {/if}

  <!-- Resize handles (only when region is finalized) -->
  {#if hasRegion && !drawing}
    {#each getHandlePositions() as [name, hx, hy]}
      <div
        class="handle handle-{name}"
        style="left:{hx - HANDLE_SIZE / 2}px;top:{hy - HANDLE_SIZE / 2}px;width:{HANDLE_SIZE}px;height:{HANDLE_SIZE}px"
      ></div>
    {/each}

    <!-- Action buttons -->
    <div
      class="actions"
      style="left:{regionX + regionW / 2}px;top:{regionY + regionH + 12}px"
    >
      <button class="btn btn-save" onclick={saveRegion} disabled={saving}>
        {saving ? "Saving..." : "Save (Enter)"}
      </button>
      <button class="btn btn-cancel" onclick={onCancel}>
        Cancel (Esc)
      </button>
    </div>
  {/if}
</div>

<style>
  .region-selector {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    cursor: crosshair;
    z-index: 10;
  }

  .dim-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    pointer-events: none;
  }

  .region-border {
    position: absolute;
    border: 2px solid #4a9eff;
    pointer-events: none;
    z-index: 11;
  }

  .dimension-label {
    position: absolute;
    transform: translateX(-50%);
    background: rgba(0, 0, 0, 0.75);
    color: white;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 12px;
    font-family: monospace;
    pointer-events: none;
    z-index: 12;
    white-space: nowrap;
  }

  .handle {
    position: absolute;
    background: #4a9eff;
    border: 1px solid white;
    border-radius: 2px;
    z-index: 12;
  }

  .handle-nw, .handle-se { cursor: nwse-resize; }
  .handle-ne, .handle-sw { cursor: nesw-resize; }
  .handle-n, .handle-s { cursor: ns-resize; }
  .handle-e, .handle-w { cursor: ew-resize; }

  .actions {
    position: absolute;
    transform: translateX(-50%);
    display: flex;
    gap: 8px;
    z-index: 12;
  }

  .btn {
    padding: 6px 16px;
    border: none;
    border-radius: 6px;
    font-size: 13px;
    font-family: system-ui, sans-serif;
    cursor: pointer;
    font-weight: 500;
  }

  .btn-save {
    background: #4a9eff;
    color: white;
  }

  .btn-save:hover {
    background: #3a8eef;
  }

  .btn-save:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-cancel {
    background: rgba(255, 255, 255, 0.15);
    color: white;
    backdrop-filter: blur(4px);
  }

  .btn-cancel:hover {
    background: rgba(255, 255, 255, 0.25);
  }
</style>
```

- [ ] **Step 2: Verify region selection works end-to-end**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo tauri dev`

Test the following:
1. Overlay appears with frozen screenshot
2. Draw a rectangle by clicking and dragging — region highlighted with dark overlay outside
3. Dimension label shows `W × H` above the region
4. Resize handles appear at corners and edges — drag them to resize
5. Click inside region and drag to reposition
6. Click outside region to start a new one
7. Press Enter or click "Save" — screenshot saved to the configured save directory
8. Press Escape — overlay closes

- [ ] **Step 3: Commit**

```bash
git add src/lib/overlay/RegionSelector.svelte
git commit -m "feat: add region selector — draw, resize, move with save/cancel"
```

---

### Task 5: Wire Overlay Launch from CLI

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/Cargo.toml`

When `screen capture` is run without `--fullscreen`, `--region`, or `--last-region`, launch the Tauri overlay app instead of printing an error.

- [ ] **Step 1: Update CLI to launch the Tauri app**

Replace the else branch at the end of `handle_capture` in `cli/src/main.rs`. The full updated file:

```rust
// screen/cli/src/main.rs
use clap::{Parser, Subcommand};
use screen_core::config::AppConfig;
use screen_core::types::{CaptureFormat, Rect};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "screen", about = "ScreenSnap — screenshot and screen recording tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Take a screenshot
    Capture {
        /// Capture the full screen
        #[arg(long)]
        fullscreen: bool,

        /// Use the last remembered region
        #[arg(long)]
        last_region: bool,

        /// Capture a specific region: x,y,width,height
        #[arg(long, value_parser = parse_region)]
        region: Option<Rect>,

        /// Output format: png, jpg, webp
        #[arg(long, short)]
        format: Option<String>,

        /// Output file path (overrides default)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Image quality for jpg/webp (1-100)
        #[arg(long, short, default_value = "90")]
        quality: u8,

        /// Display index (0 = main display)
        #[arg(long, short, default_value = "0")]
        display: usize,
    },
}

fn parse_region(s: &str) -> Result<Rect, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err("region must be x,y,width,height".to_string());
    }
    Ok(Rect {
        x: parts[0].trim().parse().map_err(|_| "invalid x")?,
        y: parts[1].trim().parse().map_err(|_| "invalid y")?,
        width: parts[2].trim().parse().map_err(|_| "invalid width")?,
        height: parts[3].trim().parse().map_err(|_| "invalid height")?,
    })
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Capture {
            fullscreen,
            last_region,
            region,
            format,
            output,
            quality,
            display,
        } => {
            if let Err(e) = handle_capture(fullscreen, last_region, region, format, output, quality, display) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    }
}

fn handle_capture(
    fullscreen: bool,
    last_region: bool,
    region: Option<Rect>,
    format_str: Option<String>,
    output: Option<PathBuf>,
    quality: u8,
    display: usize,
) -> Result<(), screen_core::ScreenError> {
    let config = AppConfig::load()?;

    let format = format_str
        .as_deref()
        .and_then(CaptureFormat::from_extension)
        .unwrap_or(config.screenshot_format);

    let save_path = output.unwrap_or_else(|| config.save_file_path());

    if let Some(region) = region {
        let path = screen_core::screenshot_region(display, region, &save_path, format, quality, config.auto_copy_clipboard)?;
        println!("Saved to: {}", path.display());

        if config.remember_last_region {
            let mut config = config;
            config.last_region = Some(screen_core::types::LastRegion { display, rect: region });
            let _ = config.save();
        }
    } else if last_region {
        match config.last_region {
            Some(last) => {
                let path = screen_core::screenshot_region(last.display, last.rect, &save_path, format, quality, config.auto_copy_clipboard)?;
                println!("Saved to: {}", path.display());
            }
            None => {
                eprintln!("No last region saved. Use --region or capture interactively first.");
                std::process::exit(1);
            }
        }
    } else if fullscreen {
        let path = screen_core::screenshot_fullscreen(display, &save_path, format, quality, config.auto_copy_clipboard)?;
        println!("Saved to: {}", path.display());
    } else {
        // Launch the Tauri overlay app for interactive region selection
        let exe = std::env::current_exe().unwrap_or_default();
        let app_dir = exe.parent().unwrap_or(std::path::Path::new("."));
        let app_path = app_dir.join("screensnap-app");

        if app_path.exists() {
            let status = std::process::Command::new(&app_path)
                .status()
                .map_err(|e| {
                    eprintln!("Failed to launch overlay: {}", e);
                    std::process::exit(1);
                })
                .unwrap();

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        } else {
            eprintln!("Interactive region selection requires the ScreenSnap app.");
            eprintln!("Use --fullscreen, --region, or --last-region for CLI-only capture.");
            std::process::exit(1);
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Verify CLI still works for direct capture**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo run -p screensnap -- capture --fullscreen --output /tmp/cli-test.png`
Expected: `Saved to: /tmp/cli-test.png`

- [ ] **Step 3: Verify all tests still pass**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: CLI launches Tauri overlay for interactive region selection"
```

---

### Task 6: End-to-End Manual Test and Polish

- [ ] **Step 1: Build release binaries**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo tauri build --debug`
Expected: builds the app bundle in `src-tauri/target/debug/bundle/`

- [ ] **Step 2: Run full end-to-end test**

Run the Tauri app directly: `cargo tauri dev`

Verify the full screenshot workflow:
1. App launches → overlay appears with frozen screenshot
2. Draw a region → dark overlay dims outside, blue border shows selection
3. Dimension label shows pixel size
4. Drag corner handles to resize
5. Click inside and drag to reposition
6. Press Enter → screenshot saved, overlay closes
7. Check the saved file exists in `~/Pictures/ScreenSnap/` (or configured directory)
8. Re-launch, press Escape → overlay closes without saving

- [ ] **Step 3: Fix any issues found during testing**

Address any visual glitches, coordinate mismatches, or interaction bugs.

- [ ] **Step 4: Run all tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test`
Expected: all tests pass

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: Phase 2a complete — Tauri overlay with interactive region selection"
```

---

## Phase 2a Summary

After completing all 6 tasks, you have:

- Svelte + Vite frontend scaffolded and building
- Tauri v2 app with transparent fullscreen overlay window
- Frozen screenshot displayed as overlay background
- Interactive region selector: draw, resize (8 handles), drag to reposition
- Dimension label showing live pixel size
- Save (Enter) and Cancel (Escape) actions
- Region saved via `screen-core` with format/clipboard support from config
- CLI falls back to launching the overlay for interactive capture

**Next phases:**
- Phase 2b: Annotation toolkit (canvas tools, toolbar, undo/redo)
- Phase 2c: System tray + global hotkeys + preferences window
- Phase 2d: Screen recording (FFmpeg integration)
