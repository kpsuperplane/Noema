import { Marker, MarkerContent, MarkerIcon } from "@/components/ui/marker";
import { BrainIcon } from "lucide-react";
import { memoryCardsFromStructuredItem } from "../../memoryCards";
import type { TurnTranscriptItem } from "../../types";
import {
  memoryCardsFromClaimOutcomes,
  memoryMarkerLabel,
  metadataCount
} from "./markerModel";
import { MemoryDetailAttachment } from "./MemoryDetailAttachment";

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
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${id}-details`}
        onClick={onToggle}
        tone={tone}
        pending={started}
        className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
      >
        <MarkerIcon>
          <BrainIcon />
        </MarkerIcon>
        <MarkerContent>{label}</MarkerContent>
      </Marker>
      {open ? (
        <MemoryDetailAttachment id={`${id}-details`} extraction={extraction} memories={memories} failed={failed} />
      ) : null}
    </div>
  );
}
