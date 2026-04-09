// src/lib/annotation/state.svelte.ts
import type { Annotation, ToolType } from "./tools/types.ts";

const MAX_UNDO = 50;

// Internal reactive state — not exported directly because Svelte 5
// forbids exporting $state variables that are reassigned.
let _annotations = $state<Annotation[]>([]);
let _activeTool = $state<ToolType>("arrow");
let _strokeColor = $state("#FF0000");
let _strokeWidth = $state(2);
let _activeAnnotation = $state<Annotation | null>(null);
let _undoStack = $state<Annotation[][]>([]);
let _redoStack = $state<Annotation[][]>([]);
let _nextStepNumber = $state(1);

// Export as getter properties for reactive template use
export const annotations = {
  get value() {
    return _annotations;
  },
};

export const activeTool = {
  get value() {
    return _activeTool;
  },
};

export const strokeColor = {
  get value() {
    return _strokeColor;
  },
};

export const strokeWidth = {
  get value() {
    return _strokeWidth;
  },
};

export const activeAnnotation = {
  get value() {
    return _activeAnnotation;
  },
};

export const nextStepNumber = {
  get value() {
    return _nextStepNumber;
  },
};

export function setActiveAnnotation(a: Annotation | null) {
  _activeAnnotation = a;
}

export function commitAnnotation(a: Annotation) {
  _undoStack.push([..._annotations]);
  if (_undoStack.length > MAX_UNDO) {
    _undoStack = _undoStack.slice(-MAX_UNDO);
  }
  _redoStack = [];
  _annotations.push(a);
  _activeAnnotation = null;
}

export function undo() {
  if (_undoStack.length === 0) return;
  _redoStack.push([..._annotations]);
  _annotations = _undoStack.pop()!;
}

export function redo() {
  if (_redoStack.length === 0) return;
  _undoStack.push([..._annotations]);
  _annotations = _redoStack.pop()!;
}

export function clearAnnotations() {
  if (_annotations.length === 0) {
    _nextStepNumber = 1;
    return;
  }
  _undoStack.push([..._annotations]);
  _redoStack = [];
  _annotations = [];
  _nextStepNumber = 1;
}

export function setTool(tool: ToolType) {
  _activeTool = tool;
}

export function setColor(color: string) {
  _strokeColor = color;
}

export function setStrokeWidth(width: number) {
  _strokeWidth = width;
}

export function incrementStepNumber() {
  _nextStepNumber++;
}

export function canUndo(): boolean {
  return _undoStack.length > 0;
}

export function canRedo(): boolean {
  return _redoStack.length > 0;
}

/** Offset all annotation coordinates by (dx, dy) so they stay fixed on screen when the region moves. */
export function offsetAnnotations(dx: number, dy: number) {
  if (dx === 0 && dy === 0) return;
  _annotations = _annotations.map((a) => offsetAnnotation(a, dx, dy));
}

function offsetAnnotation(a: Annotation, dx: number, dy: number): Annotation {
  switch (a.tool) {
    case "arrow":
    case "line":
    case "dottedline":
    case "measure":
      return {
        ...a,
        startX: a.startX + dx,
        startY: a.startY + dy,
        endX: a.endX + dx,
        endY: a.endY + dy,
      };
    case "rect":
    case "highlight":
    case "blur":
      return { ...a, x: a.x + dx, y: a.y + dy };
    case "circle":
      return { ...a, cx: a.cx + dx, cy: a.cy + dy };
    case "freehand":
      return { ...a, points: a.points.map((p) => ({ x: p.x + dx, y: p.y + dy })) };
    case "text":
    case "steps":
    case "colorpicker":
      return { ...a, x: a.x + dx, y: a.y + dy };
    default:
      return a;
  }
}
