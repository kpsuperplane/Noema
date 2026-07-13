import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { ExpandableTextBubbleContent, TextBubbleDialog } from "./ExpandableTextBubble";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

const styles = stylex.create({
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
  const [fullMessageOpen, setFullMessageOpen] = React.useState(false);
  const [overflowing, setOverflowing] = React.useState(false);
  const content = React.useMemo(() => formatInputContent(text), [text]);

  return (
    <>
      <TranscriptChatBubble interactive={overflowing} reserveAvatarSpace={false} role="input" showAvatar={false}>
        <ExpandableTextBubbleContent onOpen={() => setFullMessageOpen(true)} onOverflowChange={setOverflowing}>
          <pre {...stylex.props(styles.text)}>{content}</pre>
        </ExpandableTextBubbleContent>
      </TranscriptChatBubble>
      <TextBubbleDialog open={fullMessageOpen} title="System input" onOpenChange={setFullMessageOpen}>
        <pre {...stylex.props(styles.text)}>{content}</pre>
      </TextBubbleDialog>
    </>
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
