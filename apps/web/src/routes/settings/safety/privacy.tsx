import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/safety/privacy")({
  component: PrivacySettingsRoute
});

function PrivacySettingsRoute() {
  return <SettingsSurface section="safety-privacy" />;
}
