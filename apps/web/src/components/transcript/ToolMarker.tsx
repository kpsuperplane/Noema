import * as stylex from "@stylexjs/stylex";
import { CheckIcon, ChevronDownIcon, ClockIcon, Globe2Icon, Loader2Icon, SearchIcon, WrenchIcon, XIcon } from "lucide-react";
import { type ReactNode, useState } from "react";
import { RollingText } from "@/components/RollingText";
import { SpringDisclosure } from "@/motion/SpringDisclosure";
import {
  toolMarkerExpandable,
  toolMarkerKind,
  toolMarkerName,
  toolMarkerPending
} from "./markerModel";
import type { ToolMarkerKind } from "./markerModel";
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
  toolKind?: ToolMarkerKind;
  status: ToolMarkerCallStatus;
  expandable: boolean;
  errorMessage?: string;
  resultDetail?: ReactNode;
};

const rotate = stylex.keyframes({
  to: { transform: "rotate(360deg)" }
});

const styles = stylex.create({
  root: {
    display: "grid",
    width: "100%",
    maxWidth: "100%",
    minWidth: 0,
    gap: "var(--spacing-2)",
    justifyItems: "start"
  },
  frame: {
    display: "grid",
    width: "fit-content",
    maxWidth: "100%",
    minWidth: 0,
    gap: "var(--spacing-2)"
  },
  groupFrame: {
    gap: "var(--spacing-0)"
  },
  row: {
    display: "inline-flex",
    width: "100%",
    maxWidth: "100%",
    minWidth: 0,
    alignItems: "center",
    appearance: "none",
    gap: "var(--spacing-1-5)",
    borderWidth: 0,
    backgroundColor: "transparent",
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    lineHeight: "20px",
    paddingBlock: "var(--spacing-0-5)",
    paddingInline: "var(--spacing-0)",
    textAlign: "left",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "opacity",
    transitionTimingFunction: "var(--motion-spring-critical-easing)"
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
  toolIcon: {
    display: "inline-flex",
    width: 16,
    height: 16,
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    color: "var(--noema-text-muted)"
  },
  pending: {
    color: "var(--noema-text-muted)"
  },
  running: {
    color: "var(--noema-pine-600)"
  },
  runningSpinner: {
    animationDuration: "900ms",
    animationIterationCount: "infinite",
    animationName: rotate,
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
  chevron: {
    display: "inline-flex",
    width: 14,
    height: 14,
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    color: "var(--noema-text-muted)",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "transform",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    "@media (prefers-reduced-motion: reduce)": {
      transitionDuration: "0ms"
    }
  },
  chevronOpen: {
    transform: "rotate(180deg)"
  },
  detail: {
    minWidth: 0,
    marginLeft: "calc(var(--spacing-5) + var(--spacing-0-5))"
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
    minWidth: 0
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
  interactive = true,
  presentation = "activity",
  expandedMarkers,
  onToggleMarker
}: {
  data: ToolMarkerData;
  open: boolean;
  onToggle: () => void;
  detail?: ReactNode;
  renderDetail?: boolean;
  interactive?: boolean;
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
    const singleCall = calls.length === 1;
    const expandable = interactive && (!singleCall || collapsedCall.expandable);
    const groupContentId = `${data.markers[0]?.id ?? "tool-group"}-calls`;
    const rowContent = (
      <>
        {open && !singleCall ? (
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
            open={singleCall && open}
            presentation={presentation}
            showChevron={false}
          />
        )}
        {expandable ? (
          <span {...stylex.props(styles.chevron, open && styles.chevronOpen)} aria-hidden="true">
            <ChevronDownIcon size={14} strokeWidth={2} />
          </span>
        ) : null}
      </>
    );
    return (
      <div {...stylex.props(styles.root)}>
        <div {...stylex.props(styles.frame, styles.groupFrame)} data-slot="tool-marker-group">
          <button
            type="button"
            {...stylex.props(styles.row, expandable && styles.rowButton)}
            aria-controls={expandable && !singleCall ? groupContentId : undefined}
            aria-expanded={expandable ? open : undefined}
            data-slot="tool-marker-group-row"
            disabled={!expandable}
            onClick={expandable ? onToggle : undefined}
            title={collapsedCall.status === "error" ? collapsedCall.errorMessage : undefined}
          >
            {rowContent}
          </button>
          <SpringDisclosure open={!singleCall && open} id={groupContentId}>
                <div {...stylex.props(styles.groupContent, styles.groupContentInner)}>
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
          </SpringDisclosure>
        </div>
      </div>
    );
  }

  const defaultCall = calls[0];
  const call = !interactive
    ? { ...defaultCall, expandable: false }
    : detail === undefined
      ? defaultCall
      : { ...defaultCall, expandable: true, resultDetail: detail };

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.frame)} data-slot="tool-marker">
        <button
          type="button"
          {...stylex.props(styles.row, call.expandable && styles.rowButton)}
          aria-expanded={call.expandable ? open : undefined}
          data-slot="tool-marker-row"
          disabled={!call.expandable}
          onClick={call.expandable ? onToggle : undefined}
          title={call.status === "error" ? call.errorMessage : undefined}
        >
          <ToolMarkerRowContent call={call} open={call.expandable && open} presentation={presentation} />
        </button>
        <SpringDisclosure open={Boolean(renderDetail && call.expandable && open && call.resultDetail)}>
          <div {...stylex.props(styles.detail)}>{call.resultDetail}</div>
        </SpringDisclosure>
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
      <ToolTypeIcon kind={call.toolKind} />
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
      {call.expandable && showChevron ? (
        <span {...stylex.props(styles.chevron, open && styles.chevronOpen)} aria-hidden="true">
          <ChevronDownIcon size={14} strokeWidth={2} />
        </span>
      ) : null}
    </>
  );
}

function ToolTypeIcon({ kind }: { kind?: ToolMarkerKind }) {
  if (!kind) {
    return null;
  }
  const label = kind === "web.search" ? "Web search" : "Web page";
  const Icon = kind === "web.search" ? SearchIcon : Globe2Icon;
  return (
    <span {...stylex.props(styles.toolIcon)} role="img" aria-label={label}>
      <Icon aria-hidden="true" size={14} strokeWidth={1.8} />
    </span>
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
  const expandable = toolMarkerExpandable(marker);
  const errorMessage =
    marker.result?.item.status === "FAILED" ? marker.result.item.summary ?? marker.result.item.title : undefined;
  const call: ToolMarkerCall = {
    key: marker.id,
    name: toolMarkerName(marker),
    toolKind: toolMarkerKind(marker),
    status: toolMarkerStatus(marker),
    expandable
  };

  if (expandable) {
    call.resultDetail = <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} />;
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
