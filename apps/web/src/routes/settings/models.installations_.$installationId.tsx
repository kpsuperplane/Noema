import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/models/installations_/$installationId")({
  component: LocalModelInstallationSettingsRoute
});

function LocalModelInstallationSettingsRoute() {
  const { installationId } = Route.useParams();
  return <SettingsSurface section="models" installationId={installationId} />;
}
