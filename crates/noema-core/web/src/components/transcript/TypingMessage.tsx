import { ChatMessage, ChatMessageBubble, type ChatMessageBubbleProps, type ChatMessageProps } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";

const dotAnimation = {
  animationDuration: "1s",
  animationIterationCount: "infinite",
  animationName: "transcript-typing-dot-bounce",
  animationTimingFunction: "ease"
} as const;

type ChatMessageXStyle = ChatMessageProps["xstyle"];
type ChatMessageBubbleXStyle = ChatMessageBubbleProps["xstyle"];

const styles = stylex.create({
  message: {
    width: "calc(100% - 40px)",
    maxWidth: 720,
    minWidth: 0
  },
  bubble: {
    width: 58,
    minHeight: 36,
    backgroundColor: "var(--muted)",
    color: "var(--foreground)"
  },
  content: {
    display: "flex",
    width: "100%",
    minHeight: 36,
    alignItems: "center",
    justifyContent: "center",
    gap: 6
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 999,
    backgroundColor: "color-mix(in srgb, var(--muted-foreground) 70%, transparent)",
    ...dotAnimation
  },
  firstDot: {
    animationDelay: "-0.24s"
  },
  secondDot: {
    animationDelay: "-0.12s"
  }
});

export function TypingMessage({ showAvatar }: { showAvatar: boolean }) {
  return (
    <ChatMessage
      sender="assistant"
      avatar={<TranscriptActorAvatar lane="assistant" visible={showAvatar} />}
      xstyle={chatMessageXStyle(styles.message)}
    >
      <ChatMessageBubble xstyle={chatMessageBubbleXStyle(styles.bubble)}>
        <div {...stylex.props(styles.content)} aria-label="Noema is typing" role="status">
          <span {...stylex.props(styles.dot, styles.firstDot)} />
          <span {...stylex.props(styles.dot, styles.secondDot)} />
          <span {...stylex.props(styles.dot)} />
        </div>
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
