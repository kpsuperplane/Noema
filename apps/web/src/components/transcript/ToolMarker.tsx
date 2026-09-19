import * as stylex from "@stylexjs/stylex";
import { Badge } from "@astryxdesign/core/Badge";
import { HStack } from "@astryxdesign/core/HStack";
import { BrainIcon, ChevronDownIcon, CircleSlash2Icon, ClockIcon, Globe2Icon, Loader2Icon, PlugIcon, SearchIcon, WrenchIcon, XIcon } from "lucide-react";
import type { ReactNode } from "react";
import { FaviconImage } from "@/components/FaviconImage";
import { RollingText } from "@/components/RollingText";
import { SpringDisclosure } from "@/motion/SpringDisclosure";
import { toolMarkerExpandable, toolMarkerFaviconHost, toolMarkerKind, toolMarkerSummary, toolMarkerStatus, toolMarkerServerHost, toolMarkerServerKey } from "./markerModel";
import type { ToolMarkerCallStatus, ToolMarkerKind } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";
import { ToolTechnicalRecord } from "./ToolTechnicalRecord";

type ToolMarkerData =
  | {
      kind: "tool";
      marker: ToolMarkerGroup;
      serverHost?: string;
    }
  | {
      kind: "tool_group";
      markers: ToolMarkerGroup[];
    };

type ToolMarkerPresentation = "activity" | "content";

