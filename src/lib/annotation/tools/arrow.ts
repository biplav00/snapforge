import type { Tool, AnnotationState, Annotation, ArrowAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const arrowTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "arrow",
      color: state.color,
      strokeWidth: state.strokeWidth,
      startX: x,
      startY: y,
      endX: x,
      endY: y,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as ArrowAnnotation | null;
    if (!a || a.tool !== "arrow") return;
    state.setActiveAnnotation({ ...a, endX: x, endY: y });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as ArrowAnnotation | null;
    if (!a || a.tool !== "arrow") return;
    const dx = a.endX - a.startX;
    const dy = a.endY - a.startY;
    if (Math.sqrt(dx * dx + dy * dy) > 3) {
      state.commitAnnotation(a);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as ArrowAnnotation;
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.lineCap = "round";

    ctx.beginPath();
    ctx.moveTo(a.startX, a.startY);
    ctx.lineTo(a.endX, a.endY);
    ctx.stroke();

    const angle = Math.atan2(a.endY - a.startY, a.endX - a.startX);
    const headLen = Math.max(10, a.strokeWidth * 4);
    ctx.fillStyle = a.color;
    ctx.beginPath();
    ctx.moveTo(a.endX, a.endY);
    ctx.lineTo(
      a.endX - headLen * Math.cos(angle - Math.PI / 6),
      a.endY - headLen * Math.sin(angle - Math.PI / 6),
    );
    ctx.lineTo(
      a.endX - headLen * Math.cos(angle + Math.PI / 6),
      a.endY - headLen * Math.sin(angle + Math.PI / 6),
    );
    ctx.closePath();
    ctx.fill();
  },
};
