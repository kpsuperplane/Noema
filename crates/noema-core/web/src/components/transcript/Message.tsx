import { AnimatedMessageText } from "../MessageTextAnimation";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

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
      <AnimatedMessageText animate={animate} text={text} />
    </TranscriptChatBubble>
  );
}
