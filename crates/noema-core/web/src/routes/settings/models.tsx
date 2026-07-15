import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/models")({
  component: ModelsSettingsRoute
});

function ModelsSettingsRoute() {
  return <SettingsSurface section="models" />;
}
