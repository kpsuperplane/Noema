import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport
} from "@/components/ui/message-scroller";
import { cn } from "@/lib/utils";
import * as React from "react";
import type { TranscriptEntry } from "../../types";
import { ActivityRow } from "./ActivityRow";
import { ErrorNotice } from "./ErrorNotice";
import { Message } from "./Message";
import { MemoryMarker } from "./MemoryMarker";
import { RenderedTranscriptEntryFrame } from "./RenderedTranscriptEntryFrame";
import {
  renderableTranscriptEntries,
  renderedEntryMessageId,
  renderedTranscriptLane,
  shouldAnchorRenderedEntry,
  shouldAnimateRenderedEntryArrivalForSeen,
  shouldAnimateRenderedEntryText,
  shouldAnimateRenderedEntryTextForSeen,
  shouldCompactMarkerClusterSpacing,
  shouldRevealRenderedEntryAfterArrival,
  type RenderTranscriptEntry
} from "./renderModel";
import {
  initialSeenArrivalMessageIds,
  isScrolledToBottom,
  transcriptArrivalScrollKey,
  transcriptScrollKey
} from "./scrollModel";
import { StructuredCard } from "./StructuredCard";
import { ToolMarker } from "./ToolMarker";
import { TranscriptBottomFollower, ARRIVAL_SCROLL_FOLLOW_DURATION_MS } from "./TranscriptBottomFollower";
import { TranscriptRow } from "./TranscriptRow";
import { TypingMessage } from "./TypingMessage";
import type { ConversationAgentStatus } from "../../types";

export function Transcript({
  entries,
  pending,
  agentStatus,
  expandedActivities,
  onToggleActivity
}: {
  entries: TranscriptEntry[];
  pending: boolean;
  agentStatus: ConversationAgentStatus;
  expandedActivities: Set<string>;
  onToggleActivity: (id: string) => void;
}) {
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
        if (shouldAnimateRenderedEntryText(entry) && !nextTextAnimatingMessageIds.has(messageId)) {
          nextTextAnimatingMessageIds.add(messageId);
          textAnimatingChanged = true;
        }
      }
    }
    if (!changed && !textAnimatingChanged) {
      return;
    }

    const timeout = window.setTimeout(() => {
      if (changed) {
        setSeenArrivalMessageIds(nextSeenMessageIds);
      }
      if (textAnimatingChanged) {
        setTextAnimatingMessageIds(nextTextAnimatingMessageIds);
      }
    }, arrivalScrollKey ? ARRIVAL_SCROLL_FOLLOW_DURATION_MS : 0);

    return () => {
      window.clearTimeout(timeout);
    };
  }, [arrivalScrollKey, renderedEntries, seenArrivalMessageIds, textAnimatingMessageIds]);

  return (
    <MessageScrollerProvider autoScroll defaultScrollPosition="end" scrollPreviousItemPeek={56}>
      <MessageScroller className="min-h-0 overflow-hidden">
        <MessageScrollerViewport aria-label="Conversation transcript" onScroll={handleViewportScroll}>
          <MessageScrollerContent className="mx-auto flex min-h-full w-[var(--chat-column-width)] flex-col justify-end gap-3 px-0.5 py-6">
            {renderedEntries.map((entry, index) => {
              const lane = transcriptLane(entry);
              const previousEntry = renderedEntries[index - 1];
              const previousLane = previousEntry ? transcriptLane(previousEntry) : null;
              const showAvatar = previousLane !== lane;
              const messageId = renderedEntryMessageId(entry);
              const animateArrival = shouldAnimateRenderedEntryArrivalForSeen(
                entry,
                messageId,
                seenArrivalMessageIds
              );
              const previousEntryMessageId = previousEntry ? renderedEntryMessageId(previousEntry) : null;
              const previousEntryAnimateArrival =
                previousEntry && previousEntryMessageId
                  ? shouldAnimateRenderedEntryArrivalForSeen(previousEntry, previousEntryMessageId, seenArrivalMessageIds)
                  : false;
              const revealAfterArrival = shouldRevealRenderedEntryAfterArrival(entry, previousEntryAnimateArrival);
              const animateText = shouldAnimateRenderedEntryTextForSeen(
                entry,
                messageId,
                seenArrivalMessageIds,
                textAnimatingMessageIds
              );

              return (
                <MessageScrollerItem
                  key={messageId}
                  className={cn(
                    "flex w-full",
                    lane === "human" && "justify-end",
                    shouldCompactMarkerClusterSpacing(entry, previousEntry) && "-mt-2"
                  )}
                  data-arrival={animateArrival ? "true" : undefined}
                  data-reveal-after-arrival={revealAfterArrival ? "true" : undefined}
                  messageId={messageId}
                  scrollAnchor={shouldAnchorRenderedEntry(entry)}
                >
                  <RenderedTranscriptEntryFrame animateArrival={animateArrival}>
                    {renderTranscriptRenderEntry(entry, expandedActivities, onToggleActivity, showAvatar, animateText)}
                  </RenderedTranscriptEntryFrame>
                </MessageScrollerItem>
              );
            })}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
      <TranscriptBottomFollower
        arrivalScrollKey={arrivalScrollKey}
        followBottomRef={followBottomRef}
        scrollKey={scrollKey}
      />
    </MessageScrollerProvider>
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
  animateText: boolean
) {
  if (entry.kind === "memory_marker") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <MemoryMarker
          id={entry.id}
          extraction={entry.extraction}
          proposal={entry.proposal}
          open={expandedActivities.has(entry.id)}
          onToggle={() => onToggleActivity(entry.id)}
        />
      </TranscriptRow>
    );
  }
  if (entry.kind === "tool_marker") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <ToolMarker
          marker={entry.marker}
          open={expandedActivities.has(entry.id)}
          onToggle={() => onToggleActivity(entry.id)}
        />
      </TranscriptRow>
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
    return <Message animate={animateText} role="assistant" text={entry.text} showAvatar={showAvatar} />;
  }
  if (entry.type === "assistant_stream") {
    return <Message animate={animateText} role="assistant" text={entry.text} showAvatar={showAvatar} />;
  }
  if (entry.type === "activity") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <ActivityRow item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
      </TranscriptRow>
    );
  }
  if (entry.type === "card") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <StructuredCard item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
      </TranscriptRow>
    );
  }
  return (
    <TranscriptRow lane="assistant" showAvatar={showAvatar}>
      <ErrorNotice message={entry.message} recoverable={entry.recoverable} />
    </TranscriptRow>
  );
}
