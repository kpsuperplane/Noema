import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/tools/web")({
  component: WebSettingsRoute
});

function WebSettingsRoute() {
  return <SettingsSurface section="tools-web" />;
}
