import { describe, expect, it, vi } from "vitest";
import { lineTool } from "./line.ts";
import type { AnnotationState, LineAnnotation } from "./types.ts";

function makeState(overrides: Partial<AnnotationState> = {}): AnnotationState {
  return {
    activeAnnotation: null,
    setActiveAnnotation: vi.fn(),
    commitAnnotation: vi.fn(),
    color: "#00FF00",
    strokeWidth: 3,
    ...overrides,
  };
}

describe("lineTool", () => {
  it("creates annotation on mouseDown", () => {
    const state = makeState();
    lineTool.onMouseDown(5, 15, state);
    const arg = (state.setActiveAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as LineAnnotation;
    expect(arg.tool).toBe("line");
    expect(arg.startX).toBe(5);
    expect(arg.startY).toBe(15);
    expect(arg.strokeWidth).toBe(3);
  });

  it("commits line with sufficient length", () => {
    const active: LineAnnotation = {
      id: "t",
      tool: "line",
      color: "#00FF00",
      strokeWidth: 3,
      startX: 0,
      startY: 0,
      endX: 50,
      endY: 50,
    };
    const state = makeState({ activeAnnotation: active });
    lineTool.onMouseUp(50, 50, state);
    expect(state.commitAnnotation).toHaveBeenCalled();
  });

  it("discards too-short line", () => {
    const active: LineAnnotation = {
      id: "t",
      tool: "line",
      color: "#00FF00",
      strokeWidth: 3,
      startX: 0,
      startY: 0,
      endX: 1,
      endY: 0,
    };
    const state = makeState({ activeAnnotation: active });
    lineTool.onMouseUp(1, 0, state);
    expect(state.commitAnnotation).not.toHaveBeenCalled();
    expect(state.setActiveAnnotation).toHaveBeenCalledWith(null);
  });
});
