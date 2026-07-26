import * as stylex from "@stylexjs/stylex";
import type { MotionValue } from "motion/react";
import * as m from "motion/react-m";

export function ShellContentFrame({
  deckX,
  hasSidebar,
  navOpen
}: {
  deckX: MotionValue<number>;
  hasSidebar: boolean;
  navOpen: boolean;
}) {
  const position = hasSidebar ? styles.withSidebar : styles.primary;
  return (
    <>
      <div
        aria-hidden="true"
        data-slot="shell-navbar-background"
        {...stylex.props(styles.navbarBackground)}
      />
      <div
        aria-hidden="true"
        data-slot="shell-bottom-gutter"
        {...stylex.props(styles.bottomGutter)}
      />
      <m.div
        aria-hidden="true"
        data-slot="shell-content-frame"
        style={{ x: deckX }}
        {...stylex.props(styles.frame, styles.background, position, navOpen && styles.navOpen)}
      />
      <m.div
        aria-hidden="true"
        data-slot="shell-content-frame-outline"
        style={{ x: deckX }}
        {...stylex.props(styles.frame, styles.outline, position, navOpen && styles.navOpen)}
      />
    </>
  );
}

const styles = stylex.create({
  frame: {
    position: "fixed",
    top: "calc(52px + var(--shell-visual-viewport-offset-top, 0px))",
    zIndex: 29,
    height: "calc(var(--shell-visual-viewport-height, 100dvh) - 60px)",
    borderRadius: 18,
    cornerShape: "var(--corner-shape-page)",
    pointerEvents: "none",
    transitionProperty: "top, right, left, height, scale, border-radius, box-shadow",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (max-width: 760px)": {
      height: "calc(var(--shell-visual-viewport-height, 100dvh) - 52px)",
      borderRadius: "18px 18px 0 0"
    },
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    }
  },
  background: {
    backgroundColor: "var(--background)"
  },
  outline: {
    zIndex: 32,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    boxShadow: "0 0 24px color-mix(in srgb, var(--pine-700), transparent 80%)",
    "@media (max-width: 760px)": {
      borderWidth: 0
    }
  },
  navbarBackground: {
    position: "fixed",
    top: "var(--shell-visual-viewport-offset-top, 0px)",
    right: 0,
    left: 0,
    zIndex: 31,
    height: 52,
    backgroundColor: "var(--pine-50)",
    pointerEvents: "none",
    transitionProperty: "top",
    transitionDuration: "var(--motion-spring-standard-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    }
  },
  bottomGutter: {
    position: "fixed",
    right: 0,
    bottom: 0,
    left: 0,
    zIndex: 31,
    height: 8,
    backgroundColor: "var(--pine-50)",
    pointerEvents: "none",
    "@media (max-width: 760px)": {
      display: "none"
    }
  },
  primary: {
    right: 8,
    left: 8,
    "@media (max-width: 760px)": {
      right: 0,
      left: 0
    }
  },
  withSidebar: {
    right: 8,
    left: "var(--shell-sidebar-width)",
    "@media (max-width: 760px)": {
      right: 0,
      left: 0
    }
  },
  navOpen: {
    "@media (min-width: 761px)": {
      scale: 0.97
    },
    "@media (max-width: 760px)": {
      borderRadius: 18
    }
  }
});
