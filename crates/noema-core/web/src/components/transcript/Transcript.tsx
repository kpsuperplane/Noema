import * as React from "react";
import type { TranscriptEntry } from "@/shared/types";
import { ActivityRow } from "./ActivityRow";
import { ErrorNotice } from "./ErrorNotice";
import { Message } from "./Message";
import { RenderedTranscriptEntryFrame } from "./RenderedTranscriptEntryFrame";
import {
  renderableTranscriptEntries,
  renderedEntryMessageId,
  renderedTranscriptLane,
  shouldAnchorRenderedEntry,
  shouldAnimateRenderedEntryArrivalForSeen,
  shouldContinueRenderedEntryTextAnimation,
  shouldAnimateRenderedEntryTextForSeen,
  shouldCompactMarkerClusterSpacing,
  type RenderTranscriptEntry
} from "./renderModel";
import {
  initialSeenArrivalMessageIds,
  isScrolledToBottom,
  transcriptArrivalScrollKey,
  transcriptScrollKey
} from "./scrollModel";
import { StructuredCard } from "./StructuredCard";
import { toolMarkerExpandable } from "./markerModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";
import { ToolMarker } from "./ToolMarker";
import { TranscriptBottomFollower, ARRIVAL_SCROLL_SETTLE_DURATION_MS } from "./TranscriptBottomFollower";
import { TranscriptRow } from "./TranscriptRow";
import { TranscriptScroller, TranscriptScrollerItem, TranscriptScrollerProvider, useTranscriptScroller } from "./TranscriptScroller";
import { TypingMessage } from "./TypingMessage";
import type { ConversationAgentStatus } from "@/shared/types";
import { parseProviderUsageDebug } from "./debugUsage";

const TOOL_DETAIL_EXIT_DURATION_MS = 400;

export function Transcript({
  entries,
  loadingOlderTranscript,
  hasMoreTranscriptBefore,
  olderTranscriptPageError,
  pending,
  awaitingAssistantTurn,
  agentStatus,
  expandedActivities,
  sentMessageScrollRequest,
  onToggleActivity,
  onLoadOlderTranscript
}: {
  entries: TranscriptEntry[];
  loadingOlderTranscript: boolean;
  hasMoreTranscriptBefore: boolean;
  olderTranscriptPageError: string | null;
  pending: boolean;
  awaitingAssistantTurn: boolean;
  agentStatus: ConversationAgentStatus;
  expandedActivities: Set<string>;
  sentMessageScrollRequest: number;
  onToggleActivity: (id: string) => void;
  onLoadOlderTranscript: () => void;
}) {
  void awaitingAssistantTurn;
  const renderedEntries = renderableTranscriptEntries(entries, pending, agentStatus);
  const [seenArrivalMessageIds, setSeenArrivalMessageIds] = React.useState<ReadonlySet<string>>(() =>
    initialSeenArrivalMessageIds(renderedEntries)
  );
  const [textAnimatingMessageIds, setTextAnimatingMessageIds] = React.useState<ReadonlySet<string>>(() => new Set());
  const followBottomRef = React.useRef(true);
  const scrollKey = transcriptScrollKey(renderedEntries);
  const arrivalScrollKey = transcriptArrivalScrollKey(renderedEntries, seenArrivalMessageIds);
  const handleViewportScroll = React.useCallback((event: React.UIEvent<HTMLDivElement>) => {
    followBottomRef.current = isScrolledToBottom(event.currentTarget);
  }, []);

  React.useEffect(() => {
    const nextSeenMessageIds = new Set(seenArrivalMessageIds);
    const nextTextAnimatingMessageIds = new Set(textAnimatingMessageIds);
    let changed = false;
    let textAnimatingChanged = false;
    for (const entry of renderedEntries) {
      const messageId = renderedEntryMessageId(entry);
      if (!nextSeenMessageIds.has(messageId)) {
        nextSeenMessageIds.add(messageId);
        changed = true;
        if (shouldContinueRenderedEntryTextAnimation(entry) && !nextTextAnimatingMessageIds.has(messageId)) {
          nextTextAnimatingMessageIds.add(messageId);
          textAnimatingChanged = true;
        }
      }
    }
    if (!changed && !textAnimatingChanged) {
      return;
    }

    let cancelled = false;
    if (textAnimatingChanged) {
      window.queueMicrotask(() => {
        if (!cancelled) {
          setTextAnimatingMessageIds(nextTextAnimatingMessageIds);
        }
      });
    }

    const timeout = window.setTimeout(() => {
      setSeenArrivalMessageIds(nextSeenMessageIds);
    }, arrivalScrollKey ? ARRIVAL_SCROLL_SETTLE_DURATION_MS : 0);

    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
    };
  }, [arrivalScrollKey, renderedEntries, seenArrivalMessageIds, textAnimatingMessageIds]);

  return (
    <TranscriptScrollerProvider>
      <TranscriptScroller
        aria-label="Conversation transcript"
        entries={renderedEntries}
        hasMoreBefore={hasMoreTranscriptBefore}
        loadingBefore={loadingOlderTranscript}
        loadBeforeError={olderTranscriptPageError}
        onLoadBefore={onLoadOlderTranscript}
        onViewportScroll={handleViewportScroll}
        renderEntry={(entry, index) => {
          const lane = transcriptLane(entry);
          const previousEntry = renderedEntries[index - 1];
          const previousLane = previousEntry ? transcriptLane(previousEntry) : null;
          const showAvatar = previousLane !== lane;
          const messageId = renderedEntryMessageId(entry);
          const animateArrival = shouldAnimateRenderedEntryArrivalForSeen(entry, messageId, seenArrivalMessageIds);
          const animateText = shouldAnimateRenderedEntryTextForSeen(
            entry,
            messageId,
            seenArrivalMessageIds,
            textAnimatingMessageIds
          );

          return (
            <TranscriptScrollerItem
              key={messageId}
              align={lane === "human" ? "end" : "start"}
              compact={shouldCompactMarkerClusterSpacing(entry, previousEntry)}
              data-arrival={animateArrival ? "true" : undefined}
              messageId={messageId}
              scrollAnchor={shouldAnchorRenderedEntry(entry)}
            >
              <RenderedTranscriptEntryFrame lane={lane}>
                {renderTranscriptRenderEntry(
                  entry,
                  expandedActivities,
                  onToggleActivity,
                  showAvatar,
                  animateText,
                  followBottomRef
                )}
              </RenderedTranscriptEntryFrame>
            </TranscriptScrollerItem>
          );
        }}
      />
      <TranscriptBottomFollower
        arrivalScrollKey={arrivalScrollKey}
        followBottomRef={followBottomRef}
        sentMessageScrollRequest={sentMessageScrollRequest}
        scrollKey={scrollKey}
      />
    </TranscriptScrollerProvider>
  );
}

