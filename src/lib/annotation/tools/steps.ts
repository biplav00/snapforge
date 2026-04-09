import type { Annotation, AnnotationState, StepAnnotation, Tool } from "./types.ts";
import { generateId } from "./types.ts";

const PADDING_X = 8;
const GAP = 6;

/** Radius scales with stroke width: S=10, M=14, L=20 */
function getRadius(strokeWidth: number): number {
  return 8 + strokeWidth * 3;
}

export const stepsTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    const num = state.nextStepNumber ?? 1;
    state.setActiveAnnotation({
      id: generateId(),
      tool: "steps",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      number: num,
      label: "",
    });
  },

  onMouseMove(_x: number, _y: number, _state: AnnotationState) {},

  onMouseUp(_x: number, _y: number, _state: AnnotationState) {},

  render(ctx: CanvasRenderingContext2D, annotation: Annotation) {
    const a = annotation as StepAnnotation;
    const r = getRadius(a.strokeWidth);

    ctx.save();

    // Draw numbered circle
    ctx.fillStyle = a.color;
    ctx.beginPath();
    ctx.arc(a.x, a.y, r, 0, Math.PI * 2);
    ctx.fill();

    // Number text
    ctx.fillStyle = "white";
    ctx.font = `bold ${r}px system-ui, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(String(a.number), a.x, a.y);

    // Label box (to the right of the circle, only if label is non-empty)
    if (a.label) {
      ctx.font = `bold ${Math.max(12, r - 1)}px system-ui, sans-serif`;
      ctx.textAlign = "left";
      ctx.textBaseline = "middle";
      const metrics = ctx.measureText(a.label);
      const boxX = a.x + r + GAP;
      const boxY = a.y - r;
      const boxW = metrics.width + PADDING_X * 2;
      const boxH = r * 2;

      // Rounded rect background
      ctx.fillStyle = a.color;
      ctx.beginPath();
      ctx.roundRect(boxX, boxY, boxW, boxH, 4);
      ctx.fill();

      // Label text
      ctx.fillStyle = "white";
      ctx.fillText(a.label, boxX + PADDING_X, a.y);
    }

    ctx.restore();
  },
};
