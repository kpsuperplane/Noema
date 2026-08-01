import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/system/notifications")({
  component: NotificationsSettingsRoute
});

function NotificationsSettingsRoute() {
  return <SettingsSurface section="system-notifications" />;
}
