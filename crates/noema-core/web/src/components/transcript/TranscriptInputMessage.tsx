import * as stylex from "@stylexjs/stylex";
import { TextCursorInput } from "lucide-react";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

const styles = stylex.create({
  content: {
    display: "flex",
    minWidth: 0,
    alignItems: "flex-start",
    gap: 8
  },
  icon: {
    flexShrink: 0,
    marginTop: 3,
    color: "var(--noema-text-secondary)"
  },
  text: {
    margin: 0,
    font: "inherit",
    lineHeight: "inherit",
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap",
    wordBreak: "break-word"
  }
});

export function TranscriptInputMessage({ text }: { text: string }) {
  return (
    <TranscriptChatBubble reserveAvatarSpace={false} role="input" showAvatar={false}>
      <div {...stylex.props(styles.content)}>
        <TextCursorInput aria-hidden="true" size={16} {...stylex.props(styles.icon)} />
        <pre {...stylex.props(styles.text)}>{formatInputContent(text)}</pre>
      </div>
    </TranscriptChatBubble>
  );
}

function formatInputContent(text: string): string {
  const value = text.trim();
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}
