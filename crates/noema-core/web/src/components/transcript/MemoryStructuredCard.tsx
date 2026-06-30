import {
  Attachment,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle
} from "@/components/ui/attachment";
import { BrainIcon } from "lucide-react";
import type { MemoryCardData } from "../../memoryCards";
import { MemoryDetailList } from "./MemoryDetailList";

export function MemoryStructuredCard({ schema, memories }: { schema: string; memories: MemoryCardData[] }) {
  const count = memories.length;
  const title = count === 1 ? "Memory saved" : `${count} memories saved`;
  const source = schema === "memory_proposals" ? "Same-call proposal" : "Explicit request";

  return (
    <Attachment className="max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{source}</AttachmentDescription>
        <MemoryDetailList memories={memories} />
      </AttachmentContent>
    </Attachment>
  );
}
