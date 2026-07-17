import type { McpSettingsQuery } from "@/generated/graphql";

export type McpSettingsServer = McpSettingsQuery["mcpServers"][number];

export type McpMetadataRow = {
  label: string;
  value: string;
};

export function mcpEnabledLabel(
  server: Pick<McpSettingsServer, "enabled" | "healthStatus" | "toolCount">
) {
  if (server.enabled) return "Enabled";
  if (server.healthStatus === "healthy" && server.toolCount > 0) {
    return "Needs tool calibration";
  }
  if (server.healthStatus === "unavailable") return "Setup unavailable";
  return "Disabled";
}

export function mcpToolCountLabel(toolCount: number) {
  return `${toolCount} ${toolCount === 1 ? "tool" : "tools"}`;
}

export function mcpStatusLabel(status: string) {
  if (status === "none") return "Not required";
  return titleCaseStatus(status);
}

export function mcpMetadataRows(server: McpSettingsServer): McpMetadataRow[] {
  return [
    { label: "Server id", value: server.mcpServerId },
    { label: "Transport", value: server.transportKind },
    { label: "Tools", value: mcpToolCountLabel(server.toolCount) },
    { label: "Health", value: mcpStatusLabel(server.healthStatus) },
    { label: "Auth", value: mcpStatusLabel(server.authStatus) }
  ];
}

function titleCaseStatus(value: string) {
  return value
    .split(/[_\s-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join(" ");
}
