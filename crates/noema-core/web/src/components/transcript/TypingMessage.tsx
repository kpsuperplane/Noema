import * as stylex from "@stylexjs/stylex";
import { TranscriptRow } from "./TranscriptRow";

const dotAnimation = {
  animationDuration: "1s",
  animationIterationCount: "infinite",
  animationName: "transcript-typing-dot-bounce",
  animationTimingFunction: "ease"
} as const;

const styles = stylex.create({
  bubble: {
    display: "flex",
    width: "fit-content",
    maxWidth: "100%",
    minWidth: 0,
    flexDirection: "column",
    gap: 4
  },
  content: {
    display: "flex",
    width: 58,
    minHeight: 36,
    alignItems: "center",
    justifyContent: "center",
    gap: 6,
    borderRadius: 24,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "transparent",
    backgroundColor: "var(--muted)",
    paddingBlock: 8,
    paddingInline: 12
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 999,
    backgroundColor: "color-mix(in srgb, var(--muted-foreground) 70%, transparent)",
    ...dotAnimation
  },
  firstDot: {
    animationDelay: "-0.24s"
  },
  secondDot: {
    animationDelay: "-0.12s"
  }
});

export function TypingMessage({ showAvatar }: { showAvatar: boolean }) {
  return (
    <TranscriptRow lane="assistant" showAvatar={showAvatar}>
      <div {...stylex.props(styles.bubble)}>
        <div {...stylex.props(styles.content)} aria-label="Noema is typing" role="status">
          <span {...stylex.props(styles.dot, styles.firstDot)} />
          <span {...stylex.props(styles.dot, styles.secondDot)} />
          <span {...stylex.props(styles.dot)} />
        </div>
      </div>
    </TranscriptRow>
  );
}
