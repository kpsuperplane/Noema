import type { McpServerFieldsFragment } from "@/generated/graphql";

export type McpSettingsServer = McpServerFieldsFragment;

export function mcpToolCountLabel(toolCount: number) {
  return `${toolCount} ${toolCount === 1 ? "tool" : "tools"}`;
}
