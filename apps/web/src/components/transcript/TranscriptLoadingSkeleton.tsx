import * as stylex from "@stylexjs/stylex";
import { skeletonGlimmerStyles } from "@/components/skeletonGlimmerStyles";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

const rows = [
  { role: "assistant", showAvatar: true, lines: [0.72, 0.46] },
  { role: "user", showAvatar: true, lines: [0.58] },
  { role: "assistant", showAvatar: true, lines: [0.86, 0.66, 0.34] }
] as const;

export function TranscriptLoadingSkeleton({ animateGlimmer = true }: { animateGlimmer?: boolean }) {
  return (
    <div {...stylex.props(styles.root)} aria-label="Loading chat">
      <div {...stylex.props(styles.stack)}>
        {rows.map((row, rowIndex) => (
          <div
            key={`${row.role}-${rowIndex}`}
            {...stylex.props(styles.row, row.role === "user" && styles.rowEnd)}
          >
            <TranscriptChatBubble
              role={row.role}
              showAvatar={row.showAvatar}
            >
              <div {...stylex.props(styles.lines)}>
                {row.lines.map((lineWidth, lineIndex) => (
                  <span
                    key={`${row.role}-${rowIndex}-${lineIndex}`}
                    {...stylex.props(
                      styles.line,
                      animateGlimmer && skeletonGlimmerStyles.animated
                    )}
                    style={{ width: `${Math.round(lineWidth * 100)}%` }}
                  />
                ))}
              </div>
            </TranscriptChatBubble>
          </div>
        ))}
      </div>
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    height: "100%",
    minHeight: 0,
    alignContent: "end",
    padding: "64px 24px max(80px, calc(var(--chat-composer-dock-height, 90px) + 16px))",
    overflow: "hidden",
    "@media (max-width: 760px)": {
      paddingInline: "var(--spacing-5)"
    }
  },
  stack: {
    display: "grid",
    width: "var(--chat-column-width)",
    maxWidth: "100%",
    minWidth: 0,
    gap: "calc(var(--spacing-4) + var(--spacing-0-5))",
    marginInline: "auto",
    paddingInline: "var(--spacing-0-5)"
  },
  row: {
    display: "flex",
    width: "100%",
    minWidth: 0
  },
  rowEnd: {
    justifyContent: "flex-end"
  },
  lines: {
    display: "grid",
    width: "min(420px, 72vw)",
    gap: "var(--spacing-2)",
    paddingBlock: "var(--spacing-0-5)"
  },
  line: {
    display: "block",
    height: 10,
    minWidth: 72,
    borderRadius: 6,
    backgroundColor: "var(--skeleton-glimmer-line)"
  }
});
