import type { Tool, AnnotationState, Annotation, StepAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

const RADIUS = 14;

export const stepsTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    const num = state.nextStepNumber ?? 1;
    const annotation: StepAnnotation = {
      id: generateId(),
      tool: "steps",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      number: num,
    };
    state.commitAnnotation(annotation);
    if (state.incrementStepNumber) {
      state.incrementStepNumber();
    }
  },

  onMouseMove(_x: number, _y: number, _state: AnnotationState) {},

  onMouseUp(_x: number, _y: number, _state: AnnotationState) {},

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as StepAnnotation;
    ctx.fillStyle = a.color;
    ctx.beginPath();
    ctx.arc(a.x, a.y, RADIUS, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = "white";
    ctx.font = `bold ${RADIUS}px system-ui, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(String(a.number), a.x, a.y);

    ctx.textAlign = "start";
    ctx.textBaseline = "alphabetic";
  },
};
