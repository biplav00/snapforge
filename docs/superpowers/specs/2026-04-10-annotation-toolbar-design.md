# Annotation Layer + Toolbar — Design Spec

## Summary

Reimplement the 12-tool annotation system and floating toolbar in C++ Qt using QPainter, replacing the Svelte Canvas-based implementation. Integrates into the existing Qt overlay prototype.

## Architecture

```
qt/src/
├── OverlayWindow.h/cpp       (modify: integrate annotation canvas + toolbar)
├── Annotation.h               (data types for all 12 annotation tools)
├── AnnotationState.h/cpp      (undo/redo, active tool, color, stroke width)
├── AnnotationRenderer.h/cpp   (QPainter render functions per tool type)
├── AnnotationCanvas.h/cpp     (transparent widget over region, handles mouse/paint)
├── AnnotationToolbar.h/cpp    (floating draggable toolbar widget)
└── tools/
    ├── ToolHandler.h           (base interface: onMouseDown/Move/Up)
    ├── DragTool.h/cpp          (shared logic: arrow, rect, circle, line, dotted, freehand, highlight, blur, measure)
    ├── TextTool.h/cpp          (click placement + QLineEdit overlay)
    ├── StepsTool.h/cpp         (auto-numbered circles + optional label input)
    └── ColorPickerTool.h/cpp   (pixel sampling from pre-captured screenshot)
```

## Annotation Data Types (`Annotation.h`)

All annotations share a base:

```cpp
struct Annotation {
    QString id;          // unique UUID
    ToolType tool;       // enum
    QColor color;
    int strokeWidth;
    // Per-tool data via std::variant or tagged union
};
```

### Per-tool data

| Tool | Fields | Notes |
|---|---|---|
| Arrow | startX, startY, endX, endY | Arrowhead triangle at end |
| Rect | x, y, width, height | Auto-normalized if drawn backwards |
| Circle | cx, cy, rx, ry | Independent radii (ellipse) |
| Line | startX, startY, endX, endY | Round caps |
| DottedLine | startX, startY, endX, endY | Dash pattern = strokeWidth*3 |
| Freehand | QVector<QPointF> points | Sampled every mouse move |
| Text | x, y, text (QString), fontSize=16 | Committed via QLineEdit |
| Highlight | x, y, width, height | Filled rect with opacity=0.3 |
| Blur | x, y, width, height, intensity=10 | Pixelated block averaging |
| Steps | x, y, number (int), label (QString) | Auto-incrementing counter |
| ColorPicker | x, y | Utility — samples pixel, sets color, not stored |
| Measure | startX, startY, endX, endY | Dashed line + dots + distance label |

ToolType enum:
```cpp
enum class ToolType {
    Arrow, Rect, Circle, Line, DottedLine, Freehand,
    Text, Highlight, Blur, Steps, ColorPicker, Measure
};
```

## Annotation State (`AnnotationState.h/cpp`)

Manages annotation list, active tool, undo/redo, color, and stroke width.

**State:**
- `QVector<Annotation> annotations` — committed annotations
- `std::optional<Annotation> activeAnnotation` — currently being drawn
- `ToolType activeTool` — default Arrow
- `QColor strokeColor` — default #FF0000
- `int strokeWidth` — 1, 2, or 4 (default 2)
- `int nextStepNumber` — auto-incrementing (default 1)
- `QVector<QVector<Annotation>> undoStack` — max 50 entries
- `QVector<QVector<Annotation>> redoStack`

**Methods:**
- `commitAnnotation(Annotation)` — push undo, append, clear redo
- `undo()` / `redo()`
- `clearAnnotations()` — reset all, step counter back to 1
- `offsetAnnotations(dx, dy)` — shift all coords (for region drag)
- `setTool(ToolType)` / `setColor(QColor)` / `setStrokeWidth(int)`

## Annotation Renderer (`AnnotationRenderer.h/cpp`)

Static function: `renderAnnotation(QPainter &p, const Annotation &a)`

Dispatches by tool type. Each tool's render:

**Arrow:** `drawLine(start, end)` + filled triangle arrowhead. Head length = max(10, strokeWidth*4). Angle from atan2.

**Rect:** `drawRect(x, y, w, h)` with miter joins.

**Circle:** `drawEllipse(QRectF(cx-rx, cy-ry, 2*rx, 2*ry))`.

**Line:** `drawLine(start, end)` with round caps.

**DottedLine:** `setPen` with `setDashPattern({sw*3, sw*3})`, then `drawLine`.

**Freehand:** `QPainterPath` with `moveTo(points[0])`, `lineTo(points[i])` for rest. Round cap/join.

**Text:** `drawText(x, y, text)` with `QFont("system-ui", 16)`. Fill color = annotation color.

**Highlight:** `setOpacity(0.3)`, `fillRect(x, y, w, h)`, restore opacity.

**Blur:** Iterate the region in blocks of size `max(2, intensity)`. For each block, sample center pixel color from the pre-captured screenshot QImage. Fill block with that color using `fillRect`. Fallback: dashed rect if image not available.

