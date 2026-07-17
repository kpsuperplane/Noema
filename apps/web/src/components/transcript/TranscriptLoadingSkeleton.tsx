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
                    data-slot="skeleton-glimmer"
                    {...stylex.props(styles.line)}
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
      paddingInline: 20
    }
  },
  stack: {
    display: "grid",
    width: "var(--chat-column-width)",
    maxWidth: "100%",
    minWidth: 0,
    gap: 18,
    marginInline: "auto",
    paddingInline: 2
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
