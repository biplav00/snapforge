<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import AnnotationCanvas from "../annotation/Canvas.svelte";
import { compositeImage } from "../annotation/compositing.ts";
import {
  annotations,
  clearAnnotations,
  offsetAnnotations,
  redo,
  setStrokeWidth,
  setTool,
  undo,
} from "../annotation/state.svelte.ts";
import Toolbar from "../annotation/Toolbar.svelte";
import type { ToolType } from "../annotation/tools/types.ts";

const appWindow = getCurrentWebviewWindow();

interface Props {
  purpose: "screenshot" | "record";
  display: number;
  onSave: (path: string) => void;
  onCopy: () => void;
  onRecordingStarted: (path: string) => void;
  onCancel: () => void;
}

let { purpose, display, onSave, onCopy, onRecordingStarted, onCancel }: Props = $props();

// Region state
let startX = $state(0);
let startY = $state(0);
let endX = $state(0);
let endY = $state(0);
let drawing = $state(false);
let hasRegion = $state(false);

// Mode: "select" = drawing region, "annotate" = annotation, "record-select" = recording options
let mode = $state<"select" | "annotate" | "record-select">("select");

// Pre-captured screenshot for blur/colorpicker tools
let screenshotBase64 = $state("");

// Initialize on mount — use $effect to properly capture props
let didInit = false;
$effect(() => {
  if (didInit) return;
  didInit = true;

  // Pre-select last region if passed via URL
  const lastRegionParam = new URLSearchParams(window.location.search).get("lastRegion");
  if (lastRegionParam) {
    const parts = lastRegionParam.split(",").map(Number);
    if (parts.length === 4 && parts.every((n) => !Number.isNaN(n))) {
      startX = parts[0];
      startY = parts[1];
      endX = parts[0] + parts[2];
      endY = parts[1] + parts[3];
      hasRegion = true;
      mode = purpose === "screenshot" ? "annotate" : "record-select";
    }
  }

  // Load pre-captured screenshot
  invoke<string>("get_pre_captured_screen", { display })
    .then((b64) => {
      screenshotBase64 = b64;
    })
    .catch(() => {});
});

// Drag/resize state
let dragging = $state(false);
let resizing = $state<string | null>(null);
let dragOffsetX = 0;
let dragOffsetY = 0;

// Saving state
let saving = $state(false);

// Toast
let toastMessage = $state("");
let toastVisible = $state(false);
let toastTimeout: ReturnType<typeof setTimeout> | null = null;

function showToast(msg: string) {
  toastMessage = msg;
  toastVisible = true;
  if (toastTimeout) clearTimeout(toastTimeout);
  toastTimeout = setTimeout(() => {
    toastVisible = false;
  }, 1500);
}

const HANDLE_SIZE = 6;

const SIZE_SHORTCUTS: Record<string, number> = {
  "1": 1,
  "2": 2,
  "3": 4,
};

const TOOL_SHORTCUTS: Record<string, ToolType> = {
  a: "arrow",
  r: "rect",
  c: "circle",
  l: "line",
  d: "dottedline",
  f: "freehand",
  t: "text",
  h: "highlight",
  b: "blur",
  n: "steps",
  k: "callout",
  i: "colorpicker",
  m: "measure",
};

// Computed region bounds
let regionX = $derived(Math.min(startX, endX));
let regionY = $derived(Math.min(startY, endY));
let regionW = $derived(Math.abs(endX - startX));
let regionH = $derived(Math.abs(endY - startY));
let dimensionLabel = $derived(`${regionW} × ${regionH}`);

// --- Region interaction helpers (shared by select & annotate modes) ---