**Steps:** Filled circle at (x, y), radius = 8 + strokeWidth*3. White bold number centered inside. If label: rounded rect to the right with label text.

**Measure:** Dashed line ({6, 4}), filled 3px circles at endpoints, distance label at midpoint ("XXXpx") in dark semi-transparent box, white monospace text.

**ColorPicker:** No render (utility tool).

## Annotation Canvas (`AnnotationCanvas.h/cpp`)

A transparent QWidget positioned exactly over the selected region. Handles:

- **paintEvent:** Clear, draw all committed annotations via renderer, draw active annotation
- **Mouse events:** Delegate to current ToolHandler
- **Compositing for save:** Create QPainter on a copy of the cropped screenshot QImage, render all annotations onto it, return the composited QImage

The canvas is created when the user selects a region (transitions to annotate mode). It is destroyed/hidden when the overlay closes.

## Tool Handlers (`tools/`)

### ToolHandler interface

```cpp
class ToolHandler {
public:
    virtual ~ToolHandler() = default;
    virtual Annotation onMouseDown(QPointF pos, AnnotationState &state) = 0;
    virtual void onMouseMove(QPointF pos, Annotation &active) = 0;
    virtual bool onMouseUp(Annotation &active) = 0; // returns true if valid (commit)
};
```

### DragTool

Shared implementation for tools that drag between two points or accumulate points:
- Arrow, Rect, Circle, Line, DottedLine, Highlight, Blur, Measure: store start→end or origin→size
- Freehand: appends points on every move
- `onMouseUp` validates minimum size (distance > 3px or |w|/|h| > 3)
- Normalizes negative width/height for Rect, Highlight, Blur

### TextTool

- `onMouseDown`: Creates annotation at click point, shows QLineEdit overlay positioned at (x, y) relative to canvas
- QLineEdit styled: dark semi-transparent background, dashed white border, white text
- Enter commits text (if non-empty), Escape cancels
- No mouse move/up behavior

### StepsTool

- `onMouseDown`: Creates annotation with `number = state.nextStepNumber`, increments counter
- Shows QLineEdit for optional label, positioned offset right of circle (x+28, y-12)
- Enter commits (label can be empty — circle always drawn), Escape cancels

### ColorPickerTool

- `onMouseDown`: Samples pixel at (x*dpr, y*dpr) from the pre-captured screenshot QImage
- Sets `state.setColor(sampledColor)`
- Does not create an annotation — returns without committing

## Annotation Toolbar (`AnnotationToolbar.h/cpp`)

Floating QWidget, auto-positioned below the selected region (or above if no space, or inside near bottom).

**Layout (horizontal):**
1. Tool buttons (12): icons or text labels, toggle-style (active tool highlighted)
2. Separator
3. Color swatches: 8 preset colors + custom color button (opens QColorDialog)
4. Separator
5. Stroke width: 3 dot buttons (small/medium/large)
6. Separator
7. Undo / Redo buttons
8. Separator
9. Save / Copy / Cancel buttons

**Draggable:** Mouse press on non-button area starts drag, mouse move repositions widget.

**Keyboard shortcuts** (handled by OverlayWindow, not toolbar):
- Tools: A, R, C, L, D, F, T, H, B, N, I, M
- Sizes: 1, 2, 3
- Undo: Cmd+Z, Redo: Cmd+Shift+Z
- Save: Cmd+S or Enter, Copy: Cmd+C, Cancel: Escape

**Preset colors:** #FF0000, #00FF00, #0000FF, #FFFF00, #FF00FF, #00FFFF, #FFFFFF, #000000

## OverlayWindow Integration

The OverlayWindow gains a new mode state:

```
select → annotate (after region drawn, for screenshot purpose)
select → record-select (after region drawn, for record purpose)
```

In annotate mode:
- AnnotationCanvas is created/shown over the selected region
- AnnotationToolbar is created/shown below the region
- Mouse events inside the region go to AnnotationCanvas
- Mouse events on region edges/handles still allow resize/drag (existing behavior)
- Mouse events outside region start a new selection (clear annotations, back to select mode)

## Compositing for Save/Copy

When saving or copying:
1. Use the pre-captured screenshot QImage (full screen, RGBA)
2. Get DPI scale factor
3. Crop to selected region: `QImage cropped = screenshot.copy(px, py, pw, ph)`
4. Create `QPainter` on the cropped image
5. Scale painter by DPR: `painter.scale(dpr, dpr)`
6. Render all committed annotations onto it
7. Extract raw RGBA bytes from QImage
8. Pass to `snapforge_save_image()` or `snapforge_copy_to_clipboard()` via FFI

## What is NOT included

- Recording support (separate sub-project)
- History window (separate sub-project)
- Preferences window (separate sub-project)
- Animated marching ants (cosmetic, later)
- Functional resize handles for region (prototype has visual-only handles, acceptable)
