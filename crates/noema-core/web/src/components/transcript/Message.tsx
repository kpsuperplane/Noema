import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { AnimatedMessageText } from "../MessageTextAnimation";
import { TranscriptRow } from "./TranscriptRow";

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
      <Bubble variant={role === "user" ? "default" : "muted"}>
        <BubbleContent className="leading-[1.7] whitespace-pre-wrap">
          <AnimatedMessageText animate={animate} text={text} />
        </BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}
