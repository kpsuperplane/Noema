import React from "react";
import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, type Transition, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { ErrorMarker } from "@/components/ErrorMarker";
import { AppBootSkeleton } from "./AppBootSkeleton";

const bootWipeTransition = {
  type: "tween",
  duration: 0.4,
  ease: [0.42, 0, 0.58, 1]
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
            key="boot-glimmer-wipe"
            aria-hidden="true"
            inert
            initial={false}
            animate={{ maskPosition: "0% 0%" }}
            exit={{ maskPosition: "0% 60%" }}
            transition={reduceMotion ? { duration: 0 } : bootWipeTransition}
            {...stylex.props(styles.bootWipe)}
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
    zIndex: 100,
    overflow: "hidden",
    pointerEvents: "none",
    maskImage:
      "linear-gradient(to bottom, black 0%, black 33.333%, transparent 40%, transparent 100%)",
    WebkitMaskImage:
      "linear-gradient(to bottom, black 0%, black 33.333%, transparent 40%, transparent 100%)",
    maskPosition: "0% 0%",
    WebkitMaskPosition: "0% 0%",
    maskRepeat: "no-repeat",
    WebkitMaskRepeat: "no-repeat",
    maskSize: "100% 300%",
    WebkitMaskSize: "100% 300%",
    willChange: "mask-position"
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
