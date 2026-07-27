import type { McpSettingsQuery } from "@/generated/graphql";

export type McpSettingsServer = McpSettingsQuery["mcpServers"][number];

export function mcpToolCountLabel(toolCount: number) {
  return `${toolCount} ${toolCount === 1 ? "tool" : "tools"}`;
}
