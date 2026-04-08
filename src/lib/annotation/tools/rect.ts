import type { Annotation, AnnotationState, RectAnnotation, Tool } from "./types.ts";
import { generateId } from "./types.ts";

export const rectTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "rect",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      width: 0,
      height: 0,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as RectAnnotation | null;
    if (!a || a.tool !== "rect") return;
    state.setActiveAnnotation({
      ...a,
      width: x - a.x,
      height: y - a.y,
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as RectAnnotation | null;
    if (!a || a.tool !== "rect") return;
    if (Math.abs(a.width) > 3 && Math.abs(a.height) > 3) {
      const normalized: RectAnnotation = {
        ...a,
        x: a.width < 0 ? a.x + a.width : a.x,
        y: a.height < 0 ? a.y + a.height : a.y,
        width: Math.abs(a.width),
        height: Math.abs(a.height),
      };
      state.commitAnnotation(normalized);
    } else {
      state.setActiveAnnotation(null);
    }
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as RectAnnotation;
    ctx.strokeStyle = a.color;
    ctx.lineWidth = a.strokeWidth;
    ctx.lineJoin = "miter";
    ctx.strokeRect(a.x, a.y, a.width, a.height);
  },
};
