import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

type MarkdownXStyle = MarkdownProps["xstyle"];

const styles = stylex.create({
  markdown: {
    color: "inherit",
    fontFamily: "inherit",
    fontSize: "inherit",
    lineHeight: "inherit"
  }
});

export function Message({
  animate,
  role,
  text,
  showAvatar
}: {
  animate: boolean;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
}) {
  return (
    <TranscriptChatBubble role={role} showAvatar={showAvatar}>
      <Markdown
        autolink="gfm"
        contentWidth="100%"
        density="compact"
        headingLevelStart={3}
        isStreaming={animate}
        xstyle={markdownXStyle(styles.markdown)}
      >
        {text}
      </Markdown>
    </TranscriptChatBubble>
  );
}

function markdownXStyle(xstyle: unknown): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}
