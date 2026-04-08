import type { Annotation, AnnotationState, HighlightAnnotation, Tool } from "./types.ts";
import { generateId } from "./types.ts";

export const highlightTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "highlight",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      width: 0,
      height: 0,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as HighlightAnnotation | null;
    if (!a || a.tool !== "highlight") return;
    state.setActiveAnnotation({
      ...a,
      width: x - a.x,
      height: y - a.y,
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as HighlightAnnotation | null;
    if (!a || a.tool !== "highlight") return;
    if (Math.abs(a.width) > 3 && Math.abs(a.height) > 3) {
      const normalized: HighlightAnnotation = {
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
    const a = annotation as HighlightAnnotation;
    ctx.fillStyle = a.color;
    ctx.globalAlpha = 0.3;
    ctx.fillRect(a.x, a.y, a.width, a.height);
    ctx.globalAlpha = 1.0;
  },
};
