import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/system/clients")({
  component: ClientsSettingsRoute
});

function ClientsSettingsRoute() {
  return <SettingsSurface section="system-clients" />;
}
