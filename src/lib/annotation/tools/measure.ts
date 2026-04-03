import type { Tool, AnnotationState, Annotation, MeasureAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const measureTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "measure",
      color: state.color,
      strokeWidth: state.strokeWidth,
      startX: x,
      startY: y,
      endX: x,
      endY: y,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as MeasureAnnotation | null;
    if (!a || a.tool !== "measure") return;
    state.setActiveAnnotation({ ...a, endX: x, endY: y });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as MeasureAnnotation | null;
    if (!a || a.tool !== "measure") return;
    const dx = a.endX - a.startX;
    const dy = a.endY - a.startY;
    if (Math.sqrt(dx * dx + dy * dy) > 3) {
      state.commitAnnotation(a);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as MeasureAnnotation;
    const dx = a.endX - a.startX;
    const dy = a.endY - a.startY;
    const dist = Math.round(Math.sqrt(dx * dx + dy * dy));

    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.setLineDash([6, 4]);
    ctx.beginPath();
    ctx.moveTo(a.startX, a.startY);
    ctx.lineTo(a.endX, a.endY);
    ctx.stroke();
    ctx.setLineDash([]);

    ctx.fillStyle = a.color;
    for (const [px, py] of [[a.startX, a.startY], [a.endX, a.endY]]) {
      ctx.beginPath();
      ctx.arc(px, py, 3, 0, Math.PI * 2);
      ctx.fill();
    }

    const mx = (a.startX + a.endX) / 2;
    const my = (a.startY + a.endY) / 2;
    const label = `${dist}px`;
    ctx.font = "12px monospace";
    ctx.textBaseline = "bottom";
    const metrics = ctx.measureText(label);
    const pad = 3;

    ctx.fillStyle = "rgba(0,0,0,0.7)";
    ctx.fillRect(
      mx - metrics.width / 2 - pad,
      my - 16 - pad,
      metrics.width + pad * 2,
      16 + pad,
    );
    ctx.fillStyle = "white";
    ctx.textAlign = "center";
    ctx.fillText(label, mx, my - pad);
    ctx.textAlign = "start";
    ctx.textBaseline = "alphabetic";
  },
};
