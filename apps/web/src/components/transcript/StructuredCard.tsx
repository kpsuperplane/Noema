import type { TurnTranscriptItem } from "@/shared/types";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";

export function StructuredCard({
  item
}: {
  item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
  open: boolean;
  onToggle: () => void;
}) {
  return <TranscriptAttachmentCard title="Noema recorded an update" description={`Schema: ${item.schema}`} />;
}
