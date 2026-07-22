import { ChatMessage, ChatMessageBubble, type ChatMessageBubbleProps, type ChatMessageProps } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";
import type { IdentityAvatarActivity } from "../IdentityAvatar";
import type { ChatBubbleGroup } from "./renderModel";

type TranscriptChatBubbleRole = "user" | "assistant" | "input";
type TranscriptChatBubbleVariant = "message" | "typing";

type ChatMessageXStyle = ChatMessageProps["xstyle"];
type ChatMessageBubbleXStyle = ChatMessageBubbleProps["xstyle"];

const styles = stylex.create({
  message: {
    width: "calc(100% - var(--chat-opposite-avatar-gutter, 40px))",
    maxWidth: 720,
    minWidth: 0,
    alignItems: "flex-end",
    "@container chat-transcript (width < 600px)": {
      width: "100%",
      gap: 0
    }
  },
  messageWithoutAvatar: {
    width: "100%"
  },
  bubble: {
    minWidth: 0,
    overflow: "hidden",
    backgroundColor: "var(--muted)",
    color: "var(--foreground)"
  },
  textBubble: {
    maxWidth: "80%",
    minHeight: 40,
    fontSize: 14,
    lineHeight: 1.7,
    overflowWrap: "break-word",
    paddingBlock: "var(--spacing-2)"
  },
  userBubble: {
    backgroundColor: "var(--primary)",
    color: "var(--primary-foreground)"
  },
  inputBubble: {
    backgroundColor: "color-mix(in srgb, var(--noema-text-muted) 12%, var(--noema-surface-card))",
    color: "var(--noema-text-primary)"
  },
  interactiveBubble: {
    cursor: "pointer",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "box-shadow, filter",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    ":hover": {
      filter: "brightness(0.94)"
    },
    ":focus-within": {
      boxShadow: "inset 0 0 0 2px var(--color-accent)"
    }
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
  avatarActivity = "idle",
  avatarAnimated = false,
  children,
  group,
  role,
  showAvatar,
  reserveAvatarSpace = true,
  variant = "message",
  interactive = false
}: {
  avatarActivity?: IdentityAvatarActivity;
  avatarAnimated?: boolean;
  children: React.ReactNode;
  group?: ChatBubbleGroup;
  role: TranscriptChatBubbleRole;
  showAvatar: boolean;
  reserveAvatarSpace?: boolean;
  variant?: TranscriptChatBubbleVariant;
  interactive?: boolean;
}) {
  const lane = role === "assistant" ? "assistant" : "human";
  const sender = role === "assistant" ? "assistant" : "user";

  return (
    <ChatMessage
      sender={sender}
      avatar={reserveAvatarSpace ? (
        <TranscriptActorAvatar
          activity={avatarActivity}
          animated={avatarAnimated}
          lane={lane}
          visible={showAvatar}
        />
      ) : undefined}
      xstyle={chatMessageXStyle(styles.message, !reserveAvatarSpace && styles.messageWithoutAvatar)}
    >
      <ChatMessageBubble
        group={variant === "message" ? group : undefined}
        xstyle={chatMessageBubbleXStyle(
          styles.bubble,
          variant === "message" && styles.textBubble,
          variant === "message" && role === "user" && styles.userBubble,
          variant === "message" && role === "input" && styles.inputBubble,
          variant === "message" && role === "assistant" && styles.assistantBubble,
          interactive && styles.interactiveBubble,
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
