// src/lib/annotation/state.svelte.ts
import type { Annotation, ToolType } from "./tools/types.ts";

// Internal reactive state — not exported directly because Svelte 5
// forbids exporting $state variables that are reassigned.
let _annotations = $state<Annotation[]>([]);
let _activeTool = $state<ToolType>("arrow");
let _strokeColor = $state("#FF0000");
let _strokeWidth = $state(2);
let _activeAnnotation = $state<Annotation | null>(null);
let _undoStack = $state<Annotation[][]>([]);
let _redoStack = $state<Annotation[][]>([]);

// Export getters so consumers can read reactive values
export function getAnnotations(): Annotation[] {
  return _annotations;
}

// Re-export as a getter property on an object for reactive template use
export const annotations = {
  get value() { return _annotations; },
};

export const activeTool = {
  get value() { return _activeTool; },
};

export const strokeColor = {
  get value() { return _strokeColor; },
};

export const strokeWidth = {
  get value() { return _strokeWidth; },
};

export const activeAnnotation = {
  get value() { return _activeAnnotation; },
};

export function setActiveAnnotation(a: Annotation | null) {
  _activeAnnotation = a;
}

export function commitAnnotation(a: Annotation) {
  _undoStack.push([..._annotations]);
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
  if (_annotations.length === 0) return;
  _undoStack.push([..._annotations]);
  _redoStack = [];
  _annotations = [];
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

export function canUndo(): boolean {
  return _undoStack.length > 0;
}

export function canRedo(): boolean {
  return _redoStack.length > 0;
}
