# ScreenSnap Phase 2b: Annotation Toolkit — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an annotation layer to the screenshot overlay with 5 drawing tools (Arrow, Rectangle, Line, Freehand, Text), a floating toolbar, undo/redo, and composited export.

**Architecture:** Annotations are stored as typed objects in a Svelte 5 runes-based state module. An HTML5 Canvas renders all annotations on top of the frozen screenshot. Each tool implements a common interface for mouse events and rendering. On save, an offscreen canvas composites the screenshot region + annotations into a single PNG and sends it to Rust for disk/clipboard output.

**Tech Stack:** Svelte 5 (runes), TypeScript, HTML5 Canvas, Tauri v2 commands

---

## File Structure

```
src/lib/annotation/
├── state.ts              # annotation state management (runes)
├── Canvas.svelte         # canvas renderer + mouse event delegation
├── Toolbar.svelte        # floating toolbar (tools, color, size, actions)
├── compositing.ts        # offscreen canvas compositing for export
└── tools/
    ├── types.ts          # Annotation union type, Tool interface, ToolType
    ├── registry.ts       # tool registry — maps ToolType to Tool impl
    ├── arrow.ts          # arrow tool (line + arrowhead)
    ├── rect.ts           # rectangle tool (outline)
    ├── line.ts           # line tool (straight line)
    ├── freehand.ts       # freehand tool (polyline from points)
    └── text.ts           # text tool (click to place, input for editing)

Modified existing files:
├── src/lib/overlay/Overlay.svelte       # pass screenshotBase64 to annotation layer
├── src/lib/overlay/RegionSelector.svelte # add annotation mode after region select
├── src-tauri/src/commands.rs            # add save_composited_image, copy_composited_image
├── src-tauri/src/main.rs                # register new commands
```

---

### Task 1: Annotation Types and Tool Interface

**Files:**
- Create: `src/lib/annotation/tools/types.ts`

- [ ] **Step 1: Create the types file**

```ts
// src/lib/annotation/tools/types.ts

export type ToolType = "arrow" | "rect" | "line" | "freehand" | "text";

export interface BaseAnnotation {
  id: string;
  tool: ToolType;
  color: string;
  strokeWidth: number;
}

export interface ArrowAnnotation extends BaseAnnotation {
  tool: "arrow";
  startX: number;
  startY: number;
  endX: number;
  endY: number;
}

export interface RectAnnotation extends BaseAnnotation {
  tool: "rect";
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface LineAnnotation extends BaseAnnotation {
  tool: "line";
  startX: number;
  startY: number;
  endX: number;
  endY: number;
}

export interface FreehandAnnotation extends BaseAnnotation {
  tool: "freehand";
  points: { x: number; y: number }[];
}

export interface TextAnnotation extends BaseAnnotation {
  tool: "text";
  x: number;
  y: number;
  text: string;
  fontSize: number;
}

export type Annotation =
  | ArrowAnnotation
  | RectAnnotation
  | LineAnnotation
  | FreehandAnnotation
  | TextAnnotation;

export interface AnnotationState {
  activeAnnotation: Annotation | null;
  setActiveAnnotation: (a: Annotation | null) => void;
  commitAnnotation: (a: Annotation) => void;
  color: string;
  strokeWidth: number;
}

export interface Tool {
  onMouseDown(x: number, y: number, state: AnnotationState): void;
  onMouseMove(x: number, y: number, state: AnnotationState): void;
  onMouseUp(x: number, y: number, state: AnnotationState): void;
  render(ctx: CanvasRenderingContext2D, annotation: Annotation): void;
}

let nextId = 0;
export function generateId(): string {
  return `ann_${++nextId}_${Date.now()}`;
}
```

- [ ] **Step 2: Verify Vite build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/annotation/tools/types.ts
git commit -m "feat: add annotation types and tool interface"
```

---

### Task 2: Annotation State Management

**Files:**
- Create: `src/lib/annotation/state.ts`

- [ ] **Step 1: Create the state module**

```ts
// src/lib/annotation/state.ts
import type { Annotation, ToolType } from "./tools/types.ts";

