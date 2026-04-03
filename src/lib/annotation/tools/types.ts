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
