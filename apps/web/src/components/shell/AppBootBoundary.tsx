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
            animate={{ "--boot-reveal-size": "0vmax" }}
            exit={{ "--boot-reveal-size": "560vmax" }}
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
    "--boot-reveal-size": "0vmax",
    maskImage:
      "linear-gradient(black, black), radial-gradient(circle, black 0%, black 42%, rgb(0 0 0 / 0.96) 58%, rgb(0 0 0 / 0.72) 63%, rgb(0 0 0 / 0.28) 67%, rgb(0 0 0 / 0.04) 72%, transparent 100%)",
    WebkitMaskImage:
      "linear-gradient(black, black), radial-gradient(circle, black 0%, black 42%, rgb(0 0 0 / 0.96) 58%, rgb(0 0 0 / 0.72) 63%, rgb(0 0 0 / 0.28) 67%, rgb(0 0 0 / 0.04) 72%, transparent 100%)",
    maskComposite: "exclude",
    WebkitMaskComposite: "xor",
    maskPosition: "0 0, 50% 100%",
    WebkitMaskPosition: "0 0, 50% 100%",
    maskRepeat: "no-repeat",
    WebkitMaskRepeat: "no-repeat",
    maskSize:
      "100% 100%, var(--boot-reveal-size) var(--boot-reveal-size)",
    WebkitMaskSize:
      "100% 100%, var(--boot-reveal-size) var(--boot-reveal-size)",
    willChange: "mask-size"
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
