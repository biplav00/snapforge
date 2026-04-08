<!-- src/lib/annotation/Toolbar.svelte -->
<script lang="ts">
import {
  activeTool,
  canRedo,
  canUndo,
  redo,
  setColor,
  setStrokeWidth,
  setTool,
  strokeColor,
  strokeWidth,
  undo,
} from "./state.svelte.ts";
import type { ToolType } from "./tools/types.ts";

interface Props {
  regionX: number;
  regionY: number;
  regionW: number;
  regionH: number;
  onSave: () => void;
  onCopy: () => void;
  onCancel: () => void;
}

let { regionX, regionY, regionW, regionH, onSave, onCopy, onCancel }: Props = $props();

const COLORS = [
  "#FF0000",
  "#4A9EFF",
  "#00CC00",
  "#FFCC00",
  "#FF6600",
  "#9933FF",
  "#FFFFFF",
  "#000000",
];

const SIZES: { label: string; value: number; shortcut: string }[] = [
  { label: "S", value: 1, shortcut: "1" },
  { label: "M", value: 2, shortcut: "2" },
  { label: "L", value: 4, shortcut: "3" },
];

const TOOLS: { type: ToolType; icon: string; shortcut: string }[] = [
  { type: "arrow", icon: "→", shortcut: "A" },
  { type: "rect", icon: "□", shortcut: "R" },
  { type: "circle", icon: "○", shortcut: "C" },
  { type: "line", icon: "/", shortcut: "L" },
  { type: "dottedline", icon: "┄", shortcut: "D" },
  { type: "freehand", icon: "~", shortcut: "F" },
  { type: "text", icon: "T", shortcut: "T" },
  { type: "highlight", icon: "▬", shortcut: "H" },
  { type: "blur", icon: "▧", shortcut: "B" },
  { type: "steps", icon: "#", shortcut: "N" },
  { type: "callout", icon: "❶", shortcut: "K" },
  { type: "colorpicker", icon: "⊙", shortcut: "I" },
  { type: "measure", icon: "↔", shortcut: "M" },
];

// Position toolbar — initial position below/above region, then draggable
let defaultTop = regionY + regionH + 40 > window.innerHeight ? regionY - 36 : regionY + regionH + 6;
let defaultLeft = regionX + regionW / 2;

let toolbarTop = $state(defaultTop);
let toolbarLeft = $state(defaultLeft);

let showColorPicker = $state(false);

// Drag state
let isDragging = $state(false);
let dragStartX = 0;
let dragStartY = 0;
let dragInitLeft = 0;
let dragInitTop = 0;

function handleDragStart(e: MouseEvent) {
  e.stopPropagation();
  // Only drag from the toolbar background/gaps, not from buttons
  if ((e.target as HTMLElement).closest("button")) return;
  e.preventDefault();
  isDragging = true;
  dragStartX = e.clientX;
  dragStartY = e.clientY;
  dragInitLeft = toolbarLeft;
  dragInitTop = toolbarTop;
}

function handleDragMove(e: MouseEvent) {
  if (!isDragging) return;
  toolbarLeft = dragInitLeft + (e.clientX - dragStartX);
  toolbarTop = dragInitTop + (e.clientY - dragStartY);
}

function handleDragEnd() {
  isDragging = false;
}

function stopPropagation(e: MouseEvent) {
  e.stopPropagation();
}

function handleColorSwatchClick(e: MouseEvent, c: string) {
  e.stopPropagation();
  setColor(c);
  showColorPicker = false;
}

function toggleColorPicker(e: MouseEvent) {
  e.stopPropagation();
  showColorPicker = !showColorPicker;
}

function handleWindowClick() {
  if (showColorPicker) showColorPicker = false;
}
</script>

<svelte:window
  onclick={handleWindowClick}
  onmousemove={handleDragMove}
  onmouseup={handleDragEnd}
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="toolbar"
  class:dragging={isDragging}
  style="left:{toolbarLeft}px;top:{toolbarTop}px"
  onmousedown={handleDragStart}
  onclick={stopPropagation}
