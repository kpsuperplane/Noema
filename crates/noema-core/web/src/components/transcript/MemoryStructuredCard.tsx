import { BrainIcon } from "lucide-react";
import type { MemoryCardData } from "@/memory/cards";
import { MemoryDetailList } from "./MemoryDetailList";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";

export function MemoryStructuredCard({ schema, memories }: { schema: string; memories: MemoryCardData[] }) {
  const count = memories.length;
  const title = count === 1 ? "Memory saved" : `${count} memories saved`;
  const source = schema === "memory_proposals" ? "Same-call proposal" : "Explicit request";

  return (
    <TranscriptAttachmentCard title={title} description={source} icon={<BrainIcon />} tone="success">
      <MemoryDetailList memories={memories} />
    </TranscriptAttachmentCard>
  );
}
