import { BrainIcon } from "lucide-react";
import { memoryCardsFromStructuredItem } from "../../memoryCards";
import type { TurnTranscriptItem } from "../../types";
import {
  memoryCardsFromClaimOutcomes,
  memoryMarkerLabel,
  metadataCount
} from "./markerModel";
import { MemoryDetailAttachment } from "./MemoryDetailAttachment";
import { TranscriptMarkerFrame } from "./TranscriptMarkerFrame";

export function MemoryMarker({
  id,
  extraction,
  proposal,
  open,
  onToggle
}: {
  id: string;
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
  proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const memories = proposal ? memoryCardsFromStructuredItem(proposal) ?? [] : memoryCardsFromClaimOutcomes(extraction);
  const failed =
    extraction?.status === "FAILED" &&
    (memories.length === 0 || metadataCount(extraction.metadata, "failed_proposal_count") > 0);
  const started = extraction?.status === "STARTED";
  const label = memoryMarkerLabel(extraction);
  const tone = failed ? "error" : started ? "default" : "success";

  return (
    <div className="grid w-full max-w-full gap-2">
      <TranscriptMarkerFrame
        tone={tone}
        pending={started}
        icon={<BrainIcon />}
        buttonProps={{
          "aria-expanded": open,
          "aria-controls": `${id}-details`,
          onClick: onToggle
        }}
      >
        {label}
      </TranscriptMarkerFrame>
      {open ? (
        <MemoryDetailAttachment id={`${id}-details`} extraction={extraction} memories={memories} failed={failed} />
      ) : null}
    </div>
  );
}
