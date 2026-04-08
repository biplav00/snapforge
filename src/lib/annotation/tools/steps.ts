import type { Annotation, AnnotationState, StepAnnotation, Tool } from "./types.ts";
import { generateId } from "./types.ts";

/** Radius scales with stroke width: S=10, M=14, L=20 */
function getRadius(strokeWidth: number): number {
  return 8 + strokeWidth * 3;
}

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
    const r = getRadius(a.strokeWidth);

    ctx.save();
    ctx.fillStyle = a.color;
    ctx.beginPath();
    ctx.arc(a.x, a.y, r, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = "white";
    ctx.font = `bold ${r}px system-ui, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(String(a.number), a.x, a.y);
    ctx.restore();
  },
};
