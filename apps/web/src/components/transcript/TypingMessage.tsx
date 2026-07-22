import * as stylex from "@stylexjs/stylex";

const dotAnimation = {
  animationDuration: "1s",
  animationIterationCount: "infinite",
  animationName: "transcript-typing-dot-bounce",
  animationTimingFunction: "ease"
} as const;

const styles = stylex.create({
  content: {
    display: "flex",
    width: "100%",
    minHeight: 40,
    alignItems: "center",
    justifyContent: "center",
    gap: 6
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 999,
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
