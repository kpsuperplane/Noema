import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/system/providers")({
  component: ProvidersSettingsRoute
});

function ProvidersSettingsRoute() {
  return <SettingsSurface section="system-providers" />;
}