// Reactive state using module-level $state runes
export let annotations = $state<Annotation[]>([]);
export let activeTool = $state<ToolType>("arrow");
export let strokeColor = $state("#FF0000");
export let strokeWidth = $state(2);
export let activeAnnotation = $state<Annotation | null>(null);

let undoStack = $state<Annotation[][]>([]);
let redoStack = $state<Annotation[][]>([]);

export function setActiveAnnotation(a: Annotation | null) {
  activeAnnotation = a;
}

export function commitAnnotation(a: Annotation) {
  undoStack.push([...annotations]);
  redoStack = [];
  annotations.push(a);
  activeAnnotation = null;
}

export function undo() {
  if (undoStack.length === 0) return;
  redoStack.push([...annotations]);
  annotations = undoStack.pop()!;
}

export function redo() {
  if (redoStack.length === 0) return;
  undoStack.push([...annotations]);
  annotations = redoStack.pop()!;
}

export function clearAnnotations() {
  if (annotations.length === 0) return;
  undoStack.push([...annotations]);
  redoStack = [];
  annotations = [];
}

export function setTool(tool: ToolType) {
  activeTool = tool;
}

export function setColor(color: string) {
  strokeColor = color;
}

export function setStrokeWidth(width: number) {
  strokeWidth = width;
}

export function canUndo(): boolean {
  return undoStack.length > 0;
}

export function canRedo(): boolean {
  return redoStack.length > 0;
}
```

- [ ] **Step 2: Verify Vite build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/annotation/state.ts
git commit -m "feat: add annotation state management with undo/redo"
```

---

### Task 3: Tool Implementations — Arrow, Rectangle, Line, Freehand

**Files:**
- Create: `src/lib/annotation/tools/arrow.ts`
- Create: `src/lib/annotation/tools/rect.ts`
- Create: `src/lib/annotation/tools/line.ts`
- Create: `src/lib/annotation/tools/freehand.ts`
- Create: `src/lib/annotation/tools/registry.ts`

- [ ] **Step 1: Create arrow tool**

```ts
// src/lib/annotation/tools/arrow.ts
import type { Tool, AnnotationState, Annotation, ArrowAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const arrowTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "arrow",
      color: state.color,
      strokeWidth: state.strokeWidth,
      startX: x,
      startY: y,
      endX: x,
      endY: y,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as ArrowAnnotation | null;
    if (!a || a.tool !== "arrow") return;
    state.setActiveAnnotation({ ...a, endX: x, endY: y });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as ArrowAnnotation | null;
    if (!a || a.tool !== "arrow") return;
    const dx = a.endX - a.startX;
    const dy = a.endY - a.startY;
    if (Math.sqrt(dx * dx + dy * dy) > 3) {
      state.commitAnnotation(a);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as ArrowAnnotation;
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.lineCap = "round";

    // Draw line
    ctx.beginPath();
    ctx.moveTo(a.startX, a.startY);
    ctx.lineTo(a.endX, a.endY);
    ctx.stroke();

    // Draw arrowhead
    const angle = Math.atan2(a.endY - a.startY, a.endX - a.startX);
    const headLen = Math.max(10, a.strokeWidth * 4);
    ctx.fillStyle = a.color;
    ctx.beginPath();
    ctx.moveTo(a.endX, a.endY);
    ctx.lineTo(
      a.endX - headLen * Math.cos(angle - Math.PI / 6),
      a.endY - headLen * Math.sin(angle - Math.PI / 6),
    );
    ctx.lineTo(
      a.endX - headLen * Math.cos(angle + Math.PI / 6),
      a.endY - headLen * Math.sin(angle + Math.PI / 6),
    );
    ctx.closePath();
    ctx.fill();
  },
};
```

- [ ] **Step 2: Create rectangle tool**

