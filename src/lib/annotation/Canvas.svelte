<!-- src/lib/annotation/Canvas.svelte -->
<script lang="ts">
  import { getTool, renderAnnotation } from "./tools/registry.ts";
  import type { TextAnnotation } from "./tools/types.ts";
  import {
    annotations,
    activeAnnotation,
    activeTool,
    strokeColor,
    strokeWidth,
    setActiveAnnotation,
    commitAnnotation,
  } from "./state.svelte.ts";

  interface Props {
    regionX: number;
    regionY: number;
    regionW: number;
    regionH: number;
  }

  let { regionX, regionY, regionW, regionH }: Props = $props();

  let canvas: HTMLCanvasElement;
  let textInput: HTMLInputElement | undefined = $state(undefined);
  let textInputValue = $state("");
  let showTextInput = $state(false);
  let textInputX = $state(0);
  let textInputY = $state(0);

  // Re-render whenever annotations or activeAnnotation changes
  $effect(() => {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.clearRect(0, 0, regionW, regionH);

    // Render all committed annotations (coordinates relative to region)
    for (const a of annotations) {
      renderAnnotation(ctx, a);
    }

    // Render active (in-progress) annotation
    if (activeAnnotation && activeAnnotation.tool !== "text") {
      renderAnnotation(ctx, activeAnnotation);
    }
  });

  // Focus text input when it appears
  $effect(() => {
    if (showTextInput && textInput) {
      textInput.focus();
    }
  });

  function handleMouseDown(e: MouseEvent) {
    if (showTextInput) return; // Don't start new tool while editing text

    const x = e.clientX - regionX;
    const y = e.clientY - regionY;
    const tool = getTool(activeTool);
    tool.onMouseDown(x, y, {
      activeAnnotation,
      setActiveAnnotation,
      commitAnnotation,
      color: strokeColor,
      strokeWidth,
    });

    // If text tool just placed, show input
    if (activeTool === "text" && activeAnnotation && activeAnnotation.tool === "text") {
      showTextInput = true;
      textInputX = (activeAnnotation as TextAnnotation).x;
      textInputY = (activeAnnotation as TextAnnotation).y;
      textInputValue = "";
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (showTextInput) return;
    const x = e.clientX - regionX;
    const y = e.clientY - regionY;
    const tool = getTool(activeTool);
    tool.onMouseMove(x, y, {
      activeAnnotation,
      setActiveAnnotation,
      commitAnnotation,
      color: strokeColor,
      strokeWidth,
    });
  }

  function handleMouseUp(e: MouseEvent) {
    if (showTextInput) return;
    const x = e.clientX - regionX;
    const y = e.clientY - regionY;
    const tool = getTool(activeTool);
    tool.onMouseUp(x, y, {
      activeAnnotation,
      setActiveAnnotation,
      commitAnnotation,
      color: strokeColor,
      strokeWidth,
    });
  }

  function commitTextInput() {
    if (!activeAnnotation || activeAnnotation.tool !== "text") return;
    if (textInputValue.trim()) {
      commitAnnotation({
        ...activeAnnotation,
        text: textInputValue.trim(),
      } as TextAnnotation);
    } else {
      setActiveAnnotation(null);
    }
    showTextInput = false;
    textInputValue = "";
  }

  function handleTextKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitTextInput();
    } else if (e.key === "Escape") {
      e.preventDefault();
      setActiveAnnotation(null);
      showTextInput = false;
      textInputValue = "";
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="annotation-canvas-wrapper"
  style="left:{regionX}px;top:{regionY}px;width:{regionW}px;height:{regionH}px"
  onmousedown={handleMouseDown}
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
>
  <canvas
    bind:this={canvas}
    width={regionW}
    height={regionH}
    class="annotation-canvas"
  ></canvas>

  {#if showTextInput}
    <input
      bind:this={textInput}
      bind:value={textInputValue}
      class="text-input"
      style="left:{textInputX}px;top:{textInputY}px;color:{strokeColor}"
      onkeydown={handleTextKeydown}
      onblur={commitTextInput}
      placeholder="Type here..."
    />
  {/if}
</div>

<style>
  .annotation-canvas-wrapper {
    position: absolute;
    z-index: 15;
    cursor: crosshair;
  }

  .annotation-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }

  .text-input {
    position: absolute;
    background: transparent;
    border: 1px dashed rgba(255, 255, 255, 0.5);
    outline: none;
    font-size: 16px;
    font-family: system-ui, sans-serif;
    padding: 2px 4px;
    min-width: 100px;
    z-index: 20;
  }
</style>
