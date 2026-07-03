import { ChatToolCalls, type ChatToolCallItem, type ChatToolCallStatus } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { memoryCardsFromStructuredItem } from "@/memory/cards";
import type { TurnTranscriptItem } from "@/shared/types";
import { MemoryDetailAttachment } from "./MemoryDetailAttachment";
import {
  memoryCardsFromClaimOutcomes,
  memoryMarkerLabel,
  metadataCount,
  toolMarkerName,
  toolMarkerPending
} from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";

type ToolMarkerData =
  | {
      kind: "tool";
      marker: ToolMarkerGroup;
    }
  | {
      kind: "memory";
      id: string;
      extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
      proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
    };

const styles = stylex.create({
  root: {
    display: "grid",
    width: "100%",
    gap: 8,
    justifyItems: "start"
  }
});

export function ToolMarker({
  data,
  open,
  onToggle
}: {
  data: ToolMarkerData;
  open: boolean;
  onToggle: () => void;
}) {
  const calls = toolMarkerCalls(data);

  return (
    <div {...stylex.props(styles.root)}>
      <ChatToolCalls
        calls={calls}
        isExpanded={open}
        onExpandedChange={(nextOpen) => {
          if (nextOpen !== open) {
            onToggle();
          }
        }}
      />
    </div>
  );
}

function toolMarkerCalls(data: ToolMarkerData): ChatToolCallItem[] {
  if (data.kind === "memory") {
    return [memoryToolMarkerCall(data)];
  }

  return [activityToolMarkerCall(data.marker)];
}

function activityToolMarkerCall(marker: ToolMarkerGroup): ChatToolCallItem {
  const target = marker.result?.item.summary ?? marker.call?.item.summary;
  const errorMessage =
    marker.result?.item.status === "FAILED" ? marker.result.item.summary ?? marker.result.item.title : undefined;
  const call: ChatToolCallItem = {
    key: marker.id,
    name: toolMarkerName(marker),
    status: toolMarkerStatus(marker),
    resultDetail: <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} />
  };

  if (target) {
    call.target = target;
  }
  if (errorMessage) {
    call.errorMessage = errorMessage;
  }

  return call;
}

function memoryToolMarkerCall(marker: Extract<ToolMarkerData, { kind: "memory" }>): ChatToolCallItem {
  const memories = marker.proposal
    ? memoryCardsFromStructuredItem(marker.proposal) ?? []
    : memoryCardsFromClaimOutcomes(marker.extraction);
  const failed =
    marker.extraction?.status === "FAILED" &&
    (memories.length === 0 || metadataCount(marker.extraction.metadata, "failed_proposal_count") > 0);
  const label = memoryMarkerLabel(marker.extraction);
  const target = marker.extraction?.summary;
  const call: ChatToolCallItem = {
    key: marker.id,
    name: label,
    status: memoryToolMarkerStatus(marker, failed),
    resultDetail: (
      <MemoryDetailAttachment
        id={`${marker.id}-details`}
        extraction={marker.extraction}
        memories={memories}
        failed={failed}
      />
    )
  };

  if (target) {
    call.target = target;
  }
  if (failed) {
    call.errorMessage = marker.extraction?.summary ?? label;
  }

  return call;
}

function toolMarkerStatus(marker: ToolMarkerGroup): ChatToolCallStatus {
  if (marker.result?.item.status === "FAILED") {
    return "error";
  }
  if (marker.result) {
    return "complete";
  }
  if (toolMarkerPending(marker)) {
    return "running";
  }
  return "pending";
}

function memoryToolMarkerStatus(
  marker: Extract<ToolMarkerData, { kind: "memory" }>,
  failed: boolean
): ChatToolCallStatus {
  if (failed) {
    return "error";
  }
  if (marker.extraction?.status === "STARTED") {
    return "running";
  }
  return "complete";
}
