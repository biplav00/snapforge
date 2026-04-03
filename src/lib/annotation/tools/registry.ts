import type { Tool, ToolType } from "./types.ts";
import { arrowTool } from "./arrow.ts";
import { rectTool } from "./rect.ts";
import { lineTool } from "./line.ts";
import { freehandTool } from "./freehand.ts";

const tools: Record<ToolType, Tool> = {
  arrow: arrowTool,
  rect: rectTool,
  line: lineTool,
  freehand: freehandTool,
  text: arrowTool, // placeholder — text tool added in Task 4
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
