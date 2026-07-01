import * as stylex from "@stylexjs/stylex";
import { AnimatedMessageText } from "../MessageTextAnimation";
import { TranscriptRow } from "./TranscriptRow";

const styles = stylex.create({
  bubble: {
    display: "flex",
    width: "fit-content",
    maxWidth: "80%",
    minWidth: 0,
    flexDirection: "column",
    gap: 4
  },
  assistantBubble: {
    maxWidth: "100%"
  },
  content: {
    width: "fit-content",
    maxWidth: "100%",
    minWidth: 0,
    overflow: "hidden",
    borderRadius: 24,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "transparent",
    paddingBlock: 4,
    paddingInline: 12,
    fontSize: 14,
    lineHeight: 1.7,
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap"
  },
  userContent: {
    backgroundColor: "var(--primary)",
    color: "var(--primary-foreground)"
  },
  assistantContent: {
    backgroundColor: "var(--muted)",
    color: "var(--foreground)"
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
    <TranscriptRow lane={role === "user" ? "human" : "assistant"} showAvatar={showAvatar}>
      <div {...stylex.props(styles.bubble, role === "assistant" && styles.assistantBubble)}>
        <div {...stylex.props(styles.content, role === "user" ? styles.userContent : styles.assistantContent)}>
          <AnimatedMessageText animate={animate} text={text} />
        </div>
      </div>
    </TranscriptRow>
  );
}
