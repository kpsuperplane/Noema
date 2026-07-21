import * as stylex from "@stylexjs/stylex";
import { CheckIcon, ChevronDownIcon, ClockIcon, Loader2Icon, WrenchIcon, XIcon } from "lucide-react";
import type { ReactNode } from "react";
import {
  toolMarkerExpandable,
  toolMarkerName,
  toolMarkerPending,
  toolMarkerTarget
} from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";

type ToolMarkerData =
  {
    kind: "tool";
    marker: ToolMarkerGroup;
  }
  | {
      kind: "tool_group";
      markers: ToolMarkerGroup[];
    };

type ToolMarkerCallStatus = "pending" | "running" | "complete" | "error";
type ToolMarkerPresentation = "activity" | "content";

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
  contentName: {
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: 2,
    fontFamily: "var(--noema-font-body)",
    whiteSpace: "normal"
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
  },
  groupIcon: {
    display: "inline-flex",
    width: 16,
    height: 16,
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    color: "var(--noema-text-muted)"
  },
  groupLabel: {
    color: "var(--noema-text-secondary)",
    fontFamily: "var(--noema-font-body)",
    fontWeight: 500
  },
  groupCount: {
    display: "inline-flex",
    alignItems: "center",
    gap: "var(--spacing-1)",
    flexShrink: 0,
    color: "var(--noema-text-faint)"
  },
  groupList: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-0-5)"
  }
});

export function ToolMarker({
  data,
  open,
  onToggle,
  detail,
  renderDetail = true,
  presentation = "activity",
  expandedMarkers,
  onToggleMarker
}: {
  data: ToolMarkerData;
  open: boolean;
  onToggle: () => void;
  detail?: ReactNode;
  renderDetail?: boolean;
  presentation?: ToolMarkerPresentation;
  expandedMarkers?: ReadonlySet<string>;
  onToggleMarker?: (id: string) => void;
}) {
  const calls = toolMarkerCalls(data);
  if (calls.length === 0) {
    return null;
  }

  if (data.kind === "tool_group") {
    const latestCall = calls[calls.length - 1];
    const groupContentId = `${data.markers[0]?.id ?? "tool-group"}-calls`;
    return (
      <div {...stylex.props(styles.root)}>
        <div {...stylex.props(styles.frame)} data-slot="tool-marker-group">
          <button
            type="button"
            {...stylex.props(styles.row, styles.rowButton)}
            aria-controls={groupContentId}
            aria-expanded={open}
            data-slot="tool-marker-group-row"
            onClick={onToggle}
            title={latestCall.status === "error" ? latestCall.errorMessage : undefined}
          >
            {open ? (
              <>
                <span {...stylex.props(styles.groupIcon)} aria-hidden="true">
                  <WrenchIcon size={15} strokeWidth={1.8} />
                </span>
                <span {...stylex.props(styles.groupLabel)}>{calls.length} tool calls</span>
              </>
            ) : (
              <>
                <ToolMarkerRowContent call={latestCall} open={false} presentation={presentation} showChevron={false} />
                <span {...stylex.props(styles.groupCount)}>
                  <WrenchIcon size={14} strokeWidth={1.8} />
                  {calls.length}
                </span>
              </>
            )}
            <span {...stylex.props(styles.chevron, open && styles.chevronOpen)} aria-hidden="true">
              <ChevronDownIcon size={14} strokeWidth={2} />
            </span>
          </button>
          {open ? (
            <div id={groupContentId} {...stylex.props(styles.groupList)}>
              {data.markers.map((marker) => (
                <ToolMarker
                  key={marker.id}
                  data={{ kind: "tool", marker }}
                  expandedMarkers={expandedMarkers}
                  onToggleMarker={onToggleMarker}
                  onToggle={() => onToggleMarker?.(marker.id)}
                  open={expandedMarkers?.has(marker.id) ?? false}
                  presentation={presentation}
                  renderDetail={renderDetail}
                />
              ))}
            </div>
          ) : null}
        </div>
      </div>
    );
  }

  const defaultCall = calls[0];
  const call = detail === undefined
    ? defaultCall
    : { ...defaultCall, expandable: true, resultDetail: detail };

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
            <ToolMarkerRowContent call={call} open={open} presentation={presentation} />
          </button>
        ) : (
          <div
            {...stylex.props(styles.row)}
            data-slot="tool-marker-row"
            title={call.status === "error" ? call.errorMessage : undefined}
          >
            <ToolMarkerRowContent call={call} open={false} presentation={presentation} />
          </div>
        )}
        {renderDetail && call.expandable && open && call.resultDetail ? (
          <div {...stylex.props(styles.detail)}>{call.resultDetail}</div>
        ) : null}
      </div>
    </div>
  );
}

function ToolMarkerRowContent({
  call,
  open,
  presentation,
  showChevron = true
}: {
  call: ToolMarkerCall;
  open: boolean;
  presentation: ToolMarkerPresentation;
  showChevron?: boolean;
}) {
  return (
    <>
      <ToolStatusIcon status={call.status} />
      <span
        {...stylex.props(styles.text, styles.name, presentation === "content" && styles.contentName)}
        data-slot="tool-marker-name"
      >
        {call.name}
      </span>
      {call.target ? (
        <span {...stylex.props(styles.text, styles.target)} data-slot="tool-marker-target">
          {call.target}
        </span>
      ) : null}
      {call.expandable && showChevron ? (
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
  return data.kind === "tool_group"
    ? data.markers.map(activityToolMarkerCall)
    : [activityToolMarkerCall(data.marker)];
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