function getHandleAt(x: number, y: number): string | null {
  const handles = getHandlePositions();
  for (const [name, hx, hy] of handles) {
    if (Math.abs(x - hx) <= HANDLE_SIZE + 2 && Math.abs(y - hy) <= HANDLE_SIZE + 2) return name;
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

function isInsideRegion(x: number, y: number): boolean {
  return x >= regionX && x <= regionX + regionW && y >= regionY && y <= regionY + regionH;
}

function isOnRegionEdge(x: number, y: number): boolean {
  const margin = 6;
  if (!isInsideRegion(x, y)) return false;
  return (
    x - regionX < margin ||
    regionX + regionW - x < margin ||
    y - regionY < margin ||
    regionY + regionH - y < margin
  );
}

function applyResize(handle: string, mx: number, my: number) {
  if (handle.includes("n")) {
    if (startY < endY) startY = my;
    else endY = my;
  }
  if (handle.includes("s")) {
    if (startY < endY) endY = my;
    else startY = my;
  }
  if (handle.includes("w")) {
    if (startX < endX) startX = mx;
    else endX = mx;
  }
  if (handle.includes("e")) {
    if (startX < endX) endX = mx;
    else startX = mx;
  }
}

// --- Mouse handlers ---

function handleMouseDown(e: MouseEvent) {
  if (saving) return;

  // In annotate mode: allow resize/drag on edges and handles, re-draw outside region
  if (mode === "annotate") {
    const handle = getHandleAt(e.clientX, e.clientY);
    if (handle) {
      resizing = handle;
      e.stopPropagation();
      return;
    }
    if (isOnRegionEdge(e.clientX, e.clientY)) {
      dragging = true;
      dragOffsetX = e.clientX - regionX;
      dragOffsetY = e.clientY - regionY;
      e.stopPropagation();
      return;
    }
    // Click outside region → start new selection
    if (!isInsideRegion(e.clientX, e.clientY)) {
      mode = "select";
      hasRegion = false;
      clearAnnotations();
      startX = e.clientX;
      startY = e.clientY;
      endX = e.clientX;
      endY = e.clientY;
      drawing = true;
      return;
    }
    // Inside region: let Canvas handle annotation tool
    return;
  }

  // In record-select mode: allow resize/drag on handles and edges
  if (mode === "record-select") {
    const handle = getHandleAt(e.clientX, e.clientY);
    if (handle) {
      resizing = handle;
      e.stopPropagation();
      return;
    }
    if (isOnRegionEdge(e.clientX, e.clientY)) {
      dragging = true;
      dragOffsetX = e.clientX - regionX;
      dragOffsetY = e.clientY - regionY;
      e.stopPropagation();
      return;
    }
    // Click outside region → start new selection
    if (!isInsideRegion(e.clientX, e.clientY)) {
      mode = "select";
      hasRegion = false;
      startX = e.clientX;
      startY = e.clientY;
      endX = e.clientX;
      endY = e.clientY;
      drawing = true;
      return;
    }
    return;
  }

  // Select mode
  if (hasRegion) {
    const handle = getHandleAt(e.clientX, e.clientY);
    if (handle) {
      resizing = handle;
      return;
    }
    if (isInsideRegion(e.clientX, e.clientY)) {
      dragging = true;
      dragOffsetX = e.clientX - regionX;
      dragOffsetY = e.clientY - regionY;
      return;
    }
  }

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
    const oldRX = regionX;
    const oldRY = regionY;
    const dx = e.clientX - dragOffsetX - regionX;
    const dy = e.clientY - dragOffsetY - regionY;
    startX += dx;
    startY += dy;
    endX += dx;
    endY += dy;
    if (mode === "annotate") {
      offsetAnnotations(oldRX - regionX, oldRY - regionY);
    }
  } else if (resizing) {
    const oldRX = regionX;
    const oldRY = regionY;
    applyResize(resizing, e.clientX, e.clientY);
    if (mode === "annotate") {
      offsetAnnotations(oldRX - regionX, oldRY - regionY);
    }
  }
}

function handleMouseUp(_e: MouseEvent) {
  if (drawing) {
    drawing = false;
    if (regionW > 5 && regionH > 5) {
      hasRegion = true;
      if (purpose === "screenshot") {
        enterAnnotateMode();
      } else {
        mode = "record-select";
      }
    }
  }
  dragging = false;
  resizing = null;
}

function enterAnnotateMode() {
  mode = "annotate";
  if (annotations.value.length === 0) {
    clearAnnotations();
  }
}

/** Hide overlay, wait for it to disappear, capture screen, return base64. */
async function captureWithOverlayHidden(): Promise<string> {
  try {
    const preCaptured = await invoke<string>("get_pre_captured_screen", { display });
    return preCaptured;
  } catch {
    await appWindow.hide();
    await new Promise((r) => setTimeout(r, 200));
    const screenshot = await invoke<string>("capture_screen", { display });
    return screenshot;
  }
}

function handleKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;

  if (mode === "record-select") {
    if (e.key === "Enter") {
      e.preventDefault();
      startRecordingRegion();
    }
    return;
  }

  if (mode === "annotate") {
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "z") {
      e.preventDefault();
      redo();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === "z") {
      e.preventDefault();
      undo();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key === "c") {
      e.preventDefault();
      handleCopy();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key === "s") {
      e.preventDefault();
      handleSave();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      handleSave();
      return;
    }
    if (!e.ctrlKey && !e.metaKey && !e.altKey) {
      const size = SIZE_SHORTCUTS[e.key];
      if (size !== undefined) {
        e.preventDefault();
        setStrokeWidth(size);
        return;
      }
      const tool = TOOL_SHORTCUTS[e.key.toLowerCase()];
      if (tool) {
        e.preventDefault();
        setTool(tool);
        return;
      }
    }
  }
}

