import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/tools/apis/$connectionId")({
  component: ApiConnectionSettingsRoute
});

function ApiConnectionSettingsRoute() {
  const { connectionId } = Route.useParams();
  return <SettingsSurface section="tools-apis" connectionId={connectionId} />;
}
