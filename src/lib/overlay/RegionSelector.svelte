<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import AnnotationCanvas from "../annotation/Canvas.svelte";
  import Toolbar from "../annotation/Toolbar.svelte";
  import { compositeImage } from "../annotation/compositing.ts";
  import { undo, redo, annotations, clearAnnotations, setTool } from "../annotation/state.svelte.ts";
  import type { ToolType } from "../annotation/tools/types.ts";

  interface Props {
    screenshotBase64: string;
    onSave: (path: string) => void;
    onCopy: () => void;
    onCancel: () => void;
  }

  let { screenshotBase64, onSave, onCopy, onCancel }: Props = $props();

  // Region state
  let startX = $state(0);
  let startY = $state(0);
  let endX = $state(0);
  let endY = $state(0);
  let drawing = $state(false);
  let hasRegion = $state(false);

  // Mode: "select" for drawing region, "annotate" for annotation
  let mode = $state<"select" | "annotate">("select");

  // Drag/resize state
  let dragging = $state(false);
  let resizing = $state<string | null>(null);
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  // Saving state
  let saving = $state(false);

  // Toast message shown inside region
  let toastMessage = $state("");
  let toastVisible = $state(false);
  let toastTimeout: ReturnType<typeof setTimeout> | null = null;

  function showToast(msg: string) {
    toastMessage = msg;
    toastVisible = true;
    if (toastTimeout) clearTimeout(toastTimeout);
    toastTimeout = setTimeout(() => { toastVisible = false; }, 1500);
  }

  const HANDLE_SIZE = 8;

  // Tool shortcut mapping
  const TOOL_SHORTCUTS: Record<string, ToolType> = {
    "1": "arrow",
    "2": "rect",
    "3": "circle",
    "4": "line",
    "5": "freehand",
    "6": "text",
    "7": "highlight",
    "8": "blur",
    "9": "steps",
    "0": "colorpicker",
    "a": "arrow",
    "r": "rect",
    "c": "circle",
    "l": "line",
    "f": "freehand",
    "t": "text",
    "h": "highlight",
    "b": "blur",
    "n": "steps",
    "i": "colorpicker",
    "m": "measure",
  };

  // Computed region bounds
  let regionX = $derived(Math.min(startX, endX));
  let regionY = $derived(Math.min(startY, endY));
  let regionW = $derived(Math.abs(endX - startX));
  let regionH = $derived(Math.abs(endY - startY));

  let dimensionLabel = $derived(`${regionW} × ${regionH}`);

  function handleMouseDown(e: MouseEvent) {
    if (saving || mode === "annotate") return;

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
    if (mode === "annotate") return;
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
    if (mode === "annotate") return;
    if (drawing) {
      drawing = false;
      if (regionW > 5 && regionH > 5) {
        hasRegion = true;
        // Go directly to annotate mode after selecting a region
        enterAnnotateMode();
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

  function enterAnnotateMode() {
    mode = "annotate";
    clearAnnotations();
  }

  function handleKeydown(e: KeyboardEvent) {
    // Don't intercept keys when typing in text input
    const target = e.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;

    // Undo/redo in annotate mode
    if (mode === "annotate") {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "Z") {
        e.preventDefault();
        redo();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "z") {
        e.preventDefault();
        undo();
        return;
      }
      // Ctrl/Cmd+C to copy
      if ((e.ctrlKey || e.metaKey) && e.key === "c") {
        e.preventDefault();
        handleCopy();
        return;
      }
      // Ctrl/Cmd+S to save
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        handleSave();
        return;
      }
      // Enter to save
      if (e.key === "Enter") {
        e.preventDefault();
        handleSave();
        return;
      }
      // Tool shortcuts (only without modifiers)
      if (!e.ctrlKey && !e.metaKey && !e.altKey) {
        const tool = TOOL_SHORTCUTS[e.key.toLowerCase()];
        if (tool) {
          e.preventDefault();
          setTool(tool);
          return;
        }
      }
    }
  }

  async function handleSave() {
    if (saving) return;
    saving = true;
    try {
      let path: string;
      if (annotations.value.length > 0) {
        const base64 = await compositeImage(screenshotBase64, regionX, regionY, regionW, regionH, annotations.value);
        path = await invoke<string>("save_composited_image", { imageBase64: base64 });
      } else {
        const dpr = window.devicePixelRatio || 1;
        path = await invoke<string>("save_region", {
          display: 0,
          x: Math.round(regionX * dpr),
          y: Math.round(regionY * dpr),
          width: Math.round(regionW * dpr),
          height: Math.round(regionH * dpr),
        });
      }
      onSave(path);
    } catch (err) {
      console.error("Failed to save:", err);
      saving = false;
    }
  }

  async function handleCopy() {
    if (saving) return;
    saving = true;
    try {
      const base64 = await compositeImage(
        screenshotBase64, regionX, regionY, regionW, regionH,
        annotations.value.length > 0 ? annotations.value : [],
      );
      await invoke("copy_composited_image", { imageBase64: base64 });
      showToast("Copied to clipboard!");
    } catch (err) {
      console.error("Failed to copy:", err);
      showToast("Copy failed");
    }
    saving = false;
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
  <!-- Dark overlay with cutout -->
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

  <!-- Toast message -->
  {#if toastVisible && hasRegion}
    <div
      class="toast"
      style="left:{regionX + regionW / 2}px;top:{regionY + regionH / 2}px"
    >
      {toastMessage}
    </div>
  {/if}

  <!-- Annotation mode -->
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
</style>
