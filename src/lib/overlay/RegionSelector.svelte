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
