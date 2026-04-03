<!-- src/lib/annotation/Toolbar.svelte -->
<script lang="ts">
  import type { ToolType } from "./tools/types.ts";
  import {
    activeTool,
    strokeColor,
    strokeWidth,
    setTool,
    setColor,
    setStrokeWidth,
    undo,
    redo,
    canUndo,
    canRedo,
  } from "./state.svelte.ts";

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
    "#FF0000", "#4A9EFF", "#00CC00", "#FFCC00",
    "#FF6600", "#9933FF", "#FFFFFF", "#000000",
  ];

  const SIZES: { label: string; value: number }[] = [
    { label: "S", value: 1 },
    { label: "M", value: 2 },
    { label: "L", value: 4 },
  ];

  const TOOLS: { type: ToolType; label: string; shortcut: string }[] = [
    { type: "arrow", label: "↗", shortcut: "A" },
    { type: "rect", label: "□", shortcut: "R" },
    { type: "circle", label: "○", shortcut: "C" },
    { type: "line", label: "╱", shortcut: "L" },
    { type: "freehand", label: "✎", shortcut: "F" },
    { type: "text", label: "T", shortcut: "T" },
    { type: "highlight", label: "▬", shortcut: "H" },
    { type: "blur", label: "▦", shortcut: "B" },
    { type: "steps", label: "①", shortcut: "N" },
    { type: "colorpicker", label: "◉", shortcut: "I" },
    { type: "measure", label: "📏", shortcut: "M" },
  ];

  // Position toolbar below region, or above if near screen bottom
  let toolbarTop = $derived(
    regionY + regionH + 48 > window.innerHeight
      ? regionY - 48
      : regionY + regionH + 8
  );
  let toolbarLeft = $derived(regionX + regionW / 2);

  let showColorPicker = $state(false);

  function stopPropagation(e: MouseEvent) {
    e.stopPropagation();
  }

  function handleWindowClick() {
    if (showColorPicker) showColorPicker = false;
  }
</script>

<svelte:window onclick={handleWindowClick} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="toolbar"
  style="left:{toolbarLeft}px;top:{toolbarTop}px"
  onmousedown={stopPropagation}
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
        {t.label}
      </button>
    {/each}
  </div>

  <div class="separator"></div>

  <!-- Color selector -->
  <div class="tool-group">
    <button
      class="tool-btn color-btn"
      onclick={() => (showColorPicker = !showColorPicker)}
      title="Color"
    >
      <div class="color-swatch-current" style="background:{strokeColor.value}"></div>
    </button>
    {#if showColorPicker}
      <div class="color-picker">
        {#each COLORS as c}
          <button
            class="color-swatch"
            class:active={strokeColor.value === c}
            style="background:{c}"
            title={c}
            onclick={() => { setColor(c); showColorPicker = false; }}
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
        title="{s.label} ({s.value}px)"
      >
        {s.label}
      </button>
    {/each}
  </div>

  <div class="separator"></div>

  <!-- Undo / Redo -->
  <div class="tool-group">
    <button class="tool-btn" onclick={undo} disabled={!canUndo()} title="Undo (Ctrl+Z)">↩</button>
    <button class="tool-btn" onclick={redo} disabled={!canRedo()} title="Redo (Ctrl+Shift+Z)">↪</button>
  </div>

  <div class="separator"></div>

  <!-- Actions -->
  <div class="tool-group">
    <button class="action-btn save-btn" onclick={onSave} title="Save (⌘S / Enter)">Save</button>
    <button class="action-btn copy-btn" onclick={onCopy} title="Copy (⌘C)">Copy</button>
    <button class="action-btn cancel-btn" onclick={onCancel} title="Cancel (Esc)">Cancel</button>
  </div>
</div>

<style>
  .toolbar {
    position: absolute;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 4px;
    background: rgba(30, 30, 30, 0.92);
    backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    padding: 4px 6px;
    z-index: 20;
    user-select: none;
  }

  .tool-group {
    display: flex;
    align-items: center;
    gap: 2px;
    position: relative;
  }

  .separator {
    width: 1px;
    height: 24px;
    background: rgba(255, 255, 255, 0.15);
    margin: 0 2px;
  }

  .tool-btn {
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.8);
    font-size: 14px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .tool-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .tool-btn.active {
    background: rgba(74, 158, 255, 0.3);
    color: #4a9eff;
  }

  .tool-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .color-btn {
    padding: 4px;
  }

  .color-swatch-current {
    width: 18px;
    height: 18px;
    border-radius: 3px;
    border: 1px solid rgba(255, 255, 255, 0.3);
  }

  .color-picker {
    position: absolute;
    bottom: 40px;
    left: 50%;
    transform: translateX(-50%);
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
    background: rgba(30, 30, 30, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    padding: 6px;
  }

  .color-swatch {
    width: 24px;
    height: 24px;
    border-radius: 4px;
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
    font-size: 11px;
    font-weight: 600;
    width: 28px;
  }

  .action-btn {
    height: 28px;
    padding: 0 12px;
    border: none;
    border-radius: 5px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .save-btn {
    background: #4a9eff;
    color: white;
  }

  .save-btn:hover {
    background: #3a8eef;
  }

  .copy-btn {
    background: rgba(255, 255, 255, 0.15);
    color: white;
  }

  .copy-btn:hover {
    background: rgba(255, 255, 255, 0.25);
  }

  .cancel-btn {
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
  }

  .cancel-btn:hover {
    color: white;
  }
</style>
