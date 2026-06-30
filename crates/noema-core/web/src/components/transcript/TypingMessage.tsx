import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { TranscriptRow } from "./TranscriptRow";

export function TypingMessage({ showAvatar }: { showAvatar: boolean }) {
  return (
    <TranscriptRow lane="assistant" showAvatar={showAvatar}>
      <Bubble variant="muted">
        <BubbleContent
          className="flex min-h-9 w-[58px] items-center justify-center gap-1.5 px-3 py-2"
          aria-label="Noema is typing"
          role="status"
        >
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.24s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.12s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70" />
        </BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}
