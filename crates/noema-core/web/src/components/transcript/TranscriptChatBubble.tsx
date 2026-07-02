import { ChatMessage, ChatMessageBubble, type ChatMessageBubbleProps, type ChatMessageProps } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";

type TranscriptChatBubbleRole = "user" | "assistant";
type TranscriptChatBubbleVariant = "message" | "typing";

type ChatMessageXStyle = ChatMessageProps["xstyle"];
type ChatMessageBubbleXStyle = ChatMessageBubbleProps["xstyle"];

const styles = stylex.create({
  message: {
    width: "calc(100% - 40px)",
    maxWidth: 720,
    minWidth: 0,
    alignItems: "flex-end"
  },
  bubble: {
    minWidth: 0,
    overflow: "hidden",
    backgroundColor: "var(--muted)",
    color: "var(--foreground)"
  },
  textBubble: {
    maxWidth: "80%",
    fontSize: 14,
    lineHeight: 1.7,
    overflowWrap: "break-word",
    paddingBlock: "var(--spacing-2)"
  },
  userBubble: {
    backgroundColor: "var(--primary)",
    color: "var(--primary-foreground)"
  },
  assistantBubble: {
    maxWidth: "100%"
  },
  typingBubble: {
    width: 58,
    minHeight: 40,
    paddingBlock: 0,
    paddingInline: 0
  }
});

export function TranscriptChatBubble({
  children,
  role,
  showAvatar,
  variant = "message"
}: {
  children: React.ReactNode;
  role: TranscriptChatBubbleRole;
  showAvatar: boolean;
  variant?: TranscriptChatBubbleVariant;
}) {
  const lane = role === "user" ? "human" : "assistant";
  const sender = role === "user" ? "user" : "assistant";

  return (
    <ChatMessage
      sender={sender}
      avatar={<TranscriptActorAvatar lane={lane} visible={showAvatar} />}
      xstyle={chatMessageXStyle(styles.message)}
    >
      <ChatMessageBubble
        xstyle={chatMessageBubbleXStyle(
          styles.bubble,
          variant === "message" && styles.textBubble,
          variant === "message" && role === "user" && styles.userBubble,
          variant === "message" && role === "assistant" && styles.assistantBubble,
          variant === "typing" && styles.typingBubble
        )}
      >
        {children}
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
