import * as stylex from "@stylexjs/stylex";
import { CheckIcon, ChevronDownIcon, ClockIcon, Loader2Icon, XIcon } from "lucide-react";
import type { ReactNode } from "react";
import { memoryCardsFromStructuredItem } from "@/memory/cards";
import type { TurnTranscriptItem } from "@/shared/types";
import { MemoryDetailAttachment } from "./MemoryDetailAttachment";
import {
  memoryCardsFromClaimOutcomes,
  memoryMarkerLabel,
  metadataCount,
  toolMarkerExpandable,
  toolMarkerName,
  toolMarkerPending,
  toolMarkerTarget
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

type ToolMarkerCallStatus = "pending" | "running" | "complete" | "error";

type ToolMarkerCall = {
  key: string;
  name: string;
  status: ToolMarkerCallStatus;
  expandable: boolean;
  target?: string;
  errorMessage?: string;
  resultDetail?: ReactNode;
};

const styles = stylex.create({
  root: {
    display: "grid",
    width: "100%",
    maxWidth: "100%",
    minWidth: 0,
    gap: 8,
    justifyItems: "start"
  },
  frame: {
    display: "grid",
    width: "fit-content",
    maxWidth: "100%",
    minWidth: 0,
    gap: 8
  },
  row: {
    display: "inline-flex",
    width: "100%",
    maxWidth: "100%",
    minWidth: 0,
    alignItems: "center",
    appearance: "none",
    gap: 6,
    borderWidth: 0,
    backgroundColor: "transparent",
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    lineHeight: "20px",
    paddingBlock: 2,
    paddingInline: 0,
    textAlign: "left",
    transitionDuration: "120ms",
    transitionProperty: "opacity"
  },
  rowButton: {
    cursor: "pointer",
    ":hover": {
      opacity: 0.72
    }
  },
  statusIcon: {
    display: "inline-flex",
    width: 16,
    height: 16,
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center"
  },
  pending: {
    color: "var(--noema-text-faint)"
  },
  running: {
    color: "var(--noema-pine-600)"
  },
  runningSpinner: {
    animationDuration: "900ms",
    animationIterationCount: "infinite",
    animationName: "tool-marker-spinner-rotate",
    animationTimingFunction: "linear",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none"
    }
  },
  complete: {
    color: "var(--noema-pine-600)"
  },
  error: {
    color: "var(--noema-red-700)"
  },
  text: {
    display: "block",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap"
  },
  name: {
    flexShrink: 1,
    color: "var(--noema-text-secondary)"
  },
  target: {
    flexShrink: 10,
    color: "var(--noema-text-faint)"
  },
  chevron: {
    display: "inline-flex",
    width: 14,
    height: 14,
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    color: "var(--noema-text-faint)",
    transitionDuration: "160ms",
    transitionProperty: "transform"
  },
  chevronOpen: {
    transform: "rotate(180deg)"
  },
  detail: {
    minWidth: 0,
    marginLeft: 22
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
  const call = calls[0];

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.frame)} data-slot="tool-marker">
        {call.expandable ? (
          <button
            type="button"
            {...stylex.props(styles.row, styles.rowButton)}
            aria-expanded={open}
            data-slot="tool-marker-row"
            onClick={onToggle}
            title={call.status === "error" ? call.errorMessage : undefined}
          >
            <ToolMarkerRowContent call={call} open={open} />
          </button>
        ) : (
          <div
            {...stylex.props(styles.row)}
            data-slot="tool-marker-row"
            title={call.status === "error" ? call.errorMessage : undefined}
          >
            <ToolMarkerRowContent call={call} open={false} />
          </div>
        )}
        {call.expandable && open && call.resultDetail ? (
          <div {...stylex.props(styles.detail)}>{call.resultDetail}</div>
        ) : null}
      </div>
    </div>
  );
}

function ToolMarkerRowContent({ call, open }: { call: ToolMarkerCall; open: boolean }) {
  return (
    <>
      <ToolStatusIcon status={call.status} />
      <span {...stylex.props(styles.text, styles.name)} data-slot="tool-marker-name">
        {call.name}
      </span>
      {call.target ? (
        <span {...stylex.props(styles.text, styles.target)} data-slot="tool-marker-target">
          {call.target}
        </span>
      ) : null}
      {call.expandable ? (
        <span {...stylex.props(styles.chevron, open && styles.chevronOpen)} aria-hidden="true">
          <ChevronDownIcon size={14} strokeWidth={2} />
        </span>
      ) : null}
    </>
  );
}

function ToolStatusIcon({ status }: { status: ToolMarkerCallStatus }) {
  if (status === "running") {
    return (
      <span {...stylex.props(styles.statusIcon, styles.running, styles.runningSpinner)}>
        <Loader2Icon aria-hidden="true" size={14} strokeWidth={2} />
      </span>
    );
  }
  if (status === "pending") {
    return (
      <span {...stylex.props(styles.statusIcon, styles.pending)}>
        <ClockIcon aria-hidden="true" size={14} strokeWidth={2} />
      </span>
    );
  }
  if (status === "error") {
    return (
      <span {...stylex.props(styles.statusIcon, styles.error)}>
        <XIcon aria-hidden="true" size={14} strokeWidth={2} />
      </span>
    );
  }
  return (
    <span {...stylex.props(styles.statusIcon, styles.complete)}>
      <CheckIcon aria-hidden="true" size={14} strokeWidth={2} />
    </span>
  );
}

function toolMarkerCalls(data: ToolMarkerData): ToolMarkerCall[] {
  if (data.kind === "memory") {
    return [memoryToolMarkerCall(data)];
  }

  return [activityToolMarkerCall(data.marker)];
}

function activityToolMarkerCall(marker: ToolMarkerGroup): ToolMarkerCall {
  const target = toolMarkerTarget(marker);
  const expandable = toolMarkerExpandable(marker);
  const errorMessage =
    marker.result?.item.status === "FAILED" ? marker.result.item.summary ?? marker.result.item.title : undefined;
  const call: ToolMarkerCall = {
    key: marker.id,
    name: toolMarkerName(marker),
    status: toolMarkerStatus(marker),
    expandable
  };

  if (expandable) {
    call.resultDetail = <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} />;
  }

  if (target) {
    call.target = target;
  }
  if (errorMessage) {
    call.errorMessage = errorMessage;
  }

  return call;
}

function memoryToolMarkerCall(marker: Extract<ToolMarkerData, { kind: "memory" }>): ToolMarkerCall {
  const memories = marker.proposal
    ? memoryCardsFromStructuredItem(marker.proposal) ?? []
    : memoryCardsFromClaimOutcomes(marker.extraction);
  const failed =
    marker.extraction?.status === "FAILED" &&
    (memories.length === 0 || metadataCount(marker.extraction.metadata, "failed_proposal_count") > 0);
  const label = memoryMarkerLabel(marker.extraction);
  const target = marker.extraction?.summary;
  const call: ToolMarkerCall = {
    key: marker.id,
    name: label,
    status: memoryToolMarkerStatus(marker, failed),
    expandable: true,
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

function toolMarkerStatus(marker: ToolMarkerGroup): ToolMarkerCallStatus {
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
): ToolMarkerCallStatus {
  if (failed) {
    return "error";
  }
  if (marker.extraction?.status === "STARTED") {
    return "running";
  }
  return "complete";
}
