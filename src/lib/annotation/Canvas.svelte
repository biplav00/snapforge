<!-- src/lib/annotation/Canvas.svelte -->
<script lang="ts">
import {
  activeAnnotation,
  activeTool,
  annotations,
  commitAnnotation,
  incrementStepNumber,
  nextStepNumber,
  setActiveAnnotation,
  setColor,
  strokeColor,
  strokeWidth,
} from "./state.svelte.ts";
import { getTool, renderAnnotation } from "./tools/registry.ts";
import type { TextAnnotation } from "./tools/types.ts";

interface Props {
  regionX: number;
  regionY: number;
  regionW: number;
  regionH: number;
  screenshotBase64?: string;
}

let { regionX, regionY, regionW, regionH, screenshotBase64 = "" }: Props = $props();

let screenshotImg: HTMLImageElement | undefined = $state(undefined);

$effect(() => {
  if (screenshotBase64) {
    const img = new Image();
    img.src = `data:image/png;base64,${screenshotBase64}`;
    img.onload = () => {
      screenshotImg = img;
    };
  }
});

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

  // Draw the screenshot region as background so blur/colorpicker can read pixels
  if (screenshotImg) {
    const dpr = window.devicePixelRatio || 1;
    const sx = Math.round(regionX * dpr);
    const sy = Math.round(regionY * dpr);
    const sw = Math.round(regionW * dpr);
    const sh = Math.round(regionH * dpr);
    ctx.drawImage(screenshotImg, sx, sy, sw, sh, 0, 0, regionW, regionH);
  }

  for (const a of annotations.value) {
    renderAnnotation(ctx, a);
  }

  if (activeAnnotation.value && activeAnnotation.value.tool !== "text") {
    renderAnnotation(ctx, activeAnnotation.value);
  }
});

// Focus text input when it appears
$effect(() => {
  if (showTextInput && textInput) {
    // Use microtask to ensure DOM is ready
    queueMicrotask(() => textInput?.focus());
  }
});

function makeState() {
  return {
    activeAnnotation: activeAnnotation.value,
    setActiveAnnotation,
    commitAnnotation,
    color: strokeColor.value,
    strokeWidth: strokeWidth.value,
    screenshotImage: screenshotImg,
    setColor,
    nextStepNumber: nextStepNumber.value,
    incrementStepNumber,
  };
}

function handleMouseDown(e: MouseEvent) {
  if (showTextInput) {
    // If clicking outside the text input, commit current text
    commitTextInput();
    return;
  }

  const x = e.clientX - regionX;
  const y = e.clientY - regionY;

  if (activeTool.value === "text") {
    // Handle text tool specially — set annotation and show input immediately
    const tool = getTool("text");
    tool.onMouseDown(x, y, makeState());
    const ann = activeAnnotation.value;
    if (ann && ann.tool === "text") {
      showTextInput = true;
      textInputX = (ann as TextAnnotation).x;
      textInputY = (ann as TextAnnotation).y;
      textInputValue = "";
    }
  } else if (activeTool.value === "callout") {
    const tool = getTool("callout");
    tool.onMouseDown(x, y, makeState());
    const ann = activeAnnotation.value;
    if (ann && ann.tool === "callout") {
      showTextInput = true;
      textInputX = ann.x + 28; // offset to the right of the numbered circle
      textInputY = ann.y - 12;
      textInputValue = "";
    }
  } else {
    const tool = getTool(activeTool.value);
    tool.onMouseDown(x, y, makeState());
  }
}

function handleMouseMove(e: MouseEvent) {
  if (showTextInput) return;
  const x = e.clientX - regionX;
  const y = e.clientY - regionY;
  const tool = getTool(activeTool.value);
  tool.onMouseMove(x, y, makeState());
}

function handleMouseUp(e: MouseEvent) {
  if (showTextInput) return;
  const x = e.clientX - regionX;
  const y = e.clientY - regionY;
  const tool = getTool(activeTool.value);
  tool.onMouseUp(x, y, makeState());
}

function commitTextInput() {
  const ann = activeAnnotation.value;
  if (!ann) {
    showTextInput = false;
    textInputValue = "";
    return;
  }

  if (ann.tool === "text") {
    if (textInputValue.trim()) {
      commitAnnotation({ ...ann, text: textInputValue.trim() } as TextAnnotation);
    } else {
      setActiveAnnotation(null);
    }
  } else if (ann.tool === "callout") {
    // Commit callout with label (empty label is fine — just shows numbered circle)
    commitAnnotation({
      ...ann,
      label: textInputValue.trim(),
    } as import("./tools/types.ts").CalloutAnnotation);
    if (incrementStepNumber) incrementStepNumber();
  } else {
    setActiveAnnotation(null);
  }

  showTextInput = false;
  textInputValue = "";
}

function handleTextKeydown(e: KeyboardEvent) {
  // Stop propagation so tool shortcuts don't fire while typing
  e.stopPropagation();
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

function handleTextInput(e: Event) {
  // Stop propagation for all input events
  e.stopPropagation();
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
    <!-- svelte-ignore a11y_autofocus -->
    <input
      bind:this={textInput}
      bind:value={textInputValue}
      class="text-input"
      style="left:{textInputX}px;top:{textInputY}px;color:{strokeColor.value}"
      onkeydown={handleTextKeydown}
      oninput={handleTextInput}
      onmousedown={(e) => e.stopPropagation()}
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
    background: rgba(0, 0, 0, 0.3);
    border: 1px dashed rgba(255, 255, 255, 0.6);
    outline: none;
    font-size: 16px;
    font-family: system-ui, sans-serif;
    padding: 2px 6px;
    min-width: 120px;
    z-index: 20;
    border-radius: 3px;
  }
</style>
