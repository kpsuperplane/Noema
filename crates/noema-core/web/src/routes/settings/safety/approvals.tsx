import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/safety/approvals")({
  component: ApprovalsSettingsRoute
});

function ApprovalsSettingsRoute() {
  return <SettingsSurface section="safety-approvals" />;
}
