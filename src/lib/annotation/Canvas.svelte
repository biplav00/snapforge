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
    setColor,
    nextStepNumber,
    incrementStepNumber,
  } from "./state.svelte.ts";

  interface Props {
    regionX: number;
    regionY: number;
    regionW: number;
    regionH: number;
    screenshotBase64: string;
  }

  let { regionX, regionY, regionW, regionH, screenshotBase64 }: Props = $props();

  let screenshotImg: HTMLImageElement | undefined = $state(undefined);

  $effect(() => {
    if (screenshotBase64) {
      const img = new Image();
      img.src = `data:image/png;base64,${screenshotBase64}`;
      img.onload = () => { screenshotImg = img; };
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
      // Read the annotation that was just set
      const ann = activeAnnotation.value;
      if (ann && ann.tool === "text") {
        showTextInput = true;
        textInputX = (ann as TextAnnotation).x;
        textInputY = (ann as TextAnnotation).y;
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
    if (!activeAnnotation.value || activeAnnotation.value.tool !== "text") {
      showTextInput = false;
      textInputValue = "";
      return;
    }
    if (textInputValue.trim()) {
      commitAnnotation({
        ...activeAnnotation.value,
        text: textInputValue.trim(),
      } as TextAnnotation);
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
