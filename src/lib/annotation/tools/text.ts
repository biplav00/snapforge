import type { Tool, AnnotationState, Annotation, TextAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

export const textTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "text",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      text: "",
      fontSize: 16,
    });
  },

  onMouseMove(_x: number, _y: number, _state: AnnotationState) {
    // No-op — text doesn't drag
  },

  onMouseUp(_x: number, _y: number, _state: AnnotationState) {
    // No-op — text input is handled by Canvas component
  },

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as TextAnnotation;
    if (!a.text) return;
    ctx.fillStyle = a.color;
    ctx.font = `${a.fontSize}px system-ui, sans-serif`;
    ctx.textBaseline = "top";
    ctx.fillText(a.text, a.x, a.y);
  },
};
