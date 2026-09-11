import * as React from "react";
import { Text } from "@astryxdesign/core/Text";
import { ContextMenu } from "@astryxdesign/core/ContextMenu";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import {
  avatarActivityForAgentStatus,
  type IdentityAvatarActivity
} from "@/components/IdentityAvatar";
import { SpringDisclosure } from "@/motion/SpringDisclosure";
import type { A2UIActionSubmission, RuntimeDebugScope, TranscriptEntry } from "@/shared/types";
import { ActivityRow, activityRendersAsSystemNotice } from "./ActivityRow";
import { ArtifactReferenceCard } from "./ArtifactReferenceCard";
import { ErrorNotice } from "./ErrorNotice";
import { RenderErrorBoundary } from "@/components/errors/RenderErrorBoundary";
import { Message } from "./Message";
import { MultipleChoicePrompt } from "./MultipleChoicePrompt";
import { RenderedTranscriptEntryFrame } from "./RenderedTranscriptEntryFrame";
import {
  providerCitationsFromMetadata
} from "./ProviderCitationSources";
import {
  renderableTranscriptEntries,
  renderedChatBubbleGroup,
  renderedEntryMessageId,
  renderedTranscriptLane,
  rendersPrimaryAssistantAvatar,
  shouldAnchorRenderedEntry,
  shouldAnimateRenderedEntryArrivalForSeen,
  shouldContinueRenderedEntryTextAnimation,
  shouldAnimateRenderedEntryTextForSeen,
  shouldCompactMarkerClusterSpacing,
  transcriptEntryRenderId,
  type ChatBubbleGroup,
  type RenderTranscriptEntry
} from "./renderModel";
import {
  initialSeenArrivalMessageIds,
  isScrolledToBottom,
  transcriptArrivalMessageIds,
  transcriptArrivalScrollKey,
  transcriptScrollKey
} from "./scrollModel";
import { A2UISurface } from "./A2UISurface";
import { TaskReferenceCard } from "./TaskReferenceCard";
import { TranscriptInputMessage } from "./TranscriptInputMessage";
import { toolMarkerExpandable } from "./markerModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";
import { ToolMarker } from "./ToolMarker";
import { TranscriptBottomFollower } from "./TranscriptBottomFollower";
import { TranscriptRow } from "./TranscriptRow";
import {
  TranscriptScroller,
  TranscriptScrollerItem,
  TranscriptScrollerProvider,
  readTranscriptScrollSnapshot,
  type TranscriptScrollRestoration
} from "./TranscriptScroller";
import type { ConversationAgentStatus } from "@/shared/types";
import { parseProviderUsageDebug } from "./debugUsage";
import {
  RuntimeDebugDialog,
  type RuntimeDebugTarget
} from "./RuntimeDebugDialog";

export type TranscriptDensity = "full" | "embedded";

