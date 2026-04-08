import { describe, expect, it, vi } from "vitest";
import { rectTool } from "./rect.ts";
import type { AnnotationState, RectAnnotation } from "./types.ts";

function makeState(overrides: Partial<AnnotationState> = {}): AnnotationState {
  return {
    activeAnnotation: null,
    setActiveAnnotation: vi.fn(),
    commitAnnotation: vi.fn(),
    color: "#FF0000",
    strokeWidth: 2,
    ...overrides,
  };
}

describe("rectTool", () => {
  it("creates rect on mouseDown", () => {
    const state = makeState();
    rectTool.onMouseDown(10, 20, state);
    const arg = (state.setActiveAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as RectAnnotation;
    expect(arg.tool).toBe("rect");
    expect(arg.x).toBe(10);
    expect(arg.y).toBe(20);
    expect(arg.width).toBe(0);
    expect(arg.height).toBe(0);
  });

  it("updates dimensions on mouseMove", () => {
    const active: RectAnnotation = {
      id: "t",
      tool: "rect",
      color: "#FF0000",
      strokeWidth: 2,
      x: 10,
      y: 20,
      width: 0,
      height: 0,
    };
    const state = makeState({ activeAnnotation: active });
    rectTool.onMouseMove(110, 120, state);
    const arg = (state.setActiveAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as RectAnnotation;
    expect(arg.width).toBe(100);
    expect(arg.height).toBe(100);
  });

  it("commits rect with sufficient size", () => {
    const active: RectAnnotation = {
      id: "t",
      tool: "rect",
      color: "#FF0000",
      strokeWidth: 2,
      x: 10,
      y: 20,
      width: 50,
      height: 50,
    };
    const state = makeState({ activeAnnotation: active });
    rectTool.onMouseUp(60, 70, state);
    expect(state.commitAnnotation).toHaveBeenCalled();
  });
});
