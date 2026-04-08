import { describe, expect, it } from "vitest";
import { getTool, renderAnnotation } from "./registry.ts";
import type { ArrowAnnotation, ToolType } from "./types.ts";

describe("registry", () => {
  const allTools: ToolType[] = [
    "arrow",
    "rect",
    "line",
    "dottedline",
    "freehand",
    "text",
    "circle",
    "highlight",
    "steps",
    "blur",
    "colorpicker",
    "measure",
  ];

  it("returns a tool for every ToolType", () => {
    for (const type_ of allTools) {
      const tool = getTool(type_);
      expect(tool).toBeDefined();
      expect(tool.onMouseDown).toBeTypeOf("function");
      expect(tool.onMouseMove).toBeTypeOf("function");
      expect(tool.onMouseUp).toBeTypeOf("function");
      expect(tool.render).toBeTypeOf("function");
    }
  });

  it("renderAnnotation does not throw for a valid annotation", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 200;
    canvas.height = 200;
    const ctx = canvas.getContext("2d");
    if (!ctx) return; // jsdom lacks canvas support

    const annotation: ArrowAnnotation = {
      id: "test",
      tool: "arrow",
      color: "#FF0000",
      strokeWidth: 2,
      startX: 10,
      startY: 10,
      endX: 100,
      endY: 100,
    };

    expect(() => renderAnnotation(ctx, annotation)).not.toThrow();
  });
});
