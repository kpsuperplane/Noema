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
      <m.div
        aria-hidden="true"
        data-slot="shell-content-frame"
        style={{ x: deckX }}
        {...stylex.props(styles.frame, styles.background, position, navOpen && styles.navOpen)}
      />
      <m.div
        aria-hidden="true"
        data-slot="shell-content-frame-mask"
        style={{ x: deckX }}
        {...stylex.props(styles.frame, styles.mask, position, navOpen && styles.navOpen)}
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
    top: "calc(52px + var(--shell-chrome-viewport-top, 0px))",
    zIndex: 29,
    height: "calc(var(--shell-chrome-viewport-height, 100dvh) - 60px)",
    borderRadius: "var(--radius-page)",
    cornerShape: "var(--corner-shape-page)",
    pointerEvents: "none",
    transitionProperty: "right, left, scale, border-radius, box-shadow",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (max-width: 760px)": {
      height: "calc(var(--shell-chrome-viewport-height, 100dvh) - 52px)",
      borderRadius: "var(--radius-page) var(--radius-page) 0 0"
    },
    "@media (prefers-reduced-motion: reduce)": {
      transition: "none"
    }
  },
  background: {
    backgroundColor: "var(--background)"
  },
  mask: {
    zIndex: 31,
    clipPath: "inset(-100vmax -100vmax -100vmax 0)",
    boxShadow: "0 0 0 100vmax var(--pine-50)"
  },
  outline: {
    zIndex: 32,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    boxShadow: "var(--shadow-shell-frame)",
    "@media (max-width: 760px)": {
      borderWidth: 0
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
      borderRadius: "var(--radius-page)"
    }
  }
});
