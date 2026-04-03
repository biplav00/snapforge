// src/lib/annotation/state.svelte.ts
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