export function Transcript({
  entries,
  scrollRestoration,
  scrollRestorationKey,
  loadingOlderTranscript,
  hasMoreTranscriptBefore,
  olderTranscriptPageError,
  pending,
  awaitingAssistantTurn,
  agentStatus,
  expandedActivities,
  sentMessageScrollRequest,
  onToggleActivity,
  onSubmitA2UIAction,
  onSubmitMultipleChoiceSelection,
  onLoadOlderTranscript,
  onOpenDetail,
  density = "full",
  showActorAvatars = true,
  showTypingIndicator,
  collapseConsecutiveToolCalls = false,
  ariaLabel = "Conversation transcript"
}: {
  entries: TranscriptEntry[];
  scrollRestoration?: TranscriptScrollRestoration;
  scrollRestorationKey?: string;
  loadingOlderTranscript: boolean;
  hasMoreTranscriptBefore: boolean;
  olderTranscriptPageError: string | null;
  pending: boolean;
  awaitingAssistantTurn: boolean;
  agentStatus: ConversationAgentStatus;
  expandedActivities: Set<string>;
  sentMessageScrollRequest: number;
  onToggleActivity: (id: string) => void;
  onSubmitA2UIAction?: (action: A2UIActionSubmission) => void;
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void;
  onLoadOlderTranscript: () => void;
  onOpenDetail?: (target: ChatDetailTarget) => void;
  density?: TranscriptDensity;
  showActorAvatars?: boolean;
  showTypingIndicator?: boolean;
  collapseConsecutiveToolCalls?: boolean;
  ariaLabel?: string;
}) {
  const activeTurn = pending || awaitingAssistantTurn;
  const previousActiveTurnRef = React.useRef(activeTurn);
  const [completionAnnouncementKey, setCompletionAnnouncementKey] = React.useState(0);
  const [transcriptScrolling, setTranscriptScrolling] = React.useState(false);
  const [historyAnchorEntries, setHistoryAnchorEntries] = React.useState(entries);
  const latestEntriesRef = React.useRef(entries);
  const transcriptScrollingRef = React.useRef(false);
  React.useLayoutEffect(() => {
    latestEntriesRef.current = entries;
  }, [entries]);
  React.useEffect(() => {
    if (previousActiveTurnRef.current && !activeTurn) {
      setCompletionAnnouncementKey((current) => current + 1);
    }
    previousActiveTurnRef.current = activeTurn;
  }, [activeTurn]);
  const historyPrependDeferred =
    transcriptScrolling && isHistoryPrepend(historyAnchorEntries, entries);
  const visibleEntries = historyPrependDeferred ? historyAnchorEntries : entries;
  const handleScrollActivityChange = React.useCallback((active: boolean) => {
    const wasActive = transcriptScrollingRef.current;
    transcriptScrollingRef.current = active;
    setTranscriptScrolling(active);
    if (!active || !wasActive) {
      setHistoryAnchorEntries(latestEntriesRef.current);
    }
  }, []);
  const handleLoadOlderTranscript = React.useCallback(() => {
    setHistoryAnchorEntries(entries);
    onLoadOlderTranscript();
  }, [entries, onLoadOlderTranscript]);
  const renderedEntries = React.useMemo(
    () => renderableTranscriptEntries(visibleEntries, pending, agentStatus, collapseConsecutiveToolCalls, showTypingIndicator),
    [agentStatus, collapseConsecutiveToolCalls, pending, showTypingIndicator, visibleEntries]
  );
  const [seenArrivalMessageIds, setSeenArrivalMessageIds] = React.useState<ReadonlySet<string>>(() =>
    initialSeenArrivalMessageIds(renderedEntries)
  );
  const [textAnimatingMessageIds, setTextAnimatingMessageIds] = React.useState<ReadonlySet<string>>(() => new Set());
  const [debugTarget, setDebugTarget] = React.useState<RuntimeDebugTarget | null>(null);
  const [restoredScroll] = React.useState(() =>
    readTranscriptScrollSnapshot(scrollRestoration, scrollRestorationKey, density)
  );
  const followBottomRef = React.useRef(restoredScroll?.followBottom ?? true);
  const scrollKey = transcriptScrollKey(renderedEntries);
  const arrivalMessageIds = transcriptArrivalMessageIds(renderedEntries, seenArrivalMessageIds);
  const arrivalMessageIdsJson = JSON.stringify(arrivalMessageIds);
  const arrivalScrollKey = transcriptArrivalScrollKey(renderedEntries, seenArrivalMessageIds);
  const latestAssistantAvatar = latestAssistantAvatarAnchor(renderedEntries);
  const latestAssistantAvatarMotion = conversationAvatarMotion(
    renderedEntries,
    latestAssistantAvatar?.humanIndex ?? -1,
    agentStatus
  );
  const handleViewportScroll = React.useCallback((event: React.UIEvent<HTMLDivElement>) => {
    followBottomRef.current = isScrolledToBottom(event.currentTarget);
  }, []);

  const markArrivalsSettled = React.useCallback((messageIds: readonly string[]) => {
    setSeenArrivalMessageIds((current) => {
      const next = new Set(current);
      for (const messageId of messageIds) {
        next.add(messageId);
      }
      return next.size === current.size ? current : next;
    });
  }, []);

  React.useEffect(() => {
    const nextSeenMessageIds = new Set(seenArrivalMessageIds);
    const nextTextAnimatingMessageIds = new Set(textAnimatingMessageIds);
    const arrivalMessageIdSet = new Set(arrivalMessageIds);
    let immediateSeenChanged = false;
    let textAnimatingChanged = false;
    for (const entry of renderedEntries) {
      const messageId = renderedEntryMessageId(entry);
      if (!nextSeenMessageIds.has(messageId)) {
        if (!arrivalMessageIdSet.has(messageId)) {
          nextSeenMessageIds.add(messageId);
          immediateSeenChanged = true;
        }
        if (shouldContinueRenderedEntryTextAnimation(entry) && !nextTextAnimatingMessageIds.has(messageId)) {
          nextTextAnimatingMessageIds.add(messageId);
          textAnimatingChanged = true;
        }
      }
    }
    if (!immediateSeenChanged && !textAnimatingChanged) {
      return;
    }

    let cancelled = false;
    window.queueMicrotask(() => {
      if (!cancelled) {
        if (immediateSeenChanged) {
          setSeenArrivalMessageIds(nextSeenMessageIds);
        }
        if (textAnimatingChanged) {
          setTextAnimatingMessageIds(nextTextAnimatingMessageIds);
        }
      }
    });
    return () => {
      cancelled = true;
    };
  }, [arrivalMessageIds, renderedEntries, seenArrivalMessageIds, textAnimatingMessageIds]);

  return (
    <TranscriptScrollerProvider>
      <TranscriptScroller
        aria-label={ariaLabel}
        density={density}
        entries={renderedEntries}
        followBottomRef={followBottomRef}
        hasMoreBefore={hasMoreTranscriptBefore}
        historyPrependDeferred={historyPrependDeferred}
        loadingBefore={loadingOlderTranscript}
        loadBeforeError={olderTranscriptPageError}
        busy={activeTurn}
        completionAnnouncementKey={completionAnnouncementKey}
        onLoadBefore={handleLoadOlderTranscript}
        onScrollActivityChange={handleScrollActivityChange}
        onViewportScroll={handleViewportScroll}
        scrollRestoration={scrollRestoration}
        scrollRestorationKey={scrollRestorationKey}
        renderEntry={(entry, index) => {
          const lane = transcriptLane(entry);
          const previousEntry = renderedEntries[index - 1];
          const nextEntry = renderedEntries[index + 1];
          const previousLane = previousEntry ? transcriptLane(previousEntry) : null;
          const ownsLatestAssistantAvatar = index === latestAssistantAvatar?.index;
          const showAvatar = showActorAvatars && (previousLane !== lane || ownsLatestAssistantAvatar);
          const bubbleGroup = renderedChatBubbleGroup(entry, previousEntry, nextEntry);
          const messageId = renderedEntryMessageId(entry);
          const animateArrival = shouldAnimateRenderedEntryArrivalForSeen(entry, messageId, seenArrivalMessageIds);
          const animateText = shouldAnimateRenderedEntryTextForSeen(
            entry,
            messageId,
            seenArrivalMessageIds,
            textAnimatingMessageIds
          );
          const avatarActivity = ownsLatestAssistantAvatar ? latestAssistantAvatarMotion.activity : "idle";
          const avatarAnimated = ownsLatestAssistantAvatar && latestAssistantAvatarMotion.animated;
          const systemNotice = entry.kind === "entry" && (
            entry.entry.type === "error"
            || (entry.entry.type === "activity" && activityRendersAsSystemNotice(entry.entry.item))
          );

          return (
            <TranscriptScrollerItem
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
              <RenderErrorBoundary
                errorScope={`transcript.item.${messageId}`}
                resetKey={entry}
                fallback={() => (
                  <ErrorNotice
                    message="This conversation item could not display."
                    recoverable
                  />
                )}
              >
                <TranscriptEntryRender
                  render={() => (
                    <RenderedTranscriptEntryFrame
                      animateArrival={animateArrival}
                      lane={lane}
                      systemNotice={systemNotice}
                      systemNoticeHasAvatarGutters={showActorAvatars}
                    >
                      {renderTranscriptRenderEntry(
                      entry,
                      visibleEntries,
                      expandedActivities,
                      onToggleActivity,
                      onSubmitA2UIAction,
                      onSubmitMultipleChoiceSelection,
                      pending,
                      avatarActivity,
                      avatarAnimated,
                      showAvatar,
                      showActorAvatars,
                      bubbleGroup,
                      animateText,
                      onOpenDetail,
                      setDebugTarget
                      )}
                    </RenderedTranscriptEntryFrame>
                  )}
                />
              </RenderErrorBoundary>
            </TranscriptScrollerItem>
          );
        }}
      />
      <TranscriptBottomFollower
        arrivalScrollKey={arrivalScrollKey}
        arrivalMessageIdsJson={arrivalMessageIdsJson}
        followBottomRef={followBottomRef}
        restoreInitialScroll={restoredScroll !== null}
        onArrivalSettled={markArrivalsSettled}
        sentMessageScrollRequest={sentMessageScrollRequest}
        scrollKey={scrollKey}
      />
      <RuntimeDebugDialog
        target={debugTarget}
        onOpenChange={(open) => {
          if (!open) setDebugTarget(null);
        }}
      />
    </TranscriptScrollerProvider>
  );
}

