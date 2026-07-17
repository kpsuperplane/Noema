import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/tools/mcps")({
  component: McpSettingsRoute
});

function McpSettingsRoute() {
  return <SettingsSurface section="tools-mcps" />;
}
