import { createRouter } from "@tanstack/react-router";
import { useAppRuntime } from "./app/AppRuntimeContext";
import { routeTree } from "./routeTree.gen";

const useIosPageFade = isIosDevice();
if (useIosPageFade && typeof document !== "undefined") {
  document.documentElement.dataset.pageTransition = "fade";
}

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
  defaultPendingMinMs: 150,
  defaultViewTransition: {
    types: ({ pathChanged }) => pathChanged
      ? [useIosPageFade ? "noema-page-fade" : "noema-page-wave"]
      : false
  },
  defaultPendingComponent: RoutePending,
  defaultNotFoundComponent: NotFoundRoute
});

function isIosDevice() {
  if (typeof navigator === "undefined") return false;

  return /iP(?:ad|hone|od)/.test(navigator.userAgent) ||
    (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
}

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
