import * as stylex from "@stylexjs/stylex";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

const rows = [
  { role: "assistant", showAvatar: true, lines: [0.72, 0.46] },
  { role: "user", showAvatar: true, lines: [0.58] },
  { role: "assistant", showAvatar: true, lines: [0.86, 0.66, 0.34] }
] as const;

export function TranscriptLoadingSkeleton() {
  return (
    <div {...stylex.props(styles.root)} aria-label="Loading chat">
      <div {...stylex.props(styles.stack)}>
        {rows.map((row, rowIndex) => (
          <TranscriptChatBubble
            key={`${row.role}-${rowIndex}`}
            role={row.role}
            showAvatar={row.showAvatar}
          >
            <div {...stylex.props(styles.lines)}>
              {row.lines.map((lineWidth, lineIndex) => (
                <span
                  key={`${row.role}-${rowIndex}-${lineIndex}`}
                  data-slot="skeleton-glimmer"
                  {...stylex.props(styles.line)}
                  style={{ width: `${Math.round(lineWidth * 100)}%` }}
                />
              ))}
            </div>
          </TranscriptChatBubble>
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
    padding: "64px 24px calc(var(--chat-composer-dock-height) + 42px)",
    overflow: "hidden",
    "@media (max-width: 760px)": {
      paddingInline: 20
    }
  },
  stack: {
    display: "grid",
    width: "100%",
    justifyItems: "stretch",
    gap: 18
  },
  lines: {
    display: "grid",
    width: "min(420px, 72vw)",
    gap: 8,
    paddingBlock: 2
  },
  line: {
    display: "block",
    height: 10,
    minWidth: 72,
    borderRadius: 6,
    backgroundColor: "var(--skeleton-glimmer-line)"
  }
});
