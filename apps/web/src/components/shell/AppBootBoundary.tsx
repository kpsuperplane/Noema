import React from "react";
import * as stylex from "@stylexjs/stylex";
import { Button } from "@astryxdesign/core/Button";
import { AnimatePresence, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { ErrorMarker } from "@/components/ErrorMarker";
import {
  pageWaveLeadingTransition,
  pageWaveTrailingTransition
} from "@/motion/pageWave";
import { AppBootSkeleton } from "./AppBootSkeleton";

export function AppBootBoundary({ children }: { children: React.ReactNode }) {
  return (
    <AppBootErrorBoundary>
      <React.Suspense fallback={<AppBootSkeleton />}>
        <AppBootReveal>{children}</AppBootReveal>
      </React.Suspense>
    </AppBootErrorBoundary>
  );
}

function AppBootReveal({ children }: { children: React.ReactNode }) {
  const [revealed, setRevealed] = React.useState(false);
  const reduceMotion = useReducedMotion();

  React.useEffect(() => {
    const frame = window.requestAnimationFrame(() => setRevealed(true));
    return () => window.cancelAnimationFrame(frame);
  }, []);

  return (
    <>
      {children}
      <AnimatePresence initial={false}>
        {!revealed ? (
          <m.div
            key="boot-content-wave"
            data-slot="boot-content-wave"
            data-page-wave="out"
            aria-hidden="true"
            inert
            initial={false}
            animate={{ "--page-wave-radius": "0vmax" }}
            exit={{ "--page-wave-radius": "220vmax", opacity: 0 }}
            transition={reduceMotion ? { duration: 0 } : pageWaveTrailingTransition}
            {...stylex.props(styles.waveMask, styles.bootWipe, styles.bootWhiteWave)}
          />
        ) : null}
        {!revealed ? (
          <m.div
            key="boot-glimmer-wave"
            data-slot="boot-glimmer-wave"
            data-page-wave="out"
            aria-hidden="true"
            inert
            initial={false}
            animate={{ "--page-wave-radius": "0vmax" }}
            exit={{ "--page-wave-radius": "220vmax", opacity: 0 }}
            transition={reduceMotion ? { duration: 0 } : pageWaveLeadingTransition}
            {...stylex.props(styles.waveMask, styles.bootWipe, styles.bootGlimmerWave)}
          >
            <AppBootSkeleton animateGlimmer={false} />
          </m.div>
        ) : null}
      </AnimatePresence>
    </>
  );
}

type AppBootErrorBoundaryState = {
  error: Error | null;
};

class AppBootErrorBoundary extends React.Component<
  { children: React.ReactNode },
  AppBootErrorBoundaryState
> {
  state: AppBootErrorBoundaryState = {
    error: null
  };

  static getDerivedStateFromError(error: unknown): AppBootErrorBoundaryState {
    return {
      error: error instanceof Error ? error : new Error("Noema could not load.")
    };
  }

  render() {
    if (this.state.error) {
      return (
        <main {...stylex.props(styles.root)} aria-label="Noema status">
          <div {...stylex.props(styles.errorFrame)}>
            <img src="/assets/noema-mark.svg" width="36" height="36" alt="" />
            <ErrorMarker message="Noema could not load." recoverable={false} />
            <Button
              type="button"
              variant="secondary"
              label="Retry"
              onClick={() => this.setState({ error: null })}
            />
          </div>
        </main>
      );
    }

    return this.props.children;
  }
}

const styles = stylex.create({
  waveMask: {
    "--page-wave-radius": "0vmax",
    willChange: "mask-image"
  },
  bootWipe: {
    position: "fixed",
    inset: 0,
    overflow: "hidden",
    clipPath: {
      default: "inset(52px 8px 8px round 18px)",
      "@media (max-width: 760px)":
        "inset(calc(52px + env(safe-area-inset-top, 0px)) 0 0 round 18px 18px 0 0)"
    },
    pointerEvents: "none"
  },
  bootWhiteWave: {
    zIndex: 99,
    backgroundColor: "var(--background)"
  },
  bootGlimmerWave: {
    zIndex: 100
  },
  root: {
    display: "grid",
    minHeight: "100dvh",
    placeItems: "center",
    backgroundColor: "var(--background)",
    padding: "var(--spacing-6)"
  },
  errorFrame: {
    display: "grid",
    width: "min(520px, 100%)",
    justifyItems: "center",
    gap: "calc(var(--spacing-4) + var(--spacing-0-5))"
  }
});
