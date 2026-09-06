import React from "react";
import * as stylex from "@stylexjs/stylex";
import { Button } from "@astryxdesign/core/Button";
import { AnimatePresence, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { SetupFrame, SetupCard, SetupActions } from "./SetupFrame";
import { RenderErrorBoundary } from "@/components/errors/RenderErrorBoundary";
import {
  pageWaveLeadingTransition,
  pageWaveTrailingTransition
} from "@/motion/pageWave";
import { AppBootSkeleton } from "./AppBootSkeleton";

export function AppBootBoundary({ children }: { children: React.ReactNode }) {
  return (
    <RenderErrorBoundary
      errorScope="app.runtime"
      fallback={({ retry }) => <AppLoadError actionLabel="Retry" onAction={retry} />}
    >
      <React.Suspense fallback={<AppBootSkeleton />}>
        <AppBootReveal>{children}</AppBootReveal>
      </React.Suspense>
    </RenderErrorBoundary>
  );
}

export function AppFatalBoundary({ children }: { children: React.ReactNode }) {
  return (
    <RenderErrorBoundary
      errorScope="app.providers"
      fallback={() => <AppBootstrapError />}
    >
      {children}
    </RenderErrorBoundary>
  );
}

export function AppBootstrapError() {
  return (
    <AppLoadError
      actionLabel="Reload Noema"
      onAction={() => window.location.reload()}
    />
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

function AppLoadError({
  actionLabel,
  onAction
}: {
  actionLabel: string;
  onAction: () => void;
}) {
  return (
    <SetupFrame>
      <SetupCard title="Noema could not load" intro="Try opening it again.">
        <SetupActions><Button variant="primary" label={actionLabel} onClick={onAction} /></SetupActions>
      </SetupCard>
    </SetupFrame>
  );
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
  }
});
