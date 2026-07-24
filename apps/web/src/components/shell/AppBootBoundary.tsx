import React from "react";
import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, type Transition, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { ErrorMarker } from "@/components/ErrorMarker";
import { AppBootSkeleton } from "./AppBootSkeleton";

const bootGlimmerWaveTransition = {
  type: "tween",
  duration: 0.2,
  ease: [0.42, 0, 0.58, 1],
  opacity: { duration: 0, delay: 0.2 }
} as const satisfies Transition;

const bootContentWaveTransition = {
  ...bootGlimmerWaveTransition,
  delay: 0.08,
  opacity: { duration: 0, delay: 0.28 }
} as const satisfies Transition;

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
            aria-hidden="true"
            inert
            initial={false}
            animate={{ "--boot-reveal-radius": "0vmax" }}
            exit={{ "--boot-reveal-radius": "220vmax", opacity: 0 }}
            transition={reduceMotion ? { duration: 0 } : bootContentWaveTransition}
            {...stylex.props(styles.bootWipe, styles.bootWhiteWave)}
          />
        ) : null}
        {!revealed ? (
          <m.div
            key="boot-glimmer-wave"
            data-slot="boot-glimmer-wave"
            aria-hidden="true"
            inert
            initial={false}
            animate={{ "--boot-reveal-radius": "0vmax" }}
            exit={{ "--boot-reveal-radius": "220vmax", opacity: 0 }}
            transition={reduceMotion ? { duration: 0 } : bootGlimmerWaveTransition}
            {...stylex.props(styles.bootWipe, styles.bootGlimmerWave)}
          >
            <AppBootSkeleton />
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
            <ErrorMarker message={this.state.error.message} />
          </div>
        </main>
      );
    }

    return this.props.children;
  }
}

const styles = stylex.create({
  bootWipe: {
    position: "fixed",
    inset: 0,
    overflow: "hidden",
    pointerEvents: "none",
    "--boot-reveal-radius": "0vmax",
    maskImage:
      "radial-gradient(circle at 50% 100%, transparent 0, transparent max(0vmax, calc(var(--boot-reveal-radius) - 72vmax)), rgb(0 0 0 / 0.04) max(0vmax, calc(var(--boot-reveal-radius) - 64vmax)), rgb(0 0 0 / 0.28) max(0vmax, calc(var(--boot-reveal-radius) - 50vmax)), rgb(0 0 0 / 0.72) max(0vmax, calc(var(--boot-reveal-radius) - 36vmax)), rgb(0 0 0 / 0.96) max(0vmax, calc(var(--boot-reveal-radius) - 24vmax)), black var(--boot-reveal-radius))",
    WebkitMaskImage:
      "radial-gradient(circle at 50% 100%, transparent 0, transparent max(0vmax, calc(var(--boot-reveal-radius) - 72vmax)), rgb(0 0 0 / 0.04) max(0vmax, calc(var(--boot-reveal-radius) - 64vmax)), rgb(0 0 0 / 0.28) max(0vmax, calc(var(--boot-reveal-radius) - 50vmax)), rgb(0 0 0 / 0.72) max(0vmax, calc(var(--boot-reveal-radius) - 36vmax)), rgb(0 0 0 / 0.96) max(0vmax, calc(var(--boot-reveal-radius) - 24vmax)), black var(--boot-reveal-radius))",
    willChange: "mask-image"
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
    padding: 24
  },
  errorFrame: {
    display: "grid",
    width: "min(520px, 100%)",
    justifyItems: "center",
    gap: 18
  }
});
