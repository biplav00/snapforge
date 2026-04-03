import type { Tool, ToolType } from "./types.ts";
import { arrowTool } from "./arrow.ts";
import { rectTool } from "./rect.ts";
import { lineTool } from "./line.ts";
import { freehandTool } from "./freehand.ts";
import { textTool } from "./text.ts";
import { circleTool } from "./circle.ts";
import { highlightTool } from "./highlight.ts";
import { stepsTool } from "./steps.ts";
import { blurTool } from "./blur.ts";
import { colorPickerTool } from "./colorpicker.ts";
import { measureTool } from "./measure.ts";

const tools: Record<ToolType, Tool> = {
  arrow: arrowTool,
  rect: rectTool,
  line: lineTool,
  freehand: freehandTool,
  text: textTool,
  circle: circleTool,
  highlight: highlightTool,
  steps: stepsTool,
  blur: blurTool,
  colorpicker: colorPickerTool,
  measure: measureTool,
};

export function getTool(type: ToolType): Tool {
  return tools[type];
}

export function renderAnnotation(ctx: CanvasRenderingContext2D, annotation: { tool: ToolType } & Record<string, unknown>) {
  const tool = tools[annotation.tool];
  if (tool) {
    tool.render(ctx, annotation as never);
  }
}