function transcriptLane(entry: RenderTranscriptEntry) {
  return entry.kind === "entry"
    ? renderedTranscriptLane({ kind: "entry", entryType: entry.entry.type })
    : renderedTranscriptLane({ kind: entry.kind });
}

function renderTranscriptRenderEntry(
  entry: RenderTranscriptEntry,
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void,
  showAvatar: boolean,
  animateText: boolean,
  followBottomRef: React.MutableRefObject<boolean>
) {
  if (entry.kind === "tool_marker") {
    const open = expandedActivities.has(entry.id);
    return (
      <>
        <TranscriptRow lane="assistant" showAvatar={showAvatar}>
          <ToolMarker
            data={{ kind: "tool", marker: entry.marker }}
            open={open}
            onToggle={() => onToggleActivity(entry.id)}
            renderDetail={false}
          />
        </TranscriptRow>
        <AnimatedToolDetailRow
          open={open && toolMarkerExpandable(entry.marker)}
          marker={entry.marker}
          followBottomRef={followBottomRef}
        />
      </>
    );
  }
  if (entry.kind === "typing") {
    return <TypingMessage showAvatar={showAvatar} />;
  }
  return renderTranscriptEntry(entry.entry, expandedActivities, onToggleActivity, showAvatar, animateText);
}

function renderTranscriptEntry(
  entry: TranscriptEntry,
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void,
  showAvatar: boolean,
  animateText: boolean
) {
  if (entry.type === "user") {
    return <Message animate={animateText} role="user" text={entry.text} showAvatar={showAvatar} />;
  }
  if (entry.type === "assistant") {
    return (
      <Message
        animate={animateText}
        role="assistant"
        text={entry.text}
        showAvatar={showAvatar}
        debugUsage={parseProviderUsageDebug(entry.metadata)}
      />
    );
  }
  if (entry.type === "assistant_stream") {
    return <Message animate={animateText} role="assistant" text={entry.text} showAvatar={showAvatar} />;
  }
  if (entry.type === "activity") {
    return (
      <ActivityRow item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
    );
  }
  if (entry.type === "card") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <StructuredCard item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
      </TranscriptRow>
    );
  }
  return <ErrorNotice message={entry.message} recoverable={entry.recoverable} />;
}

function AnimatedToolDetailRow({
  open,
  marker,
  followBottomRef
}: {
  open: boolean;
  marker: Extract<RenderTranscriptEntry, { kind: "tool_marker" }>["marker"];
  followBottomRef: React.MutableRefObject<boolean>;
}) {
  const { scrollToEnd } = useTranscriptScroller();
  const [rendered, setRendered] = React.useState(open);
  const [phase, setPhase] = React.useState<"entering" | "current" | "exiting">(() => (open ? "entering" : "exiting"));

  React.useEffect(() => {
    let animationFrame: number | null = null;
    let timeout: number | null = null;

    if (open && (!rendered || phase === "exiting")) {
      animationFrame = window.requestAnimationFrame(() => {
        setRendered(true);
        setPhase("entering");
      });
    } else if (!open && rendered) {
      animationFrame = window.requestAnimationFrame(() => {
        setPhase("exiting");
      });
      timeout = window.setTimeout(() => {
        setRendered(false);
      }, TOOL_DETAIL_EXIT_DURATION_MS);
    }

    return () => {
      if (animationFrame !== null) {
        window.cancelAnimationFrame(animationFrame);
      }
      if (timeout !== null) {
        window.clearTimeout(timeout);
      }
    };
  }, [open, phase, rendered]);

  React.useLayoutEffect(() => {
    if (!rendered || !open || !followBottomRef.current) {
      return;
    }

    scrollToEnd({ behavior: "auto" });
  }, [followBottomRef, open, rendered, scrollToEnd]);

  if (!rendered) {
    return null;
  }

  return (
    <div
      data-slot="tool-detail-row-motion"
      data-state={phase}
      onAnimationEnd={() => {
        if (phase === "entering") {
          setPhase("current");
        } else if (phase === "exiting") {
          setRendered(false);
        }
      }}
    >
      <div data-slot="tool-detail-row-motion-inner">
        <TranscriptRow lane="assistant" showAvatar={false}>
          <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} />
        </TranscriptRow>
      </div>
    </div>
  );
}