/** Persist region to config so "capture last region" can restore it. */
async function rememberRegion() {
  const dpr = window.devicePixelRatio || 1;
  await invoke("save_last_region_to_config", {
    display,
    x: Math.round(regionX * dpr),
    y: Math.round(regionY * dpr),
    width: Math.round(regionW * dpr),
    height: Math.round(regionH * dpr),
  }).catch(() => {});
}

async function handleSave() {
  if (saving) return;
  saving = true;
  try {
    await rememberRegion();
    const screenshot = await captureWithOverlayHidden();
    const base64 = await compositeImage(
      screenshot,
      regionX,
      regionY,
      regionW,
      regionH,
      annotations.value,
    );
    const path = await invoke<string>("save_composited_image", { imageBase64: base64 });
    onSave(path);
  } catch (err) {
    console.error("Failed to save:", err);
    saving = false;
    await appWindow.show();
  }
}

async function handleCopy() {
  if (saving) return;
  saving = true;
  try {
    await rememberRegion();
    const screenshot = await captureWithOverlayHidden();
    const base64 = await compositeImage(
      screenshot,
      regionX,
      regionY,
      regionW,
      regionH,
      annotations.value.length > 0 ? annotations.value : [],
    );
    await invoke("copy_composited_image", { imageBase64: base64 });
    onCopy();
  } catch (err) {
    console.error("Failed to copy:", err);
    await appWindow.show();
    await appWindow.setFocus();
    showToast("Copy failed");
    saving = false;
  }
}

async function startRecordingRegion() {
  if (saving) return;
  saving = true;
  try {
    const dpr = window.devicePixelRatio || 1;
    const path = await invoke<string>("start_recording_and_show_indicator", {
      display,
      regionX: Math.round(regionX * dpr),
      regionY: Math.round(regionY * dpr),
      regionW: Math.round(regionW * dpr),
      regionH: Math.round(regionH * dpr),
    });
    onRecordingStarted(path);
  } catch (err) {
    console.error("Failed to start recording:", err);
    showToast(`Recording failed: ${err}`);
    saving = false;
  }
}

async function startRecordingFullscreen() {
  if (saving) return;
  saving = true;
  try {
    const path = await invoke<string>("start_recording_and_show_indicator", {
      display,
      regionX: null,
      regionY: null,
      regionW: null,
      regionH: null,
    });
    onRecordingStarted(path);
  } catch (err) {
    console.error("Failed to start recording:", err);
    showToast(`Recording failed: ${err}`);
    saving = false;
  }
}

// Cursor style based on hover position
function getCursor(e: MouseEvent): string {
  if (hasRegion) {
    const handle = getHandleAt(e.clientX, e.clientY);
    if (handle) {
      const cursors: Record<string, string> = {
        nw: "nwse-resize",
        se: "nwse-resize",
        ne: "nesw-resize",
        sw: "nesw-resize",
        n: "ns-resize",
        s: "ns-resize",
        w: "ew-resize",
        e: "ew-resize",
      };
      return cursors[handle] || "crosshair";
    }
    if (mode === "annotate" && isOnRegionEdge(e.clientX, e.clientY)) return "move";
  }
  return "crosshair";
}

let cursorStyle = $state("crosshair");

function handleGlobalMouseMove(e: MouseEvent) {
  cursorStyle = getCursor(e);
  handleMouseMove(e);
}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="region-selector"
  style="cursor:{cursorStyle}"
  onmousedown={handleMouseDown}
  onmousemove={handleGlobalMouseMove}
  onmouseup={handleMouseUp}
