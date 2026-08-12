import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/system/clients_/$clientId")({
  component: ClientSettingsRoute
});

function ClientSettingsRoute() {
  const { clientId } = Route.useParams();
  return <SettingsSurface section="system-clients" clientId={clientId} />;
}
