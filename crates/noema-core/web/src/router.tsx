import { createRouter } from "@tanstack/react-router";
import { routeTree } from "./routeTree.gen";

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
  defaultPendingMinMs: 150,
  defaultPendingComponent: RoutePending,
  defaultNotFoundComponent: NotFoundRoute
});

function RoutePending() {
  return (
    <div data-slot="route-pending" aria-label="Loading">
      Loading
    </div>
  );
}

function NotFoundRoute() {
  return null;
}

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
