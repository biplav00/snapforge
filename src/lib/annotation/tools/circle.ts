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
