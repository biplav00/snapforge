import type { Annotation, AnnotationState, FreehandAnnotation, Tool } from "./types.ts";
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