>
  <!-- Dark semi-transparent overlay with cutout for selected region -->
  {#if hasRegion || drawing}
    <svg class="dim-overlay" viewBox="0 0 {window.innerWidth} {window.innerHeight}">
      <defs>
        <mask id="region-mask">
          <rect width="100%" height="100%" fill="white" />
          <rect x={regionX} y={regionY} width={regionW} height={regionH} fill="black" />
        </mask>
      </defs>
      <rect width="100%" height="100%" fill="rgba(0,0,0,0.4)" mask="url(#region-mask)" />
    </svg>

    <!-- Region border: marching ants (white + dark dashed for visibility on all backgrounds) -->
    <div
      class="region-border"
      style="left:{regionX}px;top:{regionY}px;width:{regionW}px;height:{regionH}px"
    ></div>
    <div
      class="region-border-inner"
      style="left:{regionX}px;top:{regionY}px;width:{regionW}px;height:{regionH}px"
    ></div>

    <!-- Resize handles (shown always when region exists) -->
    {#if hasRegion && !drawing}
      {#each getHandlePositions() as [_name, hx, hy]}
        <div
          class="resize-handle"
          style="left:{hx}px;top:{hy}px"
        ></div>
      {/each}
    {/if}

    <!-- Dimension label -->
    <div
      class="dimension-label"
      style="left:{regionX + regionW / 2}px;top:{regionY - 24}px"
    >
      {dimensionLabel}
    </div>
  {:else}
    <div class="full-dim"></div>
  {/if}

  <!-- Toast message -->
  {#if toastVisible && hasRegion}
    <div
      class="toast"
      style="left:{regionX + regionW / 2}px;top:{regionY + regionH / 2}px"
    >
      {toastMessage}
    </div>
  {/if}

  <!-- Recording mode: region selected, show record options -->
  {#if mode === "record-select" && hasRegion}
    <div
      class="actions"
      style="left:{regionX + regionW / 2}px;top:{regionY + regionH + 12}px"
    >
      <button class="btn btn-record" onclick={startRecordingRegion} disabled={saving}>
        Record Region (Enter)
      </button>
      <button class="btn btn-record-full" onclick={startRecordingFullscreen} disabled={saving}>
        Record Fullscreen
      </button>
      <button class="btn btn-cancel" onclick={onCancel}>
        Cancel (Esc)
      </button>
    </div>
  {/if}

  <!-- Annotation mode (screenshot only) -->
  {#if mode === "annotate" && hasRegion}
    <AnnotationCanvas
      {regionX}
      {regionY}
      regionW={regionW}
      regionH={regionH}
      {screenshotBase64}
    />
    <Toolbar
      {regionX}
      {regionY}
      regionW={regionW}
      regionH={regionH}
      onSave={handleSave}
      onCopy={handleCopy}
      onCancel={onCancel}
    />
  {/if}
</div>

<style>
  .region-selector {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    z-index: 10;
  }

  .full-dim {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.15);
    pointer-events: none;
  }

  .dim-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    pointer-events: none;
  }

  /* Outer border: white dashed */
  .region-border {
    position: absolute;
    border: 1px dashed rgba(255, 255, 255, 0.9);
    pointer-events: none;
    z-index: 11;
  }

  /* Inner border: dark dashed offset — creates marching-ants visibility on any bg */
  .region-border-inner {
    position: absolute;
    border: 1px dashed rgba(0, 0, 0, 0.6);
    border-offset: 1px;
    pointer-events: none;
    z-index: 11;
    animation: march 0.4s linear infinite;
  }

  @keyframes march {
    to { border-dash-offset: 8px; }
  }

  .resize-handle {
    position: absolute;
    width: 8px;
    height: 8px;
    background: white;
    border: 1px solid rgba(0, 0, 0, 0.5);
    border-radius: 1px;
    transform: translate(-50%, -50%);
    z-index: 13;
    pointer-events: none;
  }

  .dimension-label {
    position: absolute;
    transform: translateX(-50%);
    background: rgba(0, 0, 0, 0.7);
    color: white;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 11px;
    font-family: monospace;
    pointer-events: none;
    z-index: 12;
    white-space: nowrap;
  }

  .toast {
    position: absolute;
    transform: translate(-50%, -50%);
    background: rgba(0, 0, 0, 0.8);
    color: #44ff44;
    padding: 8px 20px;
    border-radius: 8px;
    font-size: 15px;
    font-family: system-ui, sans-serif;
    font-weight: 500;
    pointer-events: none;
    z-index: 30;
    white-space: nowrap;
  }

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

  .btn-record {
    background: #ff4444;
    color: white;
  }

  .btn-record:hover { background: #ee3333; }
  .btn-record:disabled { opacity: 0.6; cursor: not-allowed; }

  .btn-record-full {
    background: rgba(255, 68, 68, 0.2);
    color: #ff6666;
    border: 1px solid rgba(255, 68, 68, 0.3);
  }

  .btn-record-full:hover { background: rgba(255, 68, 68, 0.4); color: white; }
  .btn-record-full:disabled { opacity: 0.6; cursor: not-allowed; }

  .btn-cancel {
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
  }

  .btn-cancel:hover { color: white; }
</style>
