import { renderAnnotation } from "./tools/registry.ts";
import type { Annotation } from "./tools/types.ts";

/**
 * Composite the screenshot region + annotations into a single PNG base64 string.
 */
export async function compositeImage(
  screenshotBase64: string,
  regionX: number,
  regionY: number,
  regionW: number,
  regionH: number,
  annotationList: Annotation[],
): Promise<string> {
  const dpr = window.devicePixelRatio || 1;
  const physW = Math.round(regionW * dpr);
  const physH = Math.round(regionH * dpr);
  const physX = Math.round(regionX * dpr);
  const physY = Math.round(regionY * dpr);

  // Load the full screenshot image
  const img = new Image();
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error("Failed to load screenshot"));
    img.src = `data:image/png;base64,${screenshotBase64}`;
  });

  // Create offscreen canvas at physical pixel resolution
  const canvas = document.createElement("canvas");
  canvas.width = physW;
  canvas.height = physH;
  const ctx = canvas.getContext("2d")!;

  // Draw cropped screenshot region
  ctx.drawImage(img, physX, physY, physW, physH, 0, 0, physW, physH);

  // Scale context for annotations (they use CSS pixel coordinates)
  ctx.scale(dpr, dpr);

  // Render all committed annotations
  for (const a of annotationList) {
    renderAnnotation(ctx, a);
  }

  // Export as base64 PNG (strip the data:image/png;base64, prefix)
  const dataUrl = canvas.toDataURL("image/png");
  return dataUrl.replace(/^data:image\/png;base64,/, "");
}
