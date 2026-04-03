import type { Tool, ToolType } from "./types.ts";
import { arrowTool } from "./arrow.ts";
import { rectTool } from "./rect.ts";
import { lineTool } from "./line.ts";
import { freehandTool } from "./freehand.ts";
import { textTool } from "./text.ts";

const tools: Record<ToolType, Tool> = {
  arrow: arrowTool,
  rect: rectTool,
  line: lineTool,
  freehand: freehandTool,
  text: textTool,
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
