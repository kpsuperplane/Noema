import * as stylex from "@stylexjs/stylex";

const typingDotBounce = stylex.keyframes({
  "0%, 100%": {
    transform: "translateY(-25%)",
    animationTimingFunction: "cubic-bezier(0.8, 0, 1, 1)"
  },
  "50%": {
    transform: "none",
    animationTimingFunction: "cubic-bezier(0, 0, 0.2, 1)"
  }
});

const dotAnimation = {
  animationDuration: "1s",
  animationIterationCount: "infinite",
  animationName: typingDotBounce,
  animationTimingFunction: "ease"
} as const;

const styles = stylex.create({
  content: {
    display: "flex",
    width: "100%",
    minHeight: 40,
    alignItems: "center",
    justifyContent: "center",
    gap: "var(--spacing-1-5)"
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "color-mix(in srgb, var(--muted-foreground) 70%, transparent)",
    ...dotAnimation,
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none"
    }
  },
  firstDot: {
    animationDelay: "-0.24s"
  },
  secondDot: {
    animationDelay: "-0.12s"
  }
});

export function TypingMessageContent() {
  return (
    <div {...stylex.props(styles.content)} aria-label="Noema is typing" role="status">
      <span {...stylex.props(styles.dot, styles.firstDot)} />
      <span {...stylex.props(styles.dot, styles.secondDot)} />
      <span {...stylex.props(styles.dot)} />
    </div>
  );
}