type ToolMarkerCall = {
  key: string;
  name: string;
  toolKind?: ToolMarkerKind;
  faviconHost?: string;
  serverHost?: string;
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
  recordRow: {
    minWidth: 0,
    maxWidth: "100%",
    "--tool-record-opacity": {
      default: 1,
      "@media (hover: hover)": 0
    },
    "--tool-record-pointer-events": {
      default: "auto",
      "@media (hover: hover)": "none"
    },
    ":hover": {
      "--tool-record-opacity": 1,
      "--tool-record-pointer-events": "auto"
    },
    ":focus-within": {
      "--tool-record-opacity": 1,
      "--tool-record-pointer-events": "auto"
    }
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
  onToggleMarker,
  animateText = false
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
  animateText?: boolean;
}) {
  const calls = toolMarkerCalls(data);

  if (calls.length === 0) {
    return null;
  }

  if (data.kind === "tool_group") {
    const collapsedCall = calls[calls.length - 1];
    const singleCall = calls.length === 1;
    const expandable = interactive && (!singleCall || collapsedCall.expandable);
    const groupContentId = `${data.markers[0]?.id ?? "tool-group"}-calls`;
    const rowContent = (
      <>
        {open && !singleCall ? (
          <>
            <span {...stylex.props(styles.groupIcon)} aria-hidden="true">
              {data.markers.every((marker) => marker.message) ? <BrainIcon size={15} strokeWidth={1.8} /> : <WrenchIcon size={15} strokeWidth={1.8} />}
            </span>
            <span {...stylex.props(styles.groupLabel)}>{calls.length} {data.markers.some((marker) => marker.message) ? "activities" : "tool calls"}</span>
          </>
        ) : (
          <ToolMarkerRowContent
            animateText
            call={collapsedCall}
            count={singleCall ? undefined : calls.length - 1}
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
          <HStack gap={0.5} vAlign="center" {...stylex.props(styles.recordRow)}>
            <button
              type="button"
              {...stylex.props(styles.row, expandable && styles.rowButton)}
              aria-controls={expandable && !singleCall ? groupContentId : undefined}
              aria-expanded={expandable ? open : undefined}
              data-slot="tool-marker-group-row"
              disabled={!expandable}
              onClick={expandable ? onToggle : undefined}
              title={collapsedCall.errorMessage ?? collapsedCall.name}
            >
              {rowContent}
            </button>
            {singleCall && data.markers[0] ? <ToolTechnicalRecord marker={data.markers[0]} /> : null}
          </HStack>
          <SpringDisclosure open={!singleCall && open} id={groupContentId}>
            <div {...stylex.props(styles.groupContent, styles.groupContentInner)}>
              <div {...stylex.props(styles.groupList)}>
                {data.markers.map((marker, index) => (
                  <ToolMarker
                    key={marker.id}
                    data={{ kind: "tool", marker, serverHost: calls[index]?.serverHost }}
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
        <HStack gap={0.5} vAlign="center" {...stylex.props(styles.recordRow)}>
          <button
            type="button"
            {...stylex.props(styles.row, call.expandable && styles.rowButton)}
            aria-expanded={call.expandable ? open : undefined}
            data-slot="tool-marker-row"
            disabled={!call.expandable}
            onClick={call.expandable ? onToggle : undefined}
            title={call.errorMessage ?? call.name}
          >
            <ToolMarkerRowContent animateText={animateText} call={call} open={call.expandable && open} presentation={presentation} />
          </button>
          <ToolTechnicalRecord marker={data.marker} />
        </HStack>
        <SpringDisclosure open={Boolean(renderDetail && call.expandable && open && call.resultDetail)}>
          <div {...stylex.props(styles.detail)}>{call.resultDetail}</div>
        </SpringDisclosure>
      </div>
    </div>
  );
}

function ToolMarkerRowContent({
  call,
  count,
  animateText = false,
  open,
  presentation,
  showChevron = true
}: {
  call: ToolMarkerCall;
  count?: number;
  animateText?: boolean;
  open: boolean;
  presentation: ToolMarkerPresentation;
  showChevron?: boolean;
}) {
  return (
    <>
      <ToolStatusIcon status={call.status} />
      <ToolTypeIcon faviconHost={call.faviconHost} kind={call.toolKind} serverHost={call.serverHost} />
      {animateText ? (
        <RollingText value={call.name} {...stylex.props(styles.name)} data-slot="tool-marker-name" />
      ) : (
        <span {...stylex.props(styles.text, styles.name, presentation === "content" && styles.contentName)} data-slot="tool-marker-name">
          {call.name}
        </span>
      )}
      {count ? <Badge variant="neutral" label={`+${count}`} /> : null}
      {call.expandable && showChevron ? (
        <span {...stylex.props(styles.chevron, open && styles.chevronOpen)} aria-hidden="true">
          <ChevronDownIcon size={14} strokeWidth={2} />
        </span>
      ) : null}
    </>
  );
}

function ToolTypeIcon({ faviconHost, kind, serverHost }: { faviconHost?: string; kind?: ToolMarkerKind; serverHost?: string }) {
  if (kind === "mcp") {
    return (
      <span {...stylex.props(styles.toolIcon)} aria-hidden="true">
        {serverHost ? (
          <FaviconImage hostname={serverHost} fallback={<PlugIcon size={14} strokeWidth={1.8} />} />
        ) : <PlugIcon size={14} strokeWidth={1.8} />}
      </span>
    );
  }
  if (!kind) {
    return null;
  }
  const label = kind === "thinking" ? "Thinking" : kind === "web.search" ? "Web search" : faviconHost ? `Website ${faviconHost}` : "Web page";
  const Icon = kind === "thinking" ? BrainIcon : kind === "web.search" ? SearchIcon : Globe2Icon;
  return (
    <span {...stylex.props(styles.toolIcon)} role="img" aria-label={label}>
      {kind === "web.browse" && faviconHost ? (
        <FaviconImage hostname={faviconHost} size="compact" fallback={<Globe2Icon aria-hidden="true" size={14} strokeWidth={1.8} />} />
      ) : (
        <Icon aria-hidden="true" size={14} strokeWidth={1.8} />
      )}
    </span>
  );
}

function ToolStatusIcon({ status }: { status: ToolMarkerCallStatus }) {
  if (status === "complete") {
    return null;
  }
  const label = toolStatusLabel(status);
  if (status === "running") {
    return (
      <span {...stylex.props(styles.statusIcon, styles.running, styles.runningSpinner)} role="img" aria-label={label}>
        <Loader2Icon aria-hidden="true" size={14} strokeWidth={2} />
      </span>
    );
  }
  if (status === "pending") {
    return (
      <span {...stylex.props(styles.statusIcon, styles.pending)} role="img" aria-label={label}>
        <ClockIcon aria-hidden="true" size={14} strokeWidth={2} />
      </span>
    );
  }
  if (status === "error") {
    return (
      <span {...stylex.props(styles.statusIcon, styles.error)} role="img" aria-label={label}>
        <XIcon aria-hidden="true" size={14} strokeWidth={2} />
      </span>
    );
  }
  if (status === "cancelled" || status === "interrupted" || status === "skipped") {
    return (
      <span {...stylex.props(styles.statusIcon, styles.pending)} role="img" aria-label={label}>
        <CircleSlash2Icon aria-hidden="true" size={14} strokeWidth={2} />
      </span>
    );
  }
  return null;
}

function toolMarkerCalls(data: ToolMarkerData): ToolMarkerCall[] {
  if (data.kind === "tool") return [activityToolMarkerCall(data.marker, data.serverHost)];
  const markers = data.markers;
  const serverHosts = new Map<string, string>();
  for (const marker of markers) {
    const key = toolMarkerServerKey(marker);
    const host = toolMarkerServerHost(marker);
    if (key && host) serverHosts.set(key, host);
  }
  return markers.map((marker) => activityToolMarkerCall(marker, serverHosts.get(toolMarkerServerKey(marker) ?? "")));
}

function activityToolMarkerCall(marker: ToolMarkerGroup, serverHost?: string): ToolMarkerCall {
  const expandable = toolMarkerExpandable(marker);
  const errorMessage = marker.result?.item.status === "FAILED" ? (marker.result.item.summary ?? marker.result.item.title) : undefined;
  const call: ToolMarkerCall = {
    key: marker.id,
    name: toolMarkerSummary(marker),
    toolKind: toolMarkerKind(marker),
    faviconHost: toolMarkerFaviconHost(marker),
    serverHost: toolMarkerServerHost(marker) ?? serverHost,
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

function toolStatusLabel(status: ToolMarkerCallStatus): string {
  switch (status) {
    case "pending":
      return "Pending";
    case "running":
      return "Running";
    case "complete":
      return "Completed";
    case "error":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    case "interrupted":
      return "Interrupted";
    case "skipped":
      return "Skipped";
  }
}