```ts
// src/lib/annotation/tools/rect.ts
import type { Tool, AnnotationState, Annotation, RectAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const rectTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "rect",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      width: 0,
      height: 0,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as RectAnnotation | null;
    if (!a || a.tool !== "rect") return;
    state.setActiveAnnotation({
      ...a,
      width: x - a.x,
      height: y - a.y,
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as RectAnnotation | null;
    if (!a || a.tool !== "rect") return;
    if (Math.abs(a.width) > 3 && Math.abs(a.height) > 3) {
      // Normalize so width/height are positive
      const normalized: RectAnnotation = {
        ...a,
        x: a.width < 0 ? a.x + a.width : a.x,
        y: a.height < 0 ? a.y + a.height : a.y,
        width: Math.abs(a.width),
        height: Math.abs(a.height),
      };
      state.commitAnnotation(normalized);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as RectAnnotation;
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.lineJoin = "miter";
    ctx.strokeRect(a.x, a.y, a.width, a.height);
  },
};
```

- [ ] **Step 3: Create line tool**

```ts
// src/lib/annotation/tools/line.ts
import type { Tool, AnnotationState, Annotation, LineAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const lineTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "line",
      color: state.color,
      strokeWidth: state.strokeWidth,
      startX: x,
      startY: y,
      endX: x,
      endY: y,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as LineAnnotation | null;
    if (!a || a.tool !== "line") return;
    state.setActiveAnnotation({ ...a, endX: x, endY: y });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as LineAnnotation | null;
    if (!a || a.tool !== "line") return;
    const dx = a.endX - a.startX;
    const dy = a.endY - a.startY;
    if (Math.sqrt(dx * dx + dy * dy) > 3) {
      state.commitAnnotation(a);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as LineAnnotation;
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(a.startX, a.startY);
    ctx.lineTo(a.endX, a.endY);
    ctx.stroke();
  },
};
```

- [ ] **Step 4: Create freehand tool**

```ts
// src/lib/annotation/tools/freehand.ts
import type { Tool, AnnotationState, Annotation, FreehandAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const freehandTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "freehand",
      color: state.color,
      strokeWidth: state.strokeWidth,
      points: [{ x, y }],
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as FreehandAnnotation | null;
    if (!a || a.tool !== "freehand") return;
    state.setActiveAnnotation({
      ...a,
      points: [...a.points, { x, y }],
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as FreehandAnnotation | null;
    if (!a || a.tool !== "freehand") return;
    if (a.points.length > 2) {
      state.commitAnnotation(a);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as FreehandAnnotation;
    if (a.points.length < 2) return;
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.beginPath();
    ctx.moveTo(a.points[0].x, a.points[0].y);
    for (let i = 1; i < a.points.length; i++) {
      ctx.lineTo(a.points[i].x, a.points[i].y);
    }
    ctx.stroke();
  },
};
```

- [ ] **Step 5: Create tool registry**

```ts
// src/lib/annotation/tools/registry.ts
import type { Tool, ToolType } from "./types.ts";
import { arrowTool } from "./arrow.ts";
import { rectTool } from "./rect.ts";
import { lineTool } from "./line.ts";
import { freehandTool } from "./freehand.ts";

const tools: Record<ToolType, Tool> = {
  arrow: arrowTool,
  rect: rectTool,
  line: lineTool,
  freehand: freehandTool,
  text: arrowTool, // placeholder — text tool added in Task 4
};

export function getTool(type: ToolType): Tool {
  return tools[type];
}

export function renderAnnotation(ctx: CanvasRenderingContext2D, annotation: { tool: ToolType } & Record<string, unknown>) {
  const tool = tools[annotation.tool];
  if (tool) {
    tool.render(ctx, annotation as never);
  }
}
```

- [ ] **Step 6: Verify Vite build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 7: Commit**

```bash
git add src/lib/annotation/tools/
git commit -m "feat: add annotation tools — arrow, rect, line, freehand with registry"
```

---

### Task 4: Text Tool

**Files:**
- Create: `src/lib/annotation/tools/text.ts`
- Modify: `src/lib/annotation/tools/registry.ts`

The text tool is separate because it has unique behavior — it shows an HTML input overlay for editing rather than drawing directly on canvas.

- [ ] **Step 1: Create text tool**

