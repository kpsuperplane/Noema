import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/tools/connections")({
  component: SettingsConnectionsRoute
});

function SettingsConnectionsRoute() {
  return <SettingsSurface section="tools-connections" />;
}
