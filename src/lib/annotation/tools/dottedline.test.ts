import { describe, expect, it, vi } from "vitest";
import { dottedLineTool } from "./dottedline.ts";
import type { AnnotationState, DottedLineAnnotation } from "./types.ts";

function makeState(overrides: Partial<AnnotationState> = {}): AnnotationState {
  return {
    activeAnnotation: null,
    setActiveAnnotation: vi.fn(),
    commitAnnotation: vi.fn(),
    color: "#0000FF",
    strokeWidth: 2,
    ...overrides,
  };
}

describe("dottedLineTool", () => {
  it("creates dottedline annotation on mouseDown", () => {
    const state = makeState();
    dottedLineTool.onMouseDown(10, 20, state);
    const arg = (state.setActiveAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as DottedLineAnnotation;
    expect(arg.tool).toBe("dottedline");
    expect(arg.startX).toBe(10);
  });

  it("commits with sufficient length", () => {
    const active: DottedLineAnnotation = {
      id: "t",
      tool: "dottedline",
      color: "#0000FF",
      strokeWidth: 2,
      startX: 0,
      startY: 0,
      endX: 80,
      endY: 80,
    };
    const state = makeState({ activeAnnotation: active });
    dottedLineTool.onMouseUp(80, 80, state);
    expect(state.commitAnnotation).toHaveBeenCalled();
  });

  it("render uses setLineDash", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 200;
    canvas.height = 200;
    const ctx = canvas.getContext("2d");
    if (!ctx) return; // jsdom lacks canvas support
    const spy = vi.spyOn(ctx, "setLineDash");
    const annotation: DottedLineAnnotation = {
      id: "t",
      tool: "dottedline",
      color: "#0000FF",
      strokeWidth: 2,
      startX: 10,
      startY: 10,
      endX: 100,
      endY: 100,
    };
    dottedLineTool.render(ctx, annotation);
    expect(spy).toHaveBeenCalledWith([6, 6]);
  });
});
