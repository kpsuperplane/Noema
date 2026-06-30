import {
  Attachment,
  AttachmentContent,
  AttachmentDescription,
  AttachmentTitle
} from "@/components/ui/attachment";
import { memoryCardsFromStructuredItem } from "../../memoryCards";
import type { TurnTranscriptItem } from "../../types";
import { MemoryMarker } from "./MemoryMarker";
import { MemoryStructuredCard } from "./MemoryStructuredCard";

export function StructuredCard({
  item,
  open,
  onToggle
}: {
  item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
  open: boolean;
  onToggle: () => void;
}) {
  if (item.schema === "memory_proposals") {
    return <MemoryMarker id={item.id} proposal={item} open={open} onToggle={onToggle} />;
  }

  const memories = memoryCardsFromStructuredItem(item);
  if (memories) {
    return <MemoryStructuredCard schema={item.schema} memories={memories} />;
  }

  return (
    <Attachment className="max-w-full">
      <AttachmentContent>
        <AttachmentTitle>{item.schema}</AttachmentTitle>
        <AttachmentDescription>Structured card placeholder</AttachmentDescription>
      </AttachmentContent>
    </Attachment>
  );
}
