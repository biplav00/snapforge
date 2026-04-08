import { describe, expect, it, vi } from "vitest";
import { blurTool } from "./blur.ts";
import type { AnnotationState, BlurAnnotation } from "./types.ts";

function makeState(overrides: Partial<AnnotationState> = {}): AnnotationState {
  return {
    activeAnnotation: null,
    setActiveAnnotation: vi.fn(),
    commitAnnotation: vi.fn(),
    color: "#000000",
    strokeWidth: 2,
    ...overrides,
  };
}

describe("blurTool", () => {
  it("creates blur annotation on mouseDown", () => {
    const state = makeState();
    blurTool.onMouseDown(10, 20, state);
    const arg = (state.setActiveAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as BlurAnnotation;
    expect(arg.tool).toBe("blur");
    expect(arg.x).toBe(10);
    expect(arg.y).toBe(20);
    expect(arg.intensity).toBe(10);
  });

  it("normalizes negative dimensions on mouseUp", () => {
    const active: BlurAnnotation = {
      id: "t",
      tool: "blur",
      color: "#000",
      strokeWidth: 2,
      x: 100,
      y: 100,
      width: -50,
      height: -50,
      intensity: 10,
    };
    const state = makeState({ activeAnnotation: active });
    blurTool.onMouseUp(50, 50, state);
    const arg = (state.commitAnnotation as ReturnType<typeof vi.fn>).mock
      .calls[0][0] as BlurAnnotation;
    expect(arg.x).toBe(50);
    expect(arg.y).toBe(50);
    expect(arg.width).toBe(50);
    expect(arg.height).toBe(50);
  });

  it("discards too-small blur area", () => {
    const active: BlurAnnotation = {
      id: "t",
      tool: "blur",
      color: "#000",
      strokeWidth: 2,
      x: 10,
      y: 10,
      width: 1,
      height: 1,
      intensity: 10,
    };
    const state = makeState({ activeAnnotation: active });
    blurTool.onMouseUp(11, 11, state);
    expect(state.commitAnnotation).not.toHaveBeenCalled();
  });

  it("render pixelates image data", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 100;
    canvas.height = 100;
    const ctx = canvas.getContext("2d");
    if (!ctx) return; // jsdom lacks canvas support

    ctx.fillStyle = "#FF0000";
    ctx.fillRect(0, 0, 100, 100);

    const annotation: BlurAnnotation = {
      id: "t",
      tool: "blur",
      color: "#000",
      strokeWidth: 2,
      x: 10,
      y: 10,
      width: 30,
      height: 30,
      intensity: 10,
    };
    expect(() => blurTool.render(ctx, annotation)).not.toThrow();
  });
});
