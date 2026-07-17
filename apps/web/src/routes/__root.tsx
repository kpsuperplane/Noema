import { createRootRoute, Outlet } from "@tanstack/react-router";
import { AppRoot } from "@/app/App";
import { AppBootBoundary } from "@/components/shell/AppBootBoundary";

export const Route = createRootRoute({
  component: RootRoute
});

function RootRoute() {
  return (
    <AppBootBoundary>
      <AppRoot>
        <Outlet />
      </AppRoot>
    </AppBootBoundary>
  );
}
