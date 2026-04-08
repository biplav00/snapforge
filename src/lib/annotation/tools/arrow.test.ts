import { describe, expect, it, vi } from "vitest";
import { arrowTool } from "./arrow.ts";
import type { AnnotationState, ArrowAnnotation } from "./types.ts";

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

describe("arrowTool", () => {
  it("creates annotation on mouseDown", () => {
    const state = makeState();
    arrowTool.onMouseDown(10, 20, state);
    expect(state.setActiveAnnotation).toHaveBeenCalledTimes(1);
    const arg = (state.setActiveAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as ArrowAnnotation;
    expect(arg.tool).toBe("arrow");
    expect(arg.startX).toBe(10);
    expect(arg.startY).toBe(20);
    expect(arg.color).toBe("#FF0000");
  });

  it("updates endX/endY on mouseMove", () => {
    const active: ArrowAnnotation = {
      id: "test",
      tool: "arrow",
      color: "#FF0000",
      strokeWidth: 2,
      startX: 10,
      startY: 20,
      endX: 10,
      endY: 20,
    };
    const state = makeState({ activeAnnotation: active });
    arrowTool.onMouseMove(50, 60, state);
    const arg = (state.setActiveAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as ArrowAnnotation;
    expect(arg.endX).toBe(50);
    expect(arg.endY).toBe(60);
  });

  it("commits on mouseUp when distance > 3", () => {
    const active: ArrowAnnotation = {
      id: "test",
      tool: "arrow",
      color: "#FF0000",
      strokeWidth: 2,
      startX: 0,
      startY: 0,
      endX: 100,
      endY: 100,
    };
    const state = makeState({ activeAnnotation: active });
    arrowTool.onMouseUp(100, 100, state);
    expect(state.commitAnnotation).toHaveBeenCalledWith(active);
  });

  it("discards on mouseUp when distance <= 3", () => {
    const active: ArrowAnnotation = {
      id: "test",
      tool: "arrow",
      color: "#FF0000",
      strokeWidth: 2,
      startX: 0,
      startY: 0,
      endX: 1,
      endY: 1,
    };
    const state = makeState({ activeAnnotation: active });
    arrowTool.onMouseUp(1, 1, state);
    expect(state.commitAnnotation).not.toHaveBeenCalled();
    expect(state.setActiveAnnotation).toHaveBeenCalledWith(null);
  });

  it("ignores mouseMove with no active annotation", () => {
    const state = makeState();
    arrowTool.onMouseMove(50, 60, state);
    expect(state.setActiveAnnotation).not.toHaveBeenCalled();
  });

  it("render does not throw", () => {
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
    expect(() => arrowTool.render(ctx, annotation)).not.toThrow();
  });
});
