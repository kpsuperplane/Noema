import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/settings")({
  component: SettingsLayoutRoute
});

function SettingsLayoutRoute() {
  return <Outlet />;
}
