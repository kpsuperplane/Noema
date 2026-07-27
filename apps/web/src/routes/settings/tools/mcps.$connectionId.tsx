import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/tools/mcps/$connectionId")({
  component: McpConnectionSettingsRoute
});

function McpConnectionSettingsRoute() {
  const { connectionId } = Route.useParams();
  return <SettingsSurface section="tools-mcps" connectionId={connectionId} />;
}