```ts
// src/lib/annotation/tools/text.ts
import type { Tool, AnnotationState, Annotation, TextAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

// The text tool works differently from draw tools:
// - onMouseDown sets the placement position and signals the Canvas to show a text input
// - The Canvas component handles the actual input element and calls commitAnnotation
// - onMouseMove and onMouseUp are no-ops

export const textTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "text",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      text: "",
      fontSize: 16,
    });
  },

  onMouseMove(_x: number, _y: number, _state: AnnotationState) {
    // No-op — text doesn't drag
  },

  onMouseUp(_x: number, _y: number, _state: AnnotationState) {
    // No-op — text input is handled by Canvas component
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as TextAnnotation;
    if (!a.text) return;
    ctx.fillStyle = a.color;
    ctx.font = `${a.fontSize}px system-ui, sans-serif`;
    ctx.textBaseline = "top";
    ctx.fillText(a.text, a.x, a.y);
  },
};
```

- [ ] **Step 2: Update registry to use text tool**

Replace the placeholder in `src/lib/annotation/tools/registry.ts`:

```ts
// src/lib/annotation/tools/registry.ts
import type { Tool, ToolType } from "./types.ts";
import { arrowTool } from "./arrow.ts";
import { rectTool } from "./rect.ts";
import { lineTool } from "./line.ts";
import { freehandTool } from "./freehand.ts";
import { textTool } from "./text.ts";

const tools: Record<ToolType, Tool> = {
  arrow: arrowTool,
  rect: rectTool,
  line: lineTool,
  freehand: freehandTool,
  text: textTool,
};

export function getTool(type: ToolType): Tool {
  return tools[type];
}

export function renderAnnotation(ctx: CanvasRenderingContext2D, annotation: { tool: ToolType } & Record<string, unknown>) {
  const tool = tools[annotation.tool];
  if (tool) {
    tool.render(ctx, annotation as never);
  }
}
```

- [ ] **Step 3: Verify Vite build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 4: Commit**

```bash
git add src/lib/annotation/tools/text.ts src/lib/annotation/tools/registry.ts
git commit -m "feat: add text annotation tool"
```

---

### Task 5: Annotation Canvas Component

**Files:**
- Create: `src/lib/annotation/Canvas.svelte`

This component renders all annotations on an HTML5 Canvas and handles mouse event delegation to the active tool. It also renders the text input overlay when the text tool is active.

- [ ] **Step 1: Create Canvas.svelte**

```svelte
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
  } from "./state.ts";

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
```

- [ ] **Step 2: Verify Vite build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/annotation/Canvas.svelte
git commit -m "feat: add annotation canvas — renders annotations and delegates to tools"
```

---

### Task 6: Floating Toolbar Component

**Files:**
- Create: `src/lib/annotation/Toolbar.svelte`

- [ ] **Step 1: Create Toolbar.svelte**

```svelte
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
  } from "./state.ts";

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

  const TOOLS: { type: ToolType; label: string }[] = [
    { type: "arrow", label: "↗" },
    { type: "rect", label: "□" },
    { type: "line", label: "╱" },
    { type: "freehand", label: "✎" },
    { type: "text", label: "T" },
  ];

  // Position toolbar below region, or above if near screen bottom
  let toolbarTop = $derived(
    regionY + regionH + 48 > window.innerHeight
      ? regionY - 48
      : regionY + regionH + 8
  );
  let toolbarLeft = $derived(regionX + regionW / 2);

  let showColorPicker = $state(false);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="toolbar"
  style="left:{toolbarLeft}px;top:{toolbarTop}px"
  onmousedown|stopPropagation={() => {}}
