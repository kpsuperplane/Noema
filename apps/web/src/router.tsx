import { createRouter } from "@tanstack/react-router";
import { useAppRuntime } from "./app/AppRuntimeContext";
import { shouldUseIosPageFade } from "./motion/pageWave";
import { routeTree } from "./routeTree.gen";

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
  defaultPendingMinMs: 150,
  defaultViewTransition: shouldUseIosPageFade()
    ? false
    : { types: ({ pathChanged }) => pathChanged ? ["noema-page-wave"] : false },
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
  return useAppRuntime().chatView;
}

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
