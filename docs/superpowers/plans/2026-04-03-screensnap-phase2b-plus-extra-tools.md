# ScreenSnap Phase 2b+: Additional Annotation Tools — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 6 remaining annotation tools: Circle/Ellipse, Highlight, Step Numbers, Blur/Pixelate, Color Picker (eyedropper), and Measurement.

**Architecture:** Each tool follows the existing `Tool` interface pattern (onMouseDown/Move/Up + render). New annotation types are added to the union. The blur and color picker tools need access to the screenshot image data, so `Canvas.svelte` gains a `screenshotBase64` prop. The toolbar and shortcut map are extended with the new tools.

**Tech Stack:** TypeScript, HTML5 Canvas API, Svelte 5

---

## File Structure

```
New files:
  src/lib/annotation/tools/circle.ts      # ellipse tool
  src/lib/annotation/tools/highlight.ts    # semi-transparent filled rect
  src/lib/annotation/tools/steps.ts        # auto-incrementing numbered circles
  src/lib/annotation/tools/blur.ts         # pixelate a rectangular region
  src/lib/annotation/tools/colorpicker.ts  # eyedropper — pick color from screenshot
  src/lib/annotation/tools/measure.ts      # line with pixel distance label

Modified files:
  src/lib/annotation/tools/types.ts        # add 6 new annotation interfaces + extend unions
  src/lib/annotation/tools/registry.ts     # register all new tools
  src/lib/annotation/Toolbar.svelte        # add new tool buttons
  src/lib/annotation/Canvas.svelte         # add screenshotBase64 prop for blur/colorpicker
  src/lib/overlay/RegionSelector.svelte    # pass screenshotBase64 to Canvas, update shortcuts
```

---

### Task 1: Extend Types with New Annotation Interfaces

**Files:**
- Modify: `src/lib/annotation/tools/types.ts`

- [ ] **Step 1: Add new types and extend ToolType and Annotation unions**

Replace the entire file with:

```ts
// src/lib/annotation/tools/types.ts

export type ToolType = "arrow" | "rect" | "line" | "freehand" | "text" | "circle" | "highlight" | "steps" | "blur" | "colorpicker" | "measure";

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

export interface CircleAnnotation extends BaseAnnotation {
  tool: "circle";
  cx: number;
  cy: number;
  rx: number;
  ry: number;
}

export interface HighlightAnnotation extends BaseAnnotation {
  tool: "highlight";
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface StepAnnotation extends BaseAnnotation {
  tool: "steps";
  x: number;
  y: number;
  number: number;
}

export interface BlurAnnotation extends BaseAnnotation {
  tool: "blur";
  x: number;
  y: number;
  width: number;
  height: number;
  intensity: number;
}

export interface MeasureAnnotation extends BaseAnnotation {
  tool: "measure";
  startX: number;
  startY: number;
  endX: number;
  endY: number;
}

// ColorPicker doesn't produce an annotation — it changes the active color.
// We use a stub annotation to track the click position during the interaction.
export interface ColorPickerAnnotation extends BaseAnnotation {
  tool: "colorpicker";
  x: number;
  y: number;
}

export type Annotation =
  | ArrowAnnotation
  | RectAnnotation
  | LineAnnotation
  | FreehandAnnotation
  | TextAnnotation
  | CircleAnnotation
  | HighlightAnnotation
  | StepAnnotation
  | BlurAnnotation
  | MeasureAnnotation
  | ColorPickerAnnotation;

export interface AnnotationState {
  activeAnnotation: Annotation | null;
  setActiveAnnotation: (a: Annotation | null) => void;
  commitAnnotation: (a: Annotation) => void;
  color: string;
  strokeWidth: number;
  /** Screenshot image element — available for blur/colorpicker tools */
  screenshotImage?: HTMLImageElement;
  /** Callback to change the active stroke color (used by colorpicker) */
  setColor?: (color: string) => void;
  /** Current step counter (used by steps tool) */
  nextStepNumber?: number;
  /** Callback to increment step counter */
  incrementStepNumber?: () => void;
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

- [ ] **Step 2: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds (existing tools still work since their types are unchanged)

- [ ] **Step 3: Commit**

```bash
git add src/lib/annotation/tools/types.ts
git commit -m "feat: extend annotation types with circle, highlight, steps, blur, measure, colorpicker"
```

---

### Task 2: Circle, Highlight, and Step Number Tools

**Files:**
- Create: `src/lib/annotation/tools/circle.ts`
- Create: `src/lib/annotation/tools/highlight.ts`
- Create: `src/lib/annotation/tools/steps.ts`

- [ ] **Step 1: Create circle tool**

```ts
// src/lib/annotation/tools/circle.ts
import type { Tool, AnnotationState, Annotation, CircleAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const circleTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "circle",
      color: state.color,
      strokeWidth: state.strokeWidth,
      cx: x,
      cy: y,
      rx: 0,
      ry: 0,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as CircleAnnotation | null;
    if (!a || a.tool !== "circle") return;
    state.setActiveAnnotation({
      ...a,
      rx: Math.abs(x - a.cx),
      ry: Math.abs(y - a.cy),
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as CircleAnnotation | null;
    if (!a || a.tool !== "circle") return;
    if (a.rx > 3 && a.ry > 3) {
      state.commitAnnotation(a);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as CircleAnnotation;
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.beginPath();
    ctx.ellipse(a.cx, a.cy, a.rx, a.ry, 0, 0, Math.PI * 2);
    ctx.stroke();
  },
};
```

- [ ] **Step 2: Create highlight tool**

```ts
// src/lib/annotation/tools/highlight.ts
import type { Tool, AnnotationState, Annotation, HighlightAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const highlightTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "highlight",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      width: 0,
      height: 0,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as HighlightAnnotation | null;
    if (!a || a.tool !== "highlight") return;
    state.setActiveAnnotation({
      ...a,
      width: x - a.x,
      height: y - a.y,
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as HighlightAnnotation | null;
    if (!a || a.tool !== "highlight") return;
    if (Math.abs(a.width) > 3 && Math.abs(a.height) > 3) {
      const normalized: HighlightAnnotation = {
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
    const a = annotation as HighlightAnnotation;
    ctx.fillStyle = a.color;
    ctx.globalAlpha = 0.3;
    ctx.fillRect(a.x, a.y, a.width, a.height);
    ctx.globalAlpha = 1.0;
  },
};
```

- [ ] **Step 3: Create steps tool**

```ts
// src/lib/annotation/tools/steps.ts
import type { Tool, AnnotationState, Annotation, StepAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

const RADIUS = 14;

export const stepsTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    const num = state.nextStepNumber ?? 1;
    const annotation: StepAnnotation = {
      id: generateId(),
      tool: "steps",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      number: num,
    };
    state.commitAnnotation(annotation);
    if (state.incrementStepNumber) {
      state.incrementStepNumber();
    }
  },

  onMouseMove(_x: number, _y: number, _state: AnnotationState) {
    // No-op — steps are placed on click
  },

  onMouseUp(_x: number, _y: number, _state: AnnotationState) {
    // No-op
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as StepAnnotation;
    // Filled circle
    ctx.fillStyle = a.color;
    ctx.beginPath();
    ctx.arc(a.x, a.y, RADIUS, 0, Math.PI * 2);
    ctx.fill();

    // White number
    ctx.fillStyle = "white";
    ctx.font = `bold ${RADIUS}px system-ui, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(String(a.number), a.x, a.y);

    // Reset text alignment
    ctx.textAlign = "start";
    ctx.textBaseline = "alphabetic";
  },
};
```

- [ ] **Step 4: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds with no errors

- [ ] **Step 5: Commit**

```bash
git add src/lib/annotation/tools/circle.ts src/lib/annotation/tools/highlight.ts src/lib/annotation/tools/steps.ts
git commit -m "feat: add circle, highlight, and step number annotation tools"
```

---

### Task 3: Blur and Measure Tools

**Files:**
- Create: `src/lib/annotation/tools/blur.ts`
- Create: `src/lib/annotation/tools/measure.ts`

- [ ] **Step 1: Create blur tool**

```ts
// src/lib/annotation/tools/blur.ts
import type { Tool, AnnotationState, Annotation, BlurAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

const DEFAULT_INTENSITY = 10;

export const blurTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "blur",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      width: 0,
      height: 0,
      intensity: DEFAULT_INTENSITY,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as BlurAnnotation | null;
    if (!a || a.tool !== "blur") return;
    state.setActiveAnnotation({
      ...a,
      width: x - a.x,
      height: y - a.y,
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as BlurAnnotation | null;
    if (!a || a.tool !== "blur") return;
    if (Math.abs(a.width) > 3 && Math.abs(a.height) > 3) {
      const normalized: BlurAnnotation = {
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
    const a = annotation as BlurAnnotation;
    const x = Math.round(a.x);
    const y = Math.round(a.y);
    const w = Math.round(Math.abs(a.width));
    const h = Math.round(Math.abs(a.height));
    if (w <= 0 || h <= 0) return;

    const blockSize = Math.max(2, Math.round(a.intensity));

    // Read pixels from the canvas, pixelate them, write back
    try {
      const imageData = ctx.getImageData(x, y, w, h);
      const data = imageData.data;

      for (let by = 0; by < h; by += blockSize) {
        for (let bx = 0; bx < w; bx += blockSize) {
          // Sample center pixel of block
          const sx = Math.min(bx + Math.floor(blockSize / 2), w - 1);
          const sy = Math.min(by + Math.floor(blockSize / 2), h - 1);
          const si = (sy * w + sx) * 4;
          const r = data[si];
          const g = data[si + 1];
          const b = data[si + 2];
          const alpha = data[si + 3];

          // Fill block with sampled color
          for (let dy = by; dy < Math.min(by + blockSize, h); dy++) {
            for (let dx = bx; dx < Math.min(bx + blockSize, w); dx++) {
              const di = (dy * w + dx) * 4;
              data[di] = r;
              data[di + 1] = g;
              data[di + 2] = b;
              data[di + 3] = alpha;
            }
          }
        }
      }

      ctx.putImageData(imageData, x, y);
    } catch {
      // Canvas tainted or out of bounds — draw a hatched rect as fallback
      ctx.strokeStyle = "rgba(128,128,128,0.5)";
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.strokeRect(x, y, w, h);
      ctx.setLineDash([]);
    }
  },
};
```

- [ ] **Step 2: Create measure tool**

```ts
// src/lib/annotation/tools/measure.ts
import type { Tool, AnnotationState, Annotation, MeasureAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const measureTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "measure",
      color: state.color,
      strokeWidth: state.strokeWidth,
      startX: x,
      startY: y,
      endX: x,
      endY: y,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as MeasureAnnotation | null;
    if (!a || a.tool !== "measure") return;
    state.setActiveAnnotation({ ...a, endX: x, endY: y });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as MeasureAnnotation | null;
    if (!a || a.tool !== "measure") return;
    const dx = a.endX - a.startX;
    const dy = a.endY - a.startY;
    if (Math.sqrt(dx * dx + dy * dy) > 3) {
      state.commitAnnotation(a);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as MeasureAnnotation;
    const dx = a.endX - a.startX;
    const dy = a.endY - a.startY;
    const dist = Math.round(Math.sqrt(dx * dx + dy * dy));

    // Dashed line
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.setLineDash([6, 4]);
    ctx.beginPath();
    ctx.moveTo(a.startX, a.startY);
    ctx.lineTo(a.endX, a.endY);
    ctx.stroke();
    ctx.setLineDash([]);

    // End markers (small circles)
    ctx.fillStyle = a.color;
    for (const [px, py] of [[a.startX, a.startY], [a.endX, a.endY]]) {
      ctx.beginPath();
      ctx.arc(px, py, 3, 0, Math.PI * 2);
      ctx.fill();
    }

    // Distance label at midpoint
    const mx = (a.startX + a.endX) / 2;
    const my = (a.startY + a.endY) / 2;
    const label = `${dist}px`;
    ctx.font = "12px monospace";
    ctx.textBaseline = "bottom";
    const metrics = ctx.measureText(label);
    const pad = 3;

    ctx.fillStyle = "rgba(0,0,0,0.7)";
    ctx.fillRect(
      mx - metrics.width / 2 - pad,
      my - 16 - pad,
      metrics.width + pad * 2,
      16 + pad,
    );
    ctx.fillStyle = "white";
    ctx.textAlign = "center";
    ctx.fillText(label, mx, my - pad);
    ctx.textAlign = "start";
    ctx.textBaseline = "alphabetic";
  },
};
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds

- [ ] **Step 4: Commit**

```bash
git add src/lib/annotation/tools/blur.ts src/lib/annotation/tools/measure.ts
git commit -m "feat: add blur/pixelate and measurement annotation tools"
```

---

### Task 4: Color Picker Tool

**Files:**
- Create: `src/lib/annotation/tools/colorpicker.ts`

The color picker reads a pixel from the screenshot at the click position and sets the active stroke color. It doesn't create a permanent annotation.

- [ ] **Step 1: Create colorpicker tool**

```ts
// src/lib/annotation/tools/colorpicker.ts
import type { Tool, AnnotationState, Annotation } from "./types.ts";

export const colorPickerTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    if (!state.screenshotImage || !state.setColor) return;

    // Draw screenshot to an offscreen canvas to read pixel
    const img = state.screenshotImage;
    const canvas = document.createElement("canvas");
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.drawImage(img, 0, 0);

    // Convert CSS coordinates to image coordinates
    const dpr = window.devicePixelRatio || 1;
    const ix = Math.round(x * dpr);
    const iy = Math.round(y * dpr);

    if (ix < 0 || iy < 0 || ix >= canvas.width || iy >= canvas.height) return;

    const pixel = ctx.getImageData(ix, iy, 1, 1).data;
    const hex = `#${pixel[0].toString(16).padStart(2, "0")}${pixel[1].toString(16).padStart(2, "0")}${pixel[2].toString(16).padStart(2, "0")}`.toUpperCase();

    state.setColor(hex);
  },

  onMouseMove(_x: number, _y: number, _state: AnnotationState) {
    // No-op
  },

  onMouseUp(_x: number, _y: number, _state: AnnotationState) {
    // No-op
  },

  render(_ctx: CanvasRenderingContext2D, _annotation: Annotation) {
    // Color picker doesn't render anything
  },
};
```

- [ ] **Step 2: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build`
Expected: builds

- [ ] **Step 3: Commit**

```bash
git add src/lib/annotation/tools/colorpicker.ts
git commit -m "feat: add color picker (eyedropper) tool"
```

---

### Task 5: Update Registry, Toolbar, Shortcuts, and Canvas

**Files:**
- Modify: `src/lib/annotation/tools/registry.ts`
- Modify: `src/lib/annotation/Toolbar.svelte`
- Modify: `src/lib/annotation/Canvas.svelte`
- Modify: `src/lib/annotation/state.svelte.ts`
- Modify: `src/lib/overlay/RegionSelector.svelte`

- [ ] **Step 1: Update registry.ts**

Replace the entire file:

```ts
// src/lib/annotation/tools/registry.ts
import type { Tool, ToolType } from "./types.ts";
import { arrowTool } from "./arrow.ts";
import { rectTool } from "./rect.ts";
import { lineTool } from "./line.ts";
import { freehandTool } from "./freehand.ts";
import { textTool } from "./text.ts";
import { circleTool } from "./circle.ts";
import { highlightTool } from "./highlight.ts";
import { stepsTool } from "./steps.ts";
import { blurTool } from "./blur.ts";
import { colorPickerTool } from "./colorpicker.ts";
import { measureTool } from "./measure.ts";

const tools: Record<ToolType, Tool> = {
  arrow: arrowTool,
  rect: rectTool,
  line: lineTool,
  freehand: freehandTool,
  text: textTool,
  circle: circleTool,
  highlight: highlightTool,
  steps: stepsTool,
  blur: blurTool,
  colorpicker: colorPickerTool,
  measure: measureTool,
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

- [ ] **Step 2: Add step counter to state.svelte.ts**

Read the current `src/lib/annotation/state.svelte.ts` and add after the existing internal state vars:

```ts
let _nextStepNumber = $state(1);
```

Add an export getter:

```ts
export const nextStepNumber = {
  get value() { return _nextStepNumber; },
};
```

Add to the `clearAnnotations` function, reset the counter:

```ts
_nextStepNumber = 1;
```

Add an exported function:

```ts
export function incrementStepNumber() {
  _nextStepNumber++;
}
```

- [ ] **Step 3: Update Toolbar.svelte TOOLS array**

Read `src/lib/annotation/Toolbar.svelte` and replace the TOOLS constant:

```ts
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
```

- [ ] **Step 4: Update Canvas.svelte to pass screenshot and step state**

Read `src/lib/annotation/Canvas.svelte`. Make these changes:

1. Add `screenshotBase64` to the Props interface:

```ts
  interface Props {
    regionX: number;
    regionY: number;
    regionW: number;
    regionH: number;
    screenshotBase64: string;
  }

  let { regionX, regionY, regionW, regionH, screenshotBase64 }: Props = $props();
```

2. Add imports for the new state:

```ts
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
```

3. Add a reactive screenshot image:

```ts
  let screenshotImg: HTMLImageElement | undefined = $state(undefined);

  $effect(() => {
    if (screenshotBase64) {
      const img = new Image();
      img.src = `data:image/png;base64,${screenshotBase64}`;
      img.onload = () => { screenshotImg = img; };
    }
  });
```

4. Update the `makeState()` function to include the new fields:

```ts
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
```

- [ ] **Step 5: Update RegionSelector.svelte to pass screenshotBase64 to Canvas and update shortcuts**

Read `src/lib/overlay/RegionSelector.svelte`. Make these changes:

1. Pass screenshotBase64 to AnnotationCanvas:

```svelte
    <AnnotationCanvas
      {regionX}
      {regionY}
      regionW={regionW}
      regionH={regionH}
      {screenshotBase64}
    />
```

2. Update the TOOL_SHORTCUTS map to include new tools:

```ts
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
```

- [ ] **Step 6: Verify build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build && cargo build -p screensnap-app`
Expected: both build with no errors

- [ ] **Step 7: Run all tests**

Run: `cd /Users/biplav00/Documents/personal/screen && cargo test`
Expected: all tests pass

- [ ] **Step 8: Commit**

```bash
git add src/lib/annotation/ src/lib/overlay/RegionSelector.svelte
git commit -m "feat: register all new tools — update registry, toolbar, canvas, shortcuts"
```

---

### Task 6: E2E Verification

- [ ] **Step 1: Full build**

Run: `cd /Users/biplav00/Documents/personal/screen && npm run build && cargo build -p screensnap-app && cargo test`
Expected: all pass

- [ ] **Step 2: Manual E2E test**

Run: `cargo tauri dev`

Test each new tool:
1. **Circle (C)** — drag to draw ellipse, verify outline renders
2. **Highlight (H)** — drag to draw semi-transparent colored rectangle
3. **Steps (N)** — click to place numbered circles (1, 2, 3...), verify auto-increment
4. **Blur (B)** — drag over region, verify pixelation effect
5. **Color Picker (I)** — click on screenshot, verify stroke color changes to picked color
6. **Measure (M)** — drag to draw measurement line, verify pixel distance label
7. Verify undo/redo works with all new tools
8. Verify composited save/copy includes all new annotation types

- [ ] **Step 3: Fix any issues**

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: Phase 2b+ complete — 6 additional annotation tools"
```

---

## Phase 2b+ Summary

After completing all 6 tasks:

- 11 total annotation tools: Arrow, Rectangle, Circle, Line, Freehand, Text, Highlight, Blur, Step Numbers, Color Picker, Measurement
- All tools accessible via toolbar buttons and keyboard shortcuts
- Blur uses canvas pixel manipulation for real pixelation
- Color picker reads from screenshot image data
- Step numbers auto-increment (reset on new region)
- Measurement shows pixel distance with label
- All tools support undo/redo and composited export
