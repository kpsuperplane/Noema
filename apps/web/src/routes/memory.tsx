import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/memory")({
  component: MemoryLayoutRoute
});

function MemoryLayoutRoute() {
  return <Outlet />;
}
