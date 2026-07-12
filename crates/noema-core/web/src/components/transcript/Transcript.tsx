import * as React from "react";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import type { TranscriptEntry } from "@/shared/types";
import { ActivityRow } from "./ActivityRow";
import { ArtifactReferenceCard } from "./ArtifactReferenceCard";
import { ErrorNotice } from "./ErrorNotice";
import { Message } from "./Message";
import { MultipleChoicePrompt } from "./MultipleChoicePrompt";
import { RenderedTranscriptEntryFrame } from "./RenderedTranscriptEntryFrame";
import {
  renderableTranscriptEntries,
  renderedChatBubbleGroup,
  renderedEntryMessageId,
  renderedTranscriptLane,
  shouldAnchorRenderedEntry,
  shouldAnimateRenderedEntryArrivalForSeen,
  shouldContinueRenderedEntryTextAnimation,
  shouldAnimateRenderedEntryTextForSeen,
  shouldCompactMarkerClusterSpacing,
  collapseTaskReferenceEntries,
  type ChatBubbleGroup,
  type RenderTranscriptEntry
} from "./renderModel";
import {
  initialSeenArrivalMessageIds,
  isScrolledToBottom,
  transcriptArrivalScrollKey,
  transcriptScrollKey
} from "./scrollModel";
import { StructuredCard } from "./StructuredCard";
import { TaskReferenceCard } from "./TaskReferenceCard";
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

export type TranscriptDensity = "full" | "embedded";

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
  onSubmitMultipleChoiceSelection,
  onLoadOlderTranscript,
  onOpenDetail,
  density = "full",
  ariaLabel = "Conversation transcript"
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
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void;
  onLoadOlderTranscript: () => void;
  onOpenDetail?: (target: ChatDetailTarget) => void;
  density?: TranscriptDensity;
  ariaLabel?: string;
}) {
  void awaitingAssistantTurn;
  const displayEntries = React.useMemo(() => collapseTaskReferenceEntries(entries), [entries]);
  const renderedEntries = renderableTranscriptEntries(displayEntries, pending, agentStatus);
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
        aria-label={ariaLabel}
        density={density}
        entries={renderedEntries}
        hasMoreBefore={hasMoreTranscriptBefore}
        loadingBefore={loadingOlderTranscript}
        loadBeforeError={olderTranscriptPageError}
        onLoadBefore={onLoadOlderTranscript}
        onViewportScroll={handleViewportScroll}
        renderEntry={(entry, index) => {
          const lane = transcriptLane(entry);
          const previousEntry = renderedEntries[index - 1];
          const nextEntry = renderedEntries[index + 1];
          const previousLane = previousEntry ? transcriptLane(previousEntry) : null;
          const showAvatar = previousLane !== lane;
          const bubbleGroup = renderedChatBubbleGroup(entry, previousEntry, nextEntry);
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
              compact={
                shouldCompactMarkerClusterSpacing(entry, previousEntry) ||
                bubbleGroup === "middle" ||
                bubbleGroup === "last"
              }
              data-arrival={animateArrival ? "true" : undefined}
              messageId={messageId}
              scrollAnchor={shouldAnchorRenderedEntry(entry)}
            >
              <RenderedTranscriptEntryFrame lane={lane}>
                {renderTranscriptRenderEntry(
                  entry,
                  displayEntries,
                  expandedActivities,
                  onToggleActivity,
                  onSubmitMultipleChoiceSelection,
                  showAvatar,
                  bubbleGroup,
                  animateText,
                  followBottomRef,
                  onOpenDetail
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
  entries: TranscriptEntry[],
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void,
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void,
  showAvatar: boolean,
  bubbleGroup: ChatBubbleGroup | undefined,
  animateText: boolean,
  followBottomRef: React.MutableRefObject<boolean>,
  onOpenDetail: ((target: ChatDetailTarget) => void) | undefined
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
  return renderTranscriptEntry(
    entry.entry,
    entries,
    expandedActivities,
    onToggleActivity,
    onSubmitMultipleChoiceSelection,
    showAvatar,
    bubbleGroup,
    animateText,
    onOpenDetail
  );
}

function renderTranscriptEntry(
  entry: TranscriptEntry,
  entries: TranscriptEntry[],
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void,
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void,
  showAvatar: boolean,
  bubbleGroup: ChatBubbleGroup | undefined,
  animateText: boolean,
  onOpenDetail: ((target: ChatDetailTarget) => void) | undefined
) {
  if (entry.type === "user") {
    return (
      <Message animate={animateText} group={bubbleGroup} role="user" text={entry.text} showAvatar={showAvatar} />
    );
  }
  if (entry.type === "system") {
    return (
      <Message animate={animateText} group={bubbleGroup} role="system" text={entry.text} showAvatar={false} />
    );
  }
  if (entry.type === "assistant") {
    return (
      <Message
        animate={animateText}
        group={bubbleGroup}
        role="assistant"
        text={entry.text}
        showAvatar={showAvatar}
        debugUsage={parseProviderUsageDebug(entry.metadata)}
      />
    );
  }
  if (entry.type === "assistant_stream") {
    return (
      <Message
        animate={animateText}
        group={bubbleGroup}
        role="assistant"
        text={entry.text}
        showAvatar={showAvatar}
      />
    );
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
  if (entry.type === "artifact") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <ArtifactReferenceCard item={entry.item} onOpenDetail={onOpenDetail} />
      </TranscriptRow>
    );
  }
  if (entry.type === "task") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <TaskReferenceCard
          revision={entry.item.revision}
          status={entry.item.status}
          taskId={entry.item.task_id}
          title={entry.item.title}
          onOpenDetail={onOpenDetail}
        />
      </TranscriptRow>
    );
  }
  if (entry.type === "multiple_choice_prompt") {
    const promptItemId = entry.itemId ?? entry.id;
    const submittedSelectedOptionIds = multipleChoicePromptSelectedOptionIds(entries, promptItemId);
    return (
      <MultipleChoicePrompt
        disabled={submittedSelectedOptionIds.size > 0}
        group={bubbleGroup}
        item={entry.item}
        promptItemId={promptItemId}
        submittedSelectedOptionIds={submittedSelectedOptionIds}
        showAvatar={showAvatar}
        onSubmit={onSubmitMultipleChoiceSelection}
      />
    );
  }
  if (entry.type === "multiple_choice_selection") {
    return (
      <Message
        animate={animateText}
        role="user"
        text={entry.item.selected_options.map((option) => option.label).join(", ")}
        showAvatar={showAvatar}
      />
    );
  }
  return <ErrorNotice message={entry.message} recoverable={entry.recoverable} />;
}

function multipleChoicePromptSelectedOptionIds(entries: TranscriptEntry[], promptItemId: string): ReadonlySet<string> {
  const selection = entries.find(
    (entry) => entry.type === "multiple_choice_selection" && entry.item.prompt_item_id === promptItemId
  );
  if (selection?.type !== "multiple_choice_selection") {
    return new Set();
  }
  return new Set(selection.item.selected_options.map((option) => option.id));
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
