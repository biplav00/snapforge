import type { Annotation, AnnotationState, Tool } from "./types.ts";

export const colorPickerTool: Tool = {
  onMouseDown(x: number, y: number, state: AnnotationState) {
    if (!state.screenshotImage || !state.setColor) return;

    const img = state.screenshotImage;
    const canvas = document.createElement("canvas");
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.drawImage(img, 0, 0);

    const dpr = window.devicePixelRatio || 1;
    const ix = Math.round(x * dpr);
    const iy = Math.round(y * dpr);

    if (ix < 0 || iy < 0 || ix >= canvas.width || iy >= canvas.height) return;

    const pixel = ctx.getImageData(ix, iy, 1, 1).data;
    const hex =
      `#${pixel[0].toString(16).padStart(2, "0")}${pixel[1].toString(16).padStart(2, "0")}${pixel[2].toString(16).padStart(2, "0")}`.toUpperCase();

    state.setColor(hex);
  },

  onMouseMove(_x: number, _y: number, _state: AnnotationState) {},

  onMouseUp(_x: number, _y: number, _state: AnnotationState) {},

  render(_ctx: CanvasRenderingContext2D, _annotation: Annotation) {},
};
