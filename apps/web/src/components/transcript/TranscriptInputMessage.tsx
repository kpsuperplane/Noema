import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { ExpandableTextBubbleContent } from "./ExpandableTextBubble";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

const styles = stylex.create({
  text: {
    margin: 0,
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    lineHeight: "inherit",
    overflowWrap: "anywhere",
    tabSize: 2,
    whiteSpace: "pre-wrap",
    wordBreak: "break-word"
  }
});

export function TranscriptInputMessage({ text }: { text: string }) {
  const [expanded, setExpanded] = React.useState(false);
  const [overflowing, setOverflowing] = React.useState(false);
  const content = React.useMemo(() => formatInputContent(text), [text]);

  return (
    <TranscriptChatBubble
      interactive={overflowing && !expanded}
      reserveAvatarSpace={false}
      role="input"
      showAvatar={false}
    >
      <ExpandableTextBubbleContent onExpandedChange={setExpanded} onOverflowChange={setOverflowing}>
        <pre {...stylex.props(styles.text)}>{content}</pre>
      </ExpandableTextBubbleContent>
    </TranscriptChatBubble>
  );
}

function formatInputContent(text: string): string {
  const value = text.trim();
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value
      .split("\n")
      .map(prettyPrintJsonLine)
      .join("\n");
  }
}

function prettyPrintJsonLine(line: string): string {
  const value = line.trim();
  if (!(value.startsWith("{") || value.startsWith("["))) {
    return line;
  }
  try {
    const parsed: unknown = JSON.parse(value);
    if (typeof parsed !== "object" || parsed === null) {
      return line;
    }
    const indentation = line.slice(0, line.length - line.trimStart().length);
    return JSON.stringify(parsed, null, 2)
      .split("\n")
      .map((jsonLine) => `${indentation}${jsonLine}`)
      .join("\n");
  } catch {
    return line;
  }
}
