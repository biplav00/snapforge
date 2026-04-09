import { arrowTool } from "./arrow.ts";
import { blurTool } from "./blur.ts";
import { circleTool } from "./circle.ts";
import { colorPickerTool } from "./colorpicker.ts";
import { dottedLineTool } from "./dottedline.ts";
import { freehandTool } from "./freehand.ts";
import { highlightTool } from "./highlight.ts";
import { lineTool } from "./line.ts";
import { measureTool } from "./measure.ts";
import { rectTool } from "./rect.ts";
import { stepsTool } from "./steps.ts";
import { textTool } from "./text.ts";
import type { Tool, ToolType } from "./types.ts";

const tools: Record<ToolType, Tool> = {
  arrow: arrowTool,
  rect: rectTool,
  line: lineTool,
  dottedline: dottedLineTool,
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

export function renderAnnotation(
  ctx: CanvasRenderingContext2D,
  annotation: import("./types.ts").Annotation,
) {
  const tool = tools[annotation.tool];
  if (tool) {
    tool.render(ctx, annotation);
  }
}
