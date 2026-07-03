import { BrainIcon } from "lucide-react";
import { statusLabel } from "@/shared/format";
import type { MemoryCardData } from "@/memory/cards";
import type { TurnTranscriptItem } from "@/shared/types";
import { memoryMarkerLabel, metadataCount } from "./markerModel";
import { MemoryDetailList } from "./MemoryDetailList";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";

export function MemoryDetailAttachment({
  id,
  extraction,
  memories,
  failed
}: {
  id: string;
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
  memories: MemoryCardData[];
  failed: boolean;
}) {
  const memoryCount = memories.length;
  const title = failed && memoryCount === 0 ? "Memory update failed" : memoryDetailTitle(extraction, memoryCount);
  const status = extraction ? statusLabel(extraction.status) : null;
  const description = extraction?.summary ?? (status ? `Memory extraction ${status.toLowerCase()}` : "Memory proposal");

  return (
    <TranscriptAttachmentCard
      id={id}
      title={title}
      description={description}
      icon={<BrainIcon />}
      tone={failed ? "error" : "success"}
    >
      <MemoryDetailList memories={memories} />
    </TranscriptAttachmentCard>
  );
}

function memoryDetailTitle(
  extraction: Extract<TurnTranscriptItem, { kind: "activity" }> | undefined,
  memoryCount: number
): string {
  if (extraction && memoryCount > 0 && metadataCount(extraction.metadata, "failed_proposal_count") > 0) {
    return memoryMarkerLabel(extraction);
  }
  return memoryCount === 1 ? "Memory saved" : `${memoryCount} memories saved`;
}
