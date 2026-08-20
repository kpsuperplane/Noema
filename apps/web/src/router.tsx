import { createRouter } from "@tanstack/react-router";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { useAppRuntime } from "./app/AppRuntimeContext";
import { pathnamesSharePageSurface } from "./app/routes";
import { shouldUseIosPageFade } from "./motion/pageWave";
import { routeTree } from "./routeTree.gen";
import { ErrorMarker } from "./components/ErrorMarker";

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
  defaultPendingMinMs: 150,
  defaultViewTransition: shouldUseIosPageFade()
    ? false
    : { types: ({ pathChanged }) => pathChanged ? ["noema-page-wave"] : false },
  defaultPendingComponent: RoutePending,
  defaultErrorComponent: RouteError,
  defaultNotFoundComponent: NotFoundRoute
});

router.subscribe("onBeforeNavigate", ({ fromLocation, toLocation }) => {
  if (fromLocation && pathnamesSharePageSurface(fromLocation.pathname, toLocation.pathname)) {
    router.shouldViewTransition = false;
  }
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

function RouteError({ reset }: { reset: () => void }) {
  return (
    <section
      aria-label="Page recovery"
      {...stylex.props(styles.errorSurface)}
    >
      <ErrorMarker
        message="This page could not display. Other areas remain available."
        recoverable={false}
      />
      <Button
        type="button"
        variant="secondary"
        label="Retry page"
        onClick={reset}
      />
    </section>
  );
}

const styles = stylex.create({
  errorSurface: {
    display: "grid",
    minHeight: "100%",
    alignContent: "center",
    justifyItems: "center",
    gap: "var(--spacing-3)",
    padding: "var(--spacing-4)"
  }
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
