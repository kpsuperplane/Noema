import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/agents")({
  component: AgentsSettingsRoute
});

function AgentsSettingsRoute() {
  return <SettingsSurface section="agents" />;
}
