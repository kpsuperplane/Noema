import {
  Attachment,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle
} from "@/components/ui/attachment";
import { BrainIcon } from "lucide-react";
import { statusLabel } from "../../format";
import type { MemoryCardData } from "../../memoryCards";
import type { TurnTranscriptItem } from "../../types";
import { memoryMarkerLabel, metadataCount } from "./markerModel";
import { MemoryDetailList } from "./MemoryDetailList";

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
    <Attachment id={id} state={failed ? "error" : "done"} className="max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{description}</AttachmentDescription>
        <MemoryDetailList memories={memories} />
      </AttachmentContent>
    </Attachment>
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
