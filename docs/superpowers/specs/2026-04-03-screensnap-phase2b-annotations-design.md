# ScreenSnap Phase 2b: Annotation Toolkit (Core Subset) — Design Spec

## Goal

Add an annotation layer to the screenshot overlay so users can draw on the captured region before saving. Core tools: Arrow, Rectangle, Line, Freehand, Text. Includes floating toolbar, color/stroke configuration, and undo/redo.

## Architecture

All annotations are stored as a typed array of objects (vector-based, not rasterized until export). An HTML5 Canvas layer sits on top of the frozen screenshot inside the overlay. Each tool implements a common interface for mouse interaction and rendering. A floating toolbar appears near the selected region.

When saving, the frontend composites the screenshot region + annotations into a single image on an offscreen canvas, encodes it as base64 PNG, and sends it to Rust for saving to disk.

## Components

### Annotation State (`src/lib/annotation/state.ts`)

Svelte store (using Svelte 5 runes) holding:
- `annotations: Annotation[]` — ordered array of all drawn annotations
- `undoStack: Annotation[][]` / `redoStack: Annotation[][]` — snapshot-based undo/redo
- `activeTool: ToolType` — which tool is selected (`"arrow" | "rect" | "line" | "freehand" | "text"`)
- `strokeColor: string` — current color (default: `"#FF0000"`)
- `strokeWidth: number` — current stroke width (default: `2`)
- `activeAnnotation: Annotation | null` — annotation currently being drawn

Functions: `addAnnotation()`, `undo()`, `redo()`, `clear()`, `setTool()`, `setColor()`, `setStrokeWidth()`

### Annotation Types (`src/lib/annotation/tools/types.ts`)

```ts
type ToolType = "arrow" | "rect" | "line" | "freehand" | "text";

interface BaseAnnotation {
  id: string;
  tool: ToolType;
  color: string;
  strokeWidth: number;
}

interface ArrowAnnotation extends BaseAnnotation {
  tool: "arrow";
  startX: number; startY: number;
  endX: number; endY: number;
}

interface RectAnnotation extends BaseAnnotation {
  tool: "rect";
  x: number; y: number;
  width: number; height: number;
}

interface LineAnnotation extends BaseAnnotation {
  tool: "line";
  startX: number; startY: number;
  endX: number; endY: number;
}

interface FreehandAnnotation extends BaseAnnotation {
  tool: "freehand";
  points: { x: number; y: number }[];
}

interface TextAnnotation extends BaseAnnotation {
  tool: "text";
  x: number; y: number;
  text: string;
  fontSize: number;
}

type Annotation = ArrowAnnotation | RectAnnotation | LineAnnotation | FreehandAnnotation | TextAnnotation;
```

### Tool Interface (`src/lib/annotation/tools/*.ts`)

Each tool file exports an object implementing:

```ts
interface Tool {
  onMouseDown(x: number, y: number, state: AnnotationState): void;
  onMouseMove(x: number, y: number, state: AnnotationState): void;
  onMouseUp(x: number, y: number, state: AnnotationState): void;
  render(ctx: CanvasRenderingContext2D, annotation: Annotation): void;
}
```

Tool files:
- `arrow.ts` — line with triangular arrowhead at end point
- `rect.ts` — outline rectangle (not filled)
- `line.ts` — straight line between two points
- `freehand.ts` — collects points on mousemove, draws polyline
- `text.ts` — on mousedown, sets a placement position; the Canvas component shows an `<input>` overlay for text entry; on confirm, stores the text annotation

### Canvas Renderer (`src/lib/annotation/Canvas.svelte`)

- `<canvas>` element sized to the selected region
- Positioned over the selected region in the overlay
- On every state change, clears and re-renders all annotations plus the active (in-progress) annotation
- Handles mousedown/mousemove/mouseup and delegates to the active tool
- For text tool: renders an absolutely-positioned `<input>` element at the click position

Props: `regionX`, `regionY`, `regionW`, `regionH`, `screenshotBase64`, annotation state

### Floating Toolbar (`src/lib/annotation/Toolbar.svelte`)

Positioned below the selected region (or above if near screen bottom).

Layout:
```
[Arrow] [Rect] [Line] [Freehand] [Text] | [Color] [Size] | [Undo] [Redo] | [Save] [Copy] [Cancel]
```

- Tool buttons highlight when active
- Color: row of 8 preset color swatches (red, blue, green, yellow, orange, purple, white, black) — click to select
- Size: 3 preset buttons (thin=1px, medium=2px, thick=4px)
- Undo/Redo: disabled when stack is empty
- Save: composites and saves
- Copy: composites and copies to clipboard (reuses existing clipboard support)
- Cancel: closes overlay

### Compositing & Save

When user clicks Save or Copy:
1. Create an offscreen canvas at the region's physical pixel dimensions (accounting for DPR)
2. Draw the cropped screenshot region onto it
3. Draw all annotations onto it (using the same render functions)
4. Export as PNG blob / base64
5. For Save: send base64 to new Tauri command `save_composited_image`
6. For Copy: send base64 to new Tauri command `copy_composited_image`

New Tauri commands in `src-tauri/src/commands.rs`:
- `save_composited_image(image_base64: String) -> Result<String, String>` — decodes base64 PNG, saves to configured path
- `copy_composited_image(image_base64: String) -> Result<(), String>` — decodes base64 PNG, copies to clipboard

### Changes to Existing Components

**`RegionSelector.svelte`:**
- After region is drawn (hasRegion=true), transitions to annotation mode instead of showing save/cancel immediately
- The save/cancel buttons move to the Toolbar
- Resize handles remain active during annotation mode

**`Overlay.svelte`:**
- Passes `screenshotBase64` to the annotation Canvas for compositing
- The annotation Canvas and Toolbar render when in annotation mode

## Keyboard Shortcuts

- `Ctrl+Z` / `Cmd+Z` — Undo
- `Ctrl+Shift+Z` / `Cmd+Shift+Z` — Redo
- `Escape` — Cancel (close overlay)
- `Enter` — Save (when not in text input mode)

## File Structure

```
src/lib/annotation/
├── state.ts              # annotation store (runes-based)
├── Canvas.svelte         # canvas renderer + mouse delegation
├── Toolbar.svelte        # floating toolbar
├── compositing.ts        # offscreen canvas compositing for export
└── tools/
    ├── types.ts          # Annotation types, Tool interface
    ├── arrow.ts          # arrow tool
    ├── rect.ts           # rectangle tool
    ├── line.ts           # line tool
    ├── freehand.ts       # freehand tool
    └── text.ts           # text tool
```

## Non-Goals (deferred to later phase)

- Circle/Ellipse, Blur/Pixelate, Highlight, Step Numbers, Color Picker (eyedropper), Measurement tools
- Opacity configuration per tool
- SVG export of annotation vectors
- Annotation defaults in preferences
