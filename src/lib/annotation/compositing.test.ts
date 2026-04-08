import { describe, expect, it } from "vitest";
import { compositeImage } from "./compositing.ts";
import type { ArrowAnnotation } from "./tools/types.ts";

// Create a minimal 2x2 white PNG as base64
function canvasSupported(): boolean {
  const c = document.createElement("canvas");
  return c.getContext("2d") !== null;
}

describe("compositeImage", () => {
  it.skipIf(!canvasSupported())("returns base64 PNG string with no annotations", async () => {
    const canvas = document.createElement("canvas");
    canvas.width = 100;
    canvas.height = 100;
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = "#FFFFFF";
    ctx.fillRect(0, 0, 100, 100);
    const base64 = canvas.toDataURL("image/png").replace(/^data:image\/png;base64,/, "");

    const result = await compositeImage(base64, 0, 0, 50, 50, []);
    expect(result).toBeTruthy();
    expect(typeof result).toBe("string");
    expect(() => atob(result)).not.toThrow();
  });

  it.skipIf(!canvasSupported())("returns base64 PNG string with annotations", async () => {
    const canvas = document.createElement("canvas");
    canvas.width = 100;
    canvas.height = 100;
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = "#FFFFFF";
    ctx.fillRect(0, 0, 100, 100);
    const base64 = canvas.toDataURL("image/png").replace(/^data:image\/png;base64,/, "");

    const annotation: ArrowAnnotation = {
      id: "t",
      tool: "arrow",
      color: "#FF0000",
      strokeWidth: 2,
      startX: 5,
      startY: 5,
      endX: 40,
      endY: 40,
    };
    const result = await compositeImage(base64, 0, 0, 50, 50, [annotation]);
    expect(result).toBeTruthy();
    expect(() => atob(result)).not.toThrow();
  });
});