>
  <!-- Tool buttons -->
  <div class="tool-group">
    {#each TOOLS as t}
      <button
        class="tool-btn"
        class:active={activeTool === t.type}
        onclick={() => setTool(t.type)}
        title={t.type}
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
      <div class="color-swatch-current" style="background:{strokeColor}"></div>
    </button>
    {#if showColorPicker}
      <div class="color-picker">
        {#each COLORS as c}
          <button
            class="color-swatch"
            class:active={strokeColor === c}
            style="background:{c}"
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
        class:active={strokeWidth === s.value}
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
    <button class="action-btn save-btn" onclick={onSave}>Save</button>
    <button class="action-btn copy-btn" onclick={onCopy}>Copy</button>
    <button class="action-btn cancel-btn" onclick={onCancel}>Cancel</button>
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
```

- [ ] **Step 2: Verify Vite build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/annotation/Toolbar.svelte
git commit -m "feat: add floating annotation toolbar — tools, color, size, undo/redo, actions"
```

---

### Task 7: Compositing Module and Tauri Commands

**Files:**
- Create: `src/lib/annotation/compositing.ts`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create compositing.ts**

```ts
// src/lib/annotation/compositing.ts
import { annotations } from "./state.ts";
import { renderAnnotation } from "./tools/registry.ts";

/**
 * Composite the screenshot region + annotations into a single PNG base64 string.
 *
 * @param screenshotBase64 - the full screenshot as base64 PNG
 * @param regionX - region X in CSS pixels
 * @param regionY - region Y in CSS pixels
 * @param regionW - region width in CSS pixels
 * @param regionH - region height in CSS pixels
 * @returns base64 PNG of the composited image at physical pixel resolution
 */
export async function compositeImage(
  screenshotBase64: string,
  regionX: number,
  regionY: number,
  regionW: number,
  regionH: number,
): Promise<string> {
  const dpr = window.devicePixelRatio || 1;
  const physW = Math.round(regionW * dpr);
  const physH = Math.round(regionH * dpr);
  const physX = Math.round(regionX * dpr);
  const physY = Math.round(regionY * dpr);

  // Load the full screenshot image
  const img = new Image();
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error("Failed to load screenshot"));
    img.src = `data:image/png;base64,${screenshotBase64}`;
  });

  // Create offscreen canvas at physical pixel resolution
  const canvas = document.createElement("canvas");
  canvas.width = physW;
  canvas.height = physH;
  const ctx = canvas.getContext("2d")!;

  // Draw cropped screenshot region
  ctx.drawImage(img, physX, physY, physW, physH, 0, 0, physW, physH);

  // Scale context for annotations (they use CSS pixel coordinates)
  ctx.scale(dpr, dpr);

  // Render all committed annotations
  for (const a of annotations) {
    renderAnnotation(ctx, a);
  }

  // Export as base64 PNG (strip the data:image/png;base64, prefix)
  const dataUrl = canvas.toDataURL("image/png");
  return dataUrl.replace(/^data:image\/png;base64,/, "");
}
```

- [ ] **Step 2: Add Tauri commands for composited save and copy**

Add these two commands to `src-tauri/src/commands.rs` (append after existing commands):

```rust
/// Save a composited image (screenshot + annotations) from base64 PNG.
#[tauri::command]
pub fn save_composited_image(image_base64: String) -> Result<String, String> {
    let bytes = STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("base64 decode failed: {}", e))?;

    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("image decode failed: {}", e))?;

    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let rgba = img.to_rgba8();
    screen_core::format::save_image(
        &rgba,
        &save_path,
        config.screenshot_format,
        config.jpg_quality,
    )
    .map_err(|e| e.to_string())?;

    Ok(save_path.display().to_string())
}

/// Copy a composited image (screenshot + annotations) to clipboard from base64 PNG.
#[tauri::command]
pub fn copy_composited_image(image_base64: String) -> Result<(), String> {
    let bytes = STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("base64 decode failed: {}", e))?;

    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("image decode failed: {}", e))?;

    let rgba = img.to_rgba8();
    screen_core::clipboard::copy_image_to_clipboard(&rgba)
        .map_err(|e| e.to_string())?;

    Ok(())
}
```

- [ ] **Step 3: Register new commands in main.rs**

Update `src-tauri/src/main.rs` to add the new commands to the invoke handler:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::capture_screen,
            commands::save_region,
            commands::save_fullscreen,
            commands::save_composited_image,
            commands::copy_composited_image,
        ])
        .setup(|app| {
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

- [ ] **Step 4: Verify builds**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build && cargo build -p screensnap-app`
Expected: both build with no errors

- [ ] **Step 5: Commit**

```bash
git add src/lib/annotation/compositing.ts src-tauri/src/commands.rs src-tauri/src/main.rs
git commit -m "feat: add compositing module and Tauri commands for annotated save/copy"
```

---

### Task 8: Integrate Annotation Layer into Overlay

**Files:**
- Modify: `src/lib/overlay/Overlay.svelte`
- Modify: `src/lib/overlay/RegionSelector.svelte`

This task wires the annotation canvas, toolbar, and compositing into the existing overlay flow. After the user draws a region, the overlay transitions to annotation mode where the toolbar and canvas appear.

- [ ] **Step 1: Update Overlay.svelte to pass screenshotBase64 and handle annotation save**

Replace the full content of `src/lib/overlay/Overlay.svelte`:

```svelte
<!-- src/lib/overlay/Overlay.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import RegionSelector from "./RegionSelector.svelte";

  let screenshotBase64 = $state("");
  let loading = $state(true);
  let error = $state("");
  let saved = $state(false);
  let savedMessage = $state("");

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

  function handleSaved(path: string) {
    saved = true;
    savedMessage = `Saved to: ${path}`;
    setTimeout(() => appWindow.close(), 800);
  }

  function handleCopied() {
    saved = true;
    savedMessage = "Copied to clipboard!";
    setTimeout(() => appWindow.close(), 800);
  }

  captureScreen();
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  {#if loading}
    <div class="status-msg loading">Capturing screen...</div>
  {:else if error}
    <div class="status-msg error">{error}</div>
  {:else if saved}
    <div class="status-msg success">{savedMessage}</div>
  {:else}
    <img
      src="data:image/png;base64,{screenshotBase64}"
      alt="Screen capture"
      class="screenshot"
      draggable="false"
    />
    <RegionSelector
      {screenshotBase64}
      onSave={handleSaved}
      onCopy={handleCopied}
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

  .status-msg {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    color: white;
    font-size: 24px;
    font-family: system-ui, sans-serif;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .error { color: #ff4444; }
  .success { color: #44ff44; }
</style>
```

- [ ] **Step 2: Update RegionSelector.svelte to support annotation mode**

Replace the full content of `src/lib/overlay/RegionSelector.svelte`:

```svelte
<!-- src/lib/overlay/RegionSelector.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import AnnotationCanvas from "../annotation/Canvas.svelte";
  import Toolbar from "../annotation/Toolbar.svelte";
  import { compositeImage } from "../annotation/compositing.ts";
  import { undo, redo, annotations, clearAnnotations } from "../annotation/state.ts";

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

  const HANDLE_SIZE = 8;

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
    if (mode === "select" && e.key === "Enter" && hasRegion && !saving) {
      enterAnnotateMode();
      return;
    }
    // Undo/redo in annotate mode
    if (mode === "annotate") {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "Z") {
        e.preventDefault();
        redo();
      } else if ((e.ctrlKey || e.metaKey) && e.key === "z") {
        e.preventDefault();
        undo();
      }
    }
  }

  async function handleSave() {
    if (saving) return;
    saving = true;
    try {
      let path: string;
      if (annotations.length > 0) {
        const base64 = await compositeImage(screenshotBase64, regionX, regionY, regionW, regionH);
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
      if (annotations.length > 0) {
        const base64 = await compositeImage(screenshotBase64, regionX, regionY, regionW, regionH);
        await invoke("copy_composited_image", { imageBase64: base64 });
      } else {
        // Save region then copy — use existing save_region which copies if config says so
        const dpr = window.devicePixelRatio || 1;
        await invoke<string>("save_region", {
          display: 0,
          x: Math.round(regionX * dpr),
          y: Math.round(regionY * dpr),
          width: Math.round(regionW * dpr),
          height: Math.round(regionH * dpr),
        });
      }
      onCopy();
    } catch (err) {
      console.error("Failed to copy:", err);
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

  <!-- Resize handles (select mode only) -->
  {#if hasRegion && !drawing && mode === "select"}
    {#each getHandlePositions() as [name, hx, hy]}
      <div
        class="handle handle-{name}"
        style="left:{hx - HANDLE_SIZE / 2}px;top:{hy - HANDLE_SIZE / 2}px;width:{HANDLE_SIZE}px;height:{HANDLE_SIZE}px"
      ></div>
    {/each}

    <!-- Select mode actions: Annotate or quick-save -->
    <div
      class="actions"
      style="left:{regionX + regionW / 2}px;top:{regionY + regionH + 12}px"
    >
      <button class="btn btn-annotate" onclick={enterAnnotateMode}>
        Annotate (Enter)
      </button>
      <button class="btn btn-save" onclick={handleSave} disabled={saving}>
        {saving ? "Saving..." : "Quick Save"}
      </button>
      <button class="btn btn-cancel" onclick={onCancel}>
        Cancel (Esc)
      </button>
    </div>
  {/if}

  <!-- Annotation mode -->
  {#if mode === "annotate" && hasRegion}
    <AnnotationCanvas
      {regionX}
      {regionY}
      regionW={regionW}
      regionH={regionH}
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

  .btn-annotate {
    background: #4a9eff;
    color: white;
  }

  .btn-annotate:hover {
    background: #3a8eef;
  }

  .btn-save {
    background: rgba(255, 255, 255, 0.15);
    color: white;
    backdrop-filter: blur(4px);
  }

  .btn-save:hover {
    background: rgba(255, 255, 255, 0.25);
  }

  .btn-save:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-cancel {
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
  }

  .btn-cancel:hover {
    color: white;
  }
</style>
```

- [ ] **Step 3: Verify both builds**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build && cargo build -p screensnap-app`
Expected: both build with no errors

- [ ] **Step 4: Run all Rust tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/lib/overlay/Overlay.svelte src/lib/overlay/RegionSelector.svelte
git commit -m "feat: integrate annotation layer into overlay with annotate mode"
```

---

### Task 9: End-to-End Verification

- [ ] **Step 1: Run full build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build && cargo build -p screensnap-app && cargo test`
Expected: frontend builds, Tauri compiles, all tests pass

- [ ] **Step 2: Manual E2E test**

Run: `cargo tauri dev`

Test the following workflow:
1. Overlay appears with frozen screenshot
2. Draw a region → "Annotate (Enter)", "Quick Save", and "Cancel" buttons appear
3. Click "Quick Save" → saves un-annotated region (same as Phase 2a)
4. Draw a new region → press Enter or click "Annotate"
5. Toolbar appears below region with tool buttons, color, size, undo/redo, save/copy/cancel
6. Select Arrow tool → draw an arrow inside the region
7. Select Rectangle tool → draw a rectangle
8. Select Freehand tool → draw freely
9. Select Text tool → click to place → type text → press Enter to commit
10. Click a different color → next annotation uses new color
11. Ctrl+Z → undo last annotation
12. Ctrl+Shift+Z → redo
13. Click "Save" → composited image saved to disk
14. Verify saved file contains both the screenshot region and annotations

- [ ] **Step 3: Fix any issues found during testing**

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: Phase 2b complete — annotation toolkit with 5 tools, toolbar, undo/redo"
```

---

## Phase 2b Summary

After completing all 9 tasks:

- 5 annotation tools: Arrow, Rectangle, Line, Freehand, Text
- Floating toolbar with tool selection, color palette (8 colors), stroke size (S/M/L)
- Undo/redo stack (Ctrl+Z / Ctrl+Shift+Z)
- Composited export: screenshot + annotations merged into single image
- Two save paths: "Quick Save" (no annotations) and "Save" (composited with annotations)
- Copy to clipboard support for annotated screenshots

**Next phases:**
- Phase 2b+: Additional tools (Circle, Blur, Highlight, Step Numbers, Color Picker, Measurement)
- Phase 2c: System tray + global hotkeys + preferences window
- Phase 2d: Screen recording (FFmpeg integration)