>
  <!-- Tool buttons -->
  <div class="tool-group">
    {#each TOOLS as t}
      <button
        class="tool-btn"
        class:active={activeTool.value === t.type}
        onclick={() => setTool(t.type)}
        title="{t.type} ({t.shortcut})"
      >
        {t.icon}
      </button>
    {/each}
  </div>

  <div class="sep"></div>

  <!-- Color selector -->
  <div class="tool-group">
    <button
      class="tool-btn color-btn"
      onclick={toggleColorPicker}
      title="Color"
    >
      <div class="color-dot" style="background:{strokeColor.value}"></div>
    </button>
    {#if showColorPicker}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="color-picker" onclick={stopPropagation}>
        {#each COLORS as c}
          <button
            class="color-swatch"
            class:active={strokeColor.value === c}
            style="background:{c}"
            title={c}
            onclick={(e) => handleColorSwatchClick(e, c)}
          ></button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Stroke size -->
  <div class="tool-group">
    {#each SIZES as s}
      <button
        class="tool-btn size-btn"
        class:active={strokeWidth.value === s.value}
        onclick={() => setStrokeWidth(s.value)}
        title="{s.label} size ({s.shortcut})"
      >
        <span class="size-dot" style="width:{4 + s.value * 3}px;height:{4 + s.value * 3}px"></span>
      </button>
    {/each}
  </div>

  <div class="sep"></div>

  <!-- Undo / Redo -->
  <div class="tool-group">
    <button class="tool-btn" onclick={undo} disabled={!canUndo()} title="Undo (⌘Z)">↩</button>
    <button class="tool-btn" onclick={redo} disabled={!canRedo()} title="Redo (⌘⇧Z)">↪</button>
  </div>

  <div class="sep"></div>

  <!-- Actions -->
  <div class="tool-group actions">
    <button class="act-btn save" onclick={onSave} title="Save (⌘S)">Save</button>
    <button class="act-btn copy" onclick={onCopy} title="Copy (⌘C)">Copy</button>
    <button class="act-btn cancel" onclick={onCancel} title="Cancel (Esc)">✕</button>
  </div>
</div>

<style>
  .toolbar {
    position: absolute;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 2px;
    background: rgba(24, 24, 24, 0.94);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 2px 4px;
    z-index: 20;
    user-select: none;
    cursor: grab;
  }

  .toolbar.dragging {
    cursor: grabbing;
    opacity: 0.9;
  }

  .tool-group {
    display: flex;
    align-items: center;
    gap: 1px;
    position: relative;
  }

  .sep {
    width: 1px;
    height: 20px;
    background: rgba(255, 255, 255, 0.12);
    margin: 0 2px;
  }

  .tool-btn {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: rgba(255, 255, 255, 0.75);
    font-size: 14px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    line-height: 1;
    font-family: system-ui, -apple-system, sans-serif;
  }

  .tool-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .tool-btn.active {
    background: rgba(74, 158, 255, 0.35);
    color: #6ab4ff;
  }

  .tool-btn:disabled {
    opacity: 0.25;
    cursor: not-allowed;
  }

  .color-btn {
    padding: 3px;
  }

  .color-dot {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.4);
  }

  .color-picker {
    position: absolute;
    bottom: 34px;
    left: 50%;
    transform: translateX(-50%);
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 3px;
    background: rgba(24, 24, 24, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 5px;
    z-index: 25;
  }

  .color-swatch {
    width: 20px;
    height: 20px;
    border-radius: 3px;
    border: 2px solid transparent;
    cursor: pointer;
  }

  .color-swatch.active {
    border-color: white;
  }

  .color-swatch:hover {
    border-color: rgba(255, 255, 255, 0.5);
  }

  .size-btn {
    width: 24px;
  }

  .size-dot {
    display: block;
    border-radius: 50%;
    background: currentColor;
  }

  .actions {
    gap: 3px;
  }

  .act-btn {
    height: 22px;
    padding: 0 8px;
    border: none;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, -apple-system, sans-serif;
  }

  .save {
    background: #4a9eff;
    color: white;
  }
  .save:hover { background: #3a8eef; }

  .copy {
    background: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.85);
  }
  .copy:hover { background: rgba(255, 255, 255, 0.22); color: white; }

  .cancel {
    background: transparent;
    color: rgba(255, 255, 255, 0.4);
    padding: 0 5px;
    font-size: 12px;
  }
  .cancel:hover { color: white; }
</style>
