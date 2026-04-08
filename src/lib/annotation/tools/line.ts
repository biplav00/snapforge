import type { Annotation, AnnotationState, LineAnnotation, Tool } from "./types.ts";
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
