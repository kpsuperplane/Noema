import { ChatMessage, ChatMessageBubble, type ChatMessageBubbleProps, type ChatMessageProps } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { AnimatedMessageText } from "../MessageTextAnimation";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";

type ChatMessageXStyle = ChatMessageProps["xstyle"];
type ChatMessageBubbleXStyle = ChatMessageBubbleProps["xstyle"];

const styles = stylex.create({
  message: {
    width: "100%",
    maxWidth: 760,
    minWidth: 0
  },
  bubble: {
    maxWidth: "80%",
    minWidth: 0,
    overflow: "hidden",
    fontSize: 14,
    lineHeight: 1.7,
    overflowWrap: "break-word"
  },
  assistantBubble: {
    maxWidth: "100%"
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
  const lane = role === "user" ? "human" : "assistant";
  const sender = role === "user" ? "user" : "assistant";

  return (
    <ChatMessage
      sender={sender}
      avatar={<TranscriptActorAvatar lane={lane} visible={showAvatar} />}
      xstyle={chatMessageXStyle(styles.message)}
    >
      <ChatMessageBubble xstyle={chatMessageBubbleXStyle(styles.bubble, role === "assistant" && styles.assistantBubble)}>
        <AnimatedMessageText animate={animate} text={text} />
      </ChatMessageBubble>
    </ChatMessage>
  );
}

function chatMessageXStyle(...xstyle: unknown[]): ChatMessageXStyle {
  return xstyle as unknown as ChatMessageXStyle;
}

function chatMessageBubbleXStyle(...xstyle: unknown[]): ChatMessageBubbleXStyle {
  return xstyle as unknown as ChatMessageBubbleXStyle;
}
