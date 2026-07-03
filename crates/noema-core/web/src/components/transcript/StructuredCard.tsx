import { memoryCardsFromStructuredItem } from "@/memory/cards";
import type { TurnTranscriptItem } from "@/shared/types";
import { MemoryStructuredCard } from "./MemoryStructuredCard";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
import { ToolMarker } from "./ToolMarker";

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
    return <ToolMarker data={{ kind: "memory", id: item.id, proposal: item }} open={open} onToggle={onToggle} />;
  }

  const memories = memoryCardsFromStructuredItem(item);
  if (memories) {
    return <MemoryStructuredCard schema={item.schema} memories={memories} />;
  }

  return <TranscriptAttachmentCard title="Noema recorded an update" description="Details are available in inspection." />;
}
