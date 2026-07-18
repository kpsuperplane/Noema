import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/work")({ component: WorkLayoutRoute });

function WorkLayoutRoute() {
  return <Outlet />;
}
