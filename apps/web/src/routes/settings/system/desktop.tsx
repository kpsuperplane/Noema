import { Navigate, createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";
import { isTauriRuntime } from "@/graphql/transportMode";

export const Route = createFileRoute("/settings/system/desktop")({
  component: DesktopSettingsRoute,
});

function DesktopSettingsRoute() {
  if (!isTauriRuntime()) return <Navigate to="/settings/agents" replace />;
  return <SettingsSurface section="system-desktop" />;
}
