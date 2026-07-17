import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/safety/usage")({
  component: UsageSettingsRoute
});

function UsageSettingsRoute() {
  return <SettingsSurface section="safety-usage" />;
}