function TranscriptEntryRender({
  render
}: {
  render: () => React.ReactNode;
}) {
  return render();
}

function isHistoryPrepend(current: readonly TranscriptEntry[], next: readonly TranscriptEntry[]) {
  if (current.length === 0 || next.length <= current.length) {
    return false;
  }
  const firstCurrentId = transcriptEntryRenderId(current[0]);
  const offset = next.findIndex((entry) => transcriptEntryRenderId(entry) === firstCurrentId);
  if (offset <= 0 || next.length - offset < current.length) {
    return false;
  }
  return current.every(
    (entry, index) => transcriptEntryRenderId(entry) === transcriptEntryRenderId(next[offset + index])
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
  onSubmitA2UIAction: ((action: A2UIActionSubmission) => void) | undefined,
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void,
  pending: boolean,
  avatarActivity: IdentityAvatarActivity,
  avatarAnimated: boolean,
  showAvatar: boolean,
  reserveAvatarSpace: boolean,
  bubbleGroup: ChatBubbleGroup | undefined,
  animateText: boolean,
  onOpenDetail: ((target: ChatDetailTarget) => void) | undefined,
  onDebug: (target: RuntimeDebugTarget) => void
) {
  if (entry.kind === "tool_marker" || entry.kind === "tool_marker_group") {
    const grouped = entry.kind === "tool_marker_group";
    const disclosureId = grouped && entry.markers.length > 1 ? `tool-group:${entry.id}` : entry.id;
    const directMarker = grouped ? entry.markers.length === 1 ? entry.markers[0] : undefined : entry.marker;
    const open = expandedActivities.has(disclosureId);
    return (
      <>
        <TranscriptRow
          avatarActivity={avatarActivity}
          avatarAnimated={avatarAnimated}
          lane="assistant"
          reserveAvatarSpace={reserveAvatarSpace}
          showAvatar={showAvatar}
        >
          <DebugMarkerMenu
            target={grouped ? toolGroupDebugTarget(entry.markers) : toolDebugTarget(entry.marker)}
            onDebug={onDebug}
          >
            <ToolMarker
              data={grouped
                ? { kind: "tool_group", markers: entry.markers }
                : { kind: "tool", marker: entry.marker }}
              expandedMarkers={grouped ? expandedActivities : undefined}
              onToggleMarker={grouped ? onToggleActivity : undefined}
              open={open}
              onToggle={() => onToggleActivity(disclosureId)}
              renderDetail={grouped}
            />
          </DebugMarkerMenu>
        </TranscriptRow>
        {directMarker ? (
          <SpringDisclosure
            open={open && toolMarkerExpandable(directMarker)}
            slot="tool-detail-row-motion"
          >
            <TranscriptRow lane="assistant" reserveAvatarSpace={reserveAvatarSpace} showAvatar={false}>
              <ToolDetailAttachment id={`${directMarker.id}-details`} marker={directMarker} />
            </TranscriptRow>
          </SpringDisclosure>
        ) : null}
      </>
    );
  }
  if (entry.kind === "typing") {
    return (
      <Message
        animate={false}
        avatarActivity={avatarActivity}
        avatarAnimated={avatarAnimated}
        reserveAvatarSpace={reserveAvatarSpace}
        role="assistant"
        showAvatar={showAvatar}
        text=""
        variant="typing"
      />
    );
  }
  return renderTranscriptEntry(
    entry.entry,
    entries,
    expandedActivities,
    onToggleActivity,
    onSubmitA2UIAction,
    onSubmitMultipleChoiceSelection,
    pending,
    avatarActivity,
    avatarAnimated,
    showAvatar,
    reserveAvatarSpace,
    bubbleGroup,
    animateText,
    onOpenDetail,
    onDebug
  );
}

function renderTranscriptEntry(
  entry: TranscriptEntry,
  entries: TranscriptEntry[],
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void,
  onSubmitA2UIAction: ((action: A2UIActionSubmission) => void) | undefined,
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void,
  pending: boolean,
  avatarActivity: IdentityAvatarActivity,
  avatarAnimated: boolean,
  showAvatar: boolean,
  reserveAvatarSpace: boolean,
  bubbleGroup: ChatBubbleGroup | undefined,
  animateText: boolean,
  onOpenDetail: ((target: ChatDetailTarget) => void) | undefined,
  onDebug: (target: RuntimeDebugTarget) => void
) {
  if (entry.type === "user") {
    return (
      <Message
        animate={animateText}
        avatarActivity={avatarActivity}
        avatarAnimated={avatarAnimated}
        group={bubbleGroup}
        reserveAvatarSpace={reserveAvatarSpace}
        role="user"
        text={entry.text}
        showAvatar={showAvatar}
      />
    );
  }
  if (entry.type === "input") {
    return <TranscriptInputMessage text={entry.text} />;
  }
  if (entry.type === "assistant") {
    const interrupted = entry.metadata !== null && typeof entry.metadata === "object"
      && "output_status" in entry.metadata && entry.metadata.output_status === "failed";
    const debugUsage = parseProviderUsageDebug(entry.metadata);
    const citations = providerCitationsFromMetadata(entry.metadata);
    const debugTarget = messageDebugTarget(
      entry.debugScope,
      debugUsage,
      entry.responseIndex,
      entry.debugRoundIndex
    );
    return (
      <Message
        animate={animateText}
        avatarActivity={avatarActivity}
        avatarAnimated={avatarAnimated}
        group={bubbleGroup}
        reserveAvatarSpace={reserveAvatarSpace}
        role="assistant"
        presentation={entry.presentation}
        text={entry.text}
        showAvatar={showAvatar}
        citations={citations}
        debugUsage={debugUsage}
        onDebug={debugTarget ? () => onDebug(debugTarget) : undefined}
        attachment={
          interrupted || entry.taskReferences?.length ? (
            <>
              {interrupted ? <Text type="supporting">Response interrupted</Text> : null}
              {entry.taskReferences?.map((reference, index) => (
                <TaskReferenceCard
                  key={`${reference.task_id}:${index}`}
                  taskId={reference.task_id}
                  onOpenDetail={onOpenDetail}
                />
              ))}
            </>
          ) : undefined
        }
      />
    );
  }
  if (entry.type === "assistant_stream") {
    const debugTarget = messageDebugTarget(entry.debugScope, null, entry.responseIndex);
    return (
      <Message
        animate={animateText}
        avatarActivity={avatarActivity}
        avatarAnimated={avatarAnimated}
        group={bubbleGroup}
        reserveAvatarSpace={reserveAvatarSpace}
        role="assistant"
        text={entry.text}
        showAvatar={showAvatar}
        onDebug={debugTarget ? () => onDebug(debugTarget) : undefined}
      />
    );
  }
  if (entry.type === "activity") {
    return (
      <ActivityRow item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
    );
  }
  if (entry.type === "a2ui_surface") {
    return (
      <TranscriptRow avatarActivity={avatarActivity} avatarAnimated={avatarAnimated} lane="assistant" reserveAvatarSpace={reserveAvatarSpace} showAvatar={showAvatar}>
        <A2UISurface
          item={entry.item}
          disabled={pending || !onSubmitA2UIAction}
          onSubmit={onSubmitA2UIAction}
        />
      </TranscriptRow>
    );
  }
  if (entry.type === "artifact") {
    return (
      <TranscriptRow avatarActivity={avatarActivity} avatarAnimated={avatarAnimated} lane="assistant" reserveAvatarSpace={reserveAvatarSpace} showAvatar={showAvatar}>
        <ArtifactReferenceCard item={entry.item} onOpenDetail={onOpenDetail} />
      </TranscriptRow>
    );
  }
  if (entry.type === "task") {
    return (
      <TranscriptRow avatarActivity={avatarActivity} avatarAnimated={avatarAnimated} lane="assistant" reserveAvatarSpace={reserveAvatarSpace} showAvatar={showAvatar}>
        <TaskReferenceCard
          taskId={entry.item.task_id}
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
        disabled={pending || submittedSelectedOptionIds.size > 0}
        group={bubbleGroup}
        item={entry.item}
        promptItemId={promptItemId}
        submittedSelectedOptionIds={submittedSelectedOptionIds}
        reserveAvatarSpace={reserveAvatarSpace}
        showAvatar={showAvatar}
        onSubmit={onSubmitMultipleChoiceSelection}
      />
    );
  }
  if (entry.type === "multiple_choice_selection") {
    return (
      <Message
        animate={animateText}
        reserveAvatarSpace={reserveAvatarSpace}
        role="user"
        text={entry.item.selected_options.map((option) => option.label).join(", ")}
        showAvatar={showAvatar}
      />
    );
  }
  return <ErrorNotice message={entry.message} recoverable={entry.recoverable} />;
}

function latestAssistantAvatarAnchor(renderedEntries: RenderTranscriptEntry[]) {
  for (let humanIndex = renderedEntries.length - 1; humanIndex >= 0; humanIndex -= 1) {
    const humanEntry = renderedEntries[humanIndex];
    if (humanEntry && transcriptLane(humanEntry) === "human") {
      const assistantIndex = renderedEntries.findIndex(
        (entry, index) => index > humanIndex && rendersPrimaryAssistantAvatar(entry)
      );
      return assistantIndex === -1
        ? null
        : {
            humanIndex,
            index: assistantIndex
          };
    }
  }
  return null;
}

function conversationAvatarMotion(
  renderedEntries: RenderTranscriptEntry[],
  humanIndex: number,
  agentStatus: ConversationAgentStatus
) {
  const hasLiveStream = renderedEntries
    .slice(humanIndex + 1)
    .some((entry) => entry.kind === "entry" && entry.entry.type === "assistant_stream");
  const activity = hasLiveStream ? "idle" : avatarActivityForAgentStatus(agentStatus);
  return { activity, animated: hasLiveStream || activity !== "idle" } as const;
}

function DebugMarkerMenu({
  target,
  onDebug,
  children
}: {
  target: RuntimeDebugTarget | null;
  onDebug: (target: RuntimeDebugTarget) => void;
  children: React.ReactNode;
}) {
  if (!target) return children;
  return (
    <ContextMenu items={[{ label: "Debug", onClick: () => onDebug(target) }]}>
      {children}
    </ContextMenu>
  );
}

function messageDebugTarget(
  scope: RuntimeDebugScope | undefined,
  legacyUsage: ReturnType<typeof parseProviderUsageDebug>,
  responseIndex: number | undefined,
  roundIndex?: number
): RuntimeDebugTarget | null {
  if (!scope && !legacyUsage) return null;
  return {
    scope,
    legacyUsage,
    focus: {
      kind: "provider",
      phase: legacyUsage?.phase,
      responseIndex: roundIndex === undefined ? legacyUsage?.responseIndex ?? responseIndex : undefined,
      roundIndex
    }
  };
}

function toolDebugTarget(marker: Extract<RenderTranscriptEntry, { kind: "tool_marker" }>["marker"]): RuntimeDebugTarget | null {
  const item = marker.call ?? marker.result;
  if (!item?.debugScope) return null;
  const correlationId = toolCorrelationId(item);
  return {
    scope: item.debugScope,
    legacyUsage: null,
    focus: correlationId ? { kind: "tool", correlationId } : undefined
  };
}

function toolGroupDebugTarget(
  markers: Extract<RenderTranscriptEntry, { kind: "tool_marker_group" }>["markers"]
): RuntimeDebugTarget | null {
  const scope = (markers[0]?.call ?? markers[0]?.result)?.debugScope;
  return scope ? { scope, legacyUsage: null } : null;
}

function toolCorrelationId(entry: Extract<TranscriptEntry, { type: "activity" }>): string | undefined {
  const metadata = entry.item.metadata;
  if (!metadata || typeof metadata !== "object" || Array.isArray(metadata)) return undefined;
  const action = (metadata as Record<string, unknown>).action;
  if (!action || typeof action !== "object" || Array.isArray(action)) return undefined;
  const record = action as Record<string, unknown>;
  for (const value of [record.correlation_id, record.id, record.call_id]) {
    if (typeof value === "string" && value) return value;
  }
  return undefined;
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
