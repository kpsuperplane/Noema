import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/safety/identities")({
  component: IdentitiesSettingsRoute
});

function IdentitiesSettingsRoute() {
  return <SettingsSurface section="safety-identities" />;
}
