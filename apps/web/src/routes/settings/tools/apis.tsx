import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/tools/apis")({
  component: ApiSettingsRoute
});

function ApiSettingsRoute() {
  return <SettingsSurface section="tools-apis" />;
}
