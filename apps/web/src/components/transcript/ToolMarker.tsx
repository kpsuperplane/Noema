import * as stylex from "@stylexjs/stylex";
import { CheckIcon, ChevronDownIcon, ClockIcon, Loader2Icon, WrenchIcon, XIcon } from "lucide-react";
import { type ReactNode, useState } from "react";
import { RollingText } from "@/components/RollingText";
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
  groupFrame: {
    gap: 0
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
    transitionProperty: "transform",
    "@media (prefers-reduced-motion: reduce)": {
      transitionDuration: "0ms"
    }
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
  groupList: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-0-5)",
    paddingTop: "var(--spacing-2)"
  },
  groupContent: {
    display: "grid",
    gridTemplateRows: "0fr",
    transitionDuration: "180ms",
    transitionProperty: "grid-template-rows",
    transitionTimingFunction: "ease-out",
    "@media (prefers-reduced-motion: reduce)": {
      transitionDuration: "0ms"
    }
  },
  groupContentOpen: {
    gridTemplateRows: "1fr"
  },
  groupContentInner: {
    minHeight: 0,
    overflow: "hidden"
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
  const [lastCollapsedCallKey, setLastCollapsedCallKey] = useState<string | null>(
    () => data.kind === "tool_group" ? calls[calls.length - 1]?.key ?? null : null
  );
  const activeCall = latestActiveCall(calls);
  const retainedCall = calls.find((call) => call.key === lastCollapsedCallKey);
  const collapsedCall = activeCall ?? retainedCall ?? calls[calls.length - 1];

  if (data.kind === "tool_group" && !open && collapsedCall && collapsedCall.key !== lastCollapsedCallKey) {
    setLastCollapsedCallKey(collapsedCall.key);
  }

  if (calls.length === 0) {
    return null;
  }

  if (data.kind === "tool_group") {
    const groupContentId = `${data.markers[0]?.id ?? "tool-group"}-calls`;
    return (
      <div {...stylex.props(styles.root)}>
        <div {...stylex.props(styles.frame, styles.groupFrame)} data-slot="tool-marker-group">
          <button
            type="button"
            {...stylex.props(styles.row, styles.rowButton)}
            aria-controls={groupContentId}
            aria-expanded={open}
            data-slot="tool-marker-group-row"
            onClick={onToggle}
            title={collapsedCall.status === "error" ? collapsedCall.errorMessage : undefined}
          >
            {open ? (
              <>
                <span {...stylex.props(styles.groupIcon)} aria-hidden="true">
                  <WrenchIcon size={15} strokeWidth={1.8} />
                </span>
                <span {...stylex.props(styles.groupLabel)}>{calls.length} tool calls</span>
              </>
            ) : (
              <ToolMarkerRowContent
                animateText
                call={collapsedCall}
                open={false}
                presentation={presentation}
                showChevron={false}
              />
            )}
            <span {...stylex.props(styles.chevron, open && styles.chevronOpen)} aria-hidden="true">
              <ChevronDownIcon size={14} strokeWidth={2} />
            </span>
          </button>
          <div
            id={groupContentId}
            aria-hidden={!open}
            inert={!open ? true : undefined}
            {...stylex.props(styles.groupContent, open && styles.groupContentOpen)}
          >
            <div {...stylex.props(styles.groupContentInner)}>
              <div {...stylex.props(styles.groupList)}>
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
            </div>
          </div>
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
  animateText = false,
  open,
  presentation,
  showChevron = true
}: {
  call: ToolMarkerCall;
  animateText?: boolean;
  open: boolean;
  presentation: ToolMarkerPresentation;
  showChevron?: boolean;
}) {
  return (
    <>
      <ToolStatusIcon status={call.status} />
      {animateText ? (
        <RollingText value={call.name} {...stylex.props(styles.name)} data-slot="tool-marker-name" />
      ) : (
        <span
          {...stylex.props(styles.text, styles.name, presentation === "content" && styles.contentName)}
          data-slot="tool-marker-name"
        >
          {call.name}
        </span>
      )}
      {animateText ? (
        <RollingText value={call.target ?? ""} {...stylex.props(styles.target)} data-slot="tool-marker-target" />
      ) : call.target ? (
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

function latestActiveCall(calls: ToolMarkerCall[]): ToolMarkerCall | undefined {
  for (let index = calls.length - 1; index >= 0; index -= 1) {
    const call = calls[index];
    if (call && (call.status === "pending" || call.status === "running")) {
      return call;
    }
  }
  return undefined;
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
