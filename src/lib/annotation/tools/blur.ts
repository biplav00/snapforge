import type { Tool, AnnotationState, Annotation, BlurAnnotation } from "./types.ts";
import { generateId } from "./types.ts";

const DEFAULT_INTENSITY = 10;

export const blurTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    state.setActiveAnnotation({
      id: generateId(),
      tool: "blur",
      color: state.color,
      strokeWidth: state.strokeWidth,
      x,
      y,
      width: 0,
      height: 0,
      intensity: DEFAULT_INTENSITY,
    });
  },

  onMouseMove(x: number, y: number, state: AnnotationState) {
    const a = state.activeAnnotation as BlurAnnotation | null;
    if (!a || a.tool !== "blur") return;
    state.setActiveAnnotation({
      ...a,
      width: x - a.x,
      height: y - a.y,
    });
  },

  onMouseUp(_x: number, _y: number, state: AnnotationState) {
    const a = state.activeAnnotation as BlurAnnotation | null;
    if (!a || a.tool !== "blur") return;
    if (Math.abs(a.width) > 3 && Math.abs(a.height) > 3) {
      const normalized: BlurAnnotation = {
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
    const a = annotation as BlurAnnotation;
    const x = Math.round(a.x);
    const y = Math.round(a.y);
    const w = Math.round(Math.abs(a.width));
    const h = Math.round(Math.abs(a.height));
    if (w <= 0 || h <= 0) return;

    const blockSize = Math.max(2, Math.round(a.intensity));

    try {
      const imageData = ctx.getImageData(x, y, w, h);
      const data = imageData.data;

      for (let by = 0; by < h; by += blockSize) {
        for (let bx = 0; bx < w; bx += blockSize) {
          const sx = Math.min(bx + Math.floor(blockSize / 2), w - 1);
          const sy = Math.min(by + Math.floor(blockSize / 2), h - 1);
          const si = (sy * w + sx) * 4;
          const r = data[si];
          const g = data[si + 1];
          const b = data[si + 2];
          const alpha = data[si + 3];

          for (let dy = by; dy < Math.min(by + blockSize, h); dy++) {
            for (let dx = bx; dx < Math.min(bx + blockSize, w); dx++) {
              const di = (dy * w + dx) * 4;
              data[di] = r;
              data[di + 1] = g;
              data[di + 2] = b;
              data[di + 3] = alpha;
            }
          }
        }
      }

      ctx.putImageData(imageData, x, y);
    } catch {
      ctx.strokeStyle = "rgba(128,128,128,0.5)";
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.strokeRect(x, y, w, h);
      ctx.setLineDash([]);
    }
  },
};
