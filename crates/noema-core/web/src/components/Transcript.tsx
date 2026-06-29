import {
  Attachment,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle
} from "@/components/ui/attachment";
import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { Button } from "@/components/ui/button";
import { Marker, MarkerContent, MarkerIcon } from "@/components/ui/marker";
import { ErrorMarker } from "./ErrorMarker";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID, LOCAL_HUMAN_AVATAR_ID } from "./IdentityAvatar";
import { AnimatedMessageText } from "./MessageTextAnimation";
import {
  Message as MessagePrimitive,
  MessageAvatar,
  MessageContent
} from "@/components/ui/message";
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
  useMessageScroller
} from "@/components/ui/message-scroller";
import { cn } from "@/lib/utils";
import { BrainIcon, WrenchIcon } from "lucide-react";
import * as React from "react";
import { readableKind, statusLabel } from "../format";
import { memoryCardsFromStructuredItem, type MemoryCardData } from "../memoryCards";
import type { TranscriptEntry, TurnTranscriptItem } from "../types";

type ActivityTranscriptEntry = Extract<TranscriptEntry, { type: "activity" }>;
type ActivityTranscriptItem = Extract<TurnTranscriptItem, { kind: "activity" }>;

type ToolMarkerGroup = {
  id: string;
  call?: ActivityTranscriptEntry;
  result?: ActivityTranscriptEntry;
};

type RenderTranscriptEntry =
  | { kind: "entry"; id: string; entry: TranscriptEntry; suppressArrival?: boolean }
  | { kind: "typing"; id: string }
  | {
      kind: "memory_marker";
      id: string;
      source?: TranscriptEntry["source"];
      extraction?: ActivityTranscriptItem;
      proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
    }
  | {
      kind: "tool_marker";
      id: string;
      source?: TranscriptEntry["source"];
      marker: ToolMarkerGroup;
      suppressArrival?: boolean;
    };

type TranscriptLane = "human" | "assistant";

type MemoryDetailRowData = { label: string; value: string };

type MemoryClaimOutcome = {
  claimId: string;
  outcome: "created" | "reinforced";
  factPreview?: string;
  sensitivity?: string;
};

export type MemoryDetailItem = {
  title: string;
  rows: MemoryDetailRowData[];
};

type RenderTranscriptLaneCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" }
  | { kind: "tool_marker" };

const ARRIVAL_SCROLL_FOLLOW_DURATION_MS = 360;

export function Transcript({
  entries,
  pending,
  expandedActivities,
  onToggleActivity
}: {
  entries: TranscriptEntry[];
  pending: boolean;
  expandedActivities: Set<string>;
  onToggleActivity: (id: string) => void;
}) {
  const renderedEntries = renderableTranscriptEntries(entries, pending);
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
              const lane =
                entry.kind === "entry"
                  ? renderedTranscriptLane({ kind: "entry", entryType: entry.entry.type })
                  : renderedTranscriptLane({ kind: entry.kind });
              const previousEntry = renderedEntries[index - 1];
              const previousLane =
                previousEntry && previousEntry.kind === "entry"
                  ? renderedTranscriptLane({ kind: "entry", entryType: previousEntry.entry.type })
                  : previousEntry
                    ? renderedTranscriptLane({ kind: previousEntry.kind })
                    : null;
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

function renderableTranscriptEntries(entries: TranscriptEntry[], pending: boolean): RenderTranscriptEntry[] {
  const typingContinuationRenderIds = typingContinuationAssistantRenderIds(entries);
  const renderedEntries = groupTranscriptMarkers(entries).map((entry): RenderTranscriptEntry => {
    if (entry.kind !== "entry" || !typingContinuationRenderIds.has(transcriptEntryRenderId(entry.entry))) {
      return entry;
    }
    return { ...entry, suppressArrival: true };
  });
  if (shouldShowTypingIndicator(entries, pending)) {
    renderedEntries.push({ kind: "typing", id: "typing-indicator" });
  }
  return renderedEntries;
}

type TranscriptEntryAnchorCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" }
  | { kind: "tool_marker" };

export function shouldAnchorTranscriptEntry(entry: TranscriptEntryAnchorCandidate): boolean {
  void entry;
  return false;
}

function shouldAnchorRenderedEntry(entry: RenderTranscriptEntry): boolean {
  if (entry.kind === "entry") {
    return shouldAnchorTranscriptEntry({ kind: "entry", entryType: entry.entry.type });
  }
  return shouldAnchorTranscriptEntry({ kind: entry.kind });
}

function shouldAnimateRenderedEntryArrival(entry: RenderTranscriptEntry): boolean {
  if (entry.kind === "entry" && entry.suppressArrival) {
    return false;
  }
  if (entry.kind === "tool_marker" && entry.suppressArrival) {
    return false;
  }
  if (entry.kind === "typing") {
    return false;
  }
  if (entry.kind === "entry") {
    return entry.entry.source !== "replay";
  }
  return entry.source !== "replay";
}

function shouldAnimateRenderedEntryArrivalForSeen(
  entry: RenderTranscriptEntry,
  messageId: string,
  seenMessageIds: ReadonlySet<string>
): boolean {
  return shouldAnimateMessageArrival({
    eligible: shouldAnimateRenderedEntryArrival(entry),
    messageId,
    seenMessageIds
  });
}

function shouldAnimateMessageArrival({
  eligible,
  messageId,
  seenMessageIds
}: {
  eligible: boolean;
  messageId: string;
  seenMessageIds: ReadonlySet<string>;
}): boolean {
  return eligible && !seenMessageIds.has(messageId);
}

function shouldRevealRenderedEntryAfterArrival(
  entry: RenderTranscriptEntry,
  previousEntryAnimateArrival: boolean
): boolean {
  return entry.kind === "typing" && previousEntryAnimateArrival;
}

function shouldAnimateRenderedEntryText(entry: RenderTranscriptEntry): boolean {
  return entry.kind === "entry" && isTextTranscriptEntry(entry.entry) && shouldAnimateMessageText(entry.entry);
}

function shouldAnimateRenderedEntryTextForSeen(
  entry: RenderTranscriptEntry,
  messageId: string,
  seenMessageIds: ReadonlySet<string>,
  textAnimatingMessageIds: ReadonlySet<string>
): boolean {
  return (
    shouldAnimateRenderedEntryText(entry) &&
    (!seenMessageIds.has(messageId) || textAnimatingMessageIds.has(messageId))
  );
}

function isTextTranscriptEntry(entry: TranscriptEntry): entry is Extract<TranscriptEntry, { text: string }> {
  return "text" in entry;
}

function typingContinuationAssistantRenderIds(entries: TranscriptEntry[]): Set<string> {
  const renderIds = new Set<string>();
  const lastUserIndex = latestUserEntryIndex(entries);
  if (lastUserIndex === -1) {
    return renderIds;
  }

  for (let index = lastUserIndex + 1; index < entries.length; index += 1) {
    const entry = entries[index];
    if (!entry || (entry.type !== "assistant" && entry.type !== "assistant_stream")) {
      continue;
    }

    if (entry.streamId) {
      renderIds.add(transcriptEntryRenderId(entry));
    }
    break;
  }

  return renderIds;
}

export function transcriptEntryLane(entryType: TranscriptEntry["type"]): TranscriptLane {
  return entryType === "user" ? "human" : "assistant";
}

export function renderedTranscriptLane(entry: RenderTranscriptLaneCandidate): TranscriptLane {
  if (entry.kind === "entry") {
    return transcriptEntryLane(entry.entryType);
  }
  return "assistant";
}

function shouldCompactMarkerClusterSpacing(
  entry: RenderTranscriptEntry,
  previousEntry: RenderTranscriptEntry | undefined
): boolean {
  return (
    isMarkerRenderEntry(entry) &&
    !!previousEntry &&
    (isTextMessageRenderEntry(previousEntry) || isMarkerRenderEntry(previousEntry))
  );
}

function isMarkerRenderEntry(entry: RenderTranscriptEntry): boolean {
  return entry.kind === "memory_marker" || entry.kind === "tool_marker";
}

function isTextMessageRenderEntry(entry: RenderTranscriptEntry): boolean {
  return (
    entry.kind === "entry" &&
    (entry.entry.type === "user" || entry.entry.type === "assistant" || entry.entry.type === "assistant_stream")
  );
}

export function shouldShowTypingIndicator(entries: TranscriptEntry[], pending: boolean): boolean {
  if (!pending) {
    return false;
  }

  const lastUserIndex = latestUserEntryIndex(entries);
  if (lastUserIndex === -1) {
    return false;
  }

  return !entries
    .slice(lastUserIndex + 1)
    .some((entry) => entry.type === "assistant" || entry.type === "assistant_stream");
}

function latestUserEntryIndex(entries: TranscriptEntry[]): number {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    if (entries[index]?.type === "user") {
      return index;
    }
  }
  return -1;
}

function groupTranscriptMarkers(entries: TranscriptEntry[]): RenderTranscriptEntry[] {
  const rendered: RenderTranscriptEntry[] = [];

  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index];
    const nextEntry = entries[index + 1];
    const followingEntry = entries[index + 2];

    if (
      entry.type === "activity" &&
      entry.item.activity_kind === "memory_extraction" &&
      entry.item.status === "STARTED" &&
      nextEntry &&
      nextEntry.type === "card" &&
      nextEntry.item.schema === "memory_proposals" &&
      followingEntry &&
      followingEntry.type === "activity" &&
      followingEntry.item.activity_kind === "memory_extraction" &&
      followingEntry.item.id === entry.item.id &&
      sameTurn(entry, nextEntry) &&
      sameTurn(entry, followingEntry)
    ) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}:${followingEntry.id}`,
        source: transcriptGroupSource(entry, nextEntry, followingEntry),
        extraction: followingEntry.item,
        proposal: nextEntry.item
      });
      index += 2;
      continue;
    }

    if (
      entry.type === "activity" &&
      entry.item.activity_kind === "memory_extraction" &&
      nextEntry &&
      nextEntry.type === "card" &&
      nextEntry.item.schema === "memory_proposals" &&
      sameTurn(entry, nextEntry)
    ) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}`,
        source: transcriptGroupSource(entry, nextEntry),
        extraction: entry.item,
        proposal: nextEntry.item
      });
      index += 1;
      continue;
    }

    if (
      entry.type === "card" &&
      entry.item.schema === "memory_proposals" &&
      nextEntry &&
      nextEntry.type === "activity" &&
      nextEntry.item.activity_kind === "memory_extraction" &&
      sameTurn(entry, nextEntry)
    ) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}`,
        source: transcriptGroupSource(entry, nextEntry),
        extraction: nextEntry.item,
        proposal: entry.item
      });
      index += 1;
      continue;
    }

    if (entry.type === "activity" && entry.item.activity_kind === "memory_extraction") {
      rendered.push({
        kind: "memory_marker",
        id: entry.id,
        source: transcriptGroupSource(entry),
        extraction: entry.item
      });
      continue;
    }

    if (entry.type === "card" && entry.item.schema === "memory_proposals") {
      rendered.push({
        kind: "memory_marker",
        id: entry.id,
        source: transcriptGroupSource(entry),
        proposal: entry.item
      });
      continue;
    }

    if (entry.type === "activity" && entry.item.activity_kind === "tool_call") {
      if (
        nextEntry &&
        nextEntry.type === "activity" &&
        nextEntry.item.activity_kind === "tool_result" &&
        sameTurn(entry, nextEntry)
      ) {
        const id = entry.id;
        rendered.push({
          kind: "tool_marker",
          id,
          source: transcriptGroupSource(entry, nextEntry),
          marker: { id, call: entry, result: nextEntry },
          suppressArrival: true
        });
        index += 1;
        continue;
      }

      rendered.push({
        kind: "tool_marker",
        id: entry.id,
        source: transcriptGroupSource(entry),
        marker: { id: entry.id, call: entry }
      });
      continue;
    }

    if (entry.type === "activity" && entry.item.activity_kind === "tool_result") {
      rendered.push({
        kind: "tool_marker",
        id: entry.id,
        source: transcriptGroupSource(entry),
        marker: { id: entry.id, result: entry }
      });
      continue;
    }

    rendered.push({ kind: "entry", id: entry.id, entry });
  }

  return rendered;
}

function sameTurn(left: TranscriptEntry, right: TranscriptEntry) {
  return !left.turnId || !right.turnId || left.turnId === right.turnId;
}

function transcriptGroupSource(...entries: TranscriptEntry[]): TranscriptEntry["source"] | undefined {
  return entries.every((entry) => entry.source === "replay") ? "replay" : undefined;
}

function toolMarkerTone(marker: ToolMarkerGroup): "default" | "error" {
  return marker.result?.item.status === "FAILED" ? "error" : "default";
}

function toolMarkerPending(marker: ToolMarkerGroup): boolean {
  return marker.call?.item.status === "STARTED" && !marker.result;
}

function toolMarkerLabel(marker: ToolMarkerGroup): string {
  const toolName = toolNameFromMetadata(marker.call?.item.metadata) ?? toolNameFromMetadata(marker.result?.item.metadata);
  if (toolName) {
    if (marker.call && !marker.result && marker.call.item.status === "STARTED") {
      return `Using ${toolName}`;
    }
    return `Used ${toolName}`;
  }
  return marker.call?.item.title ?? marker.result?.item.title ?? "Tool activity";
}

function toolNameFromMetadata(metadata: unknown): string | null {
  if (!isRecord(metadata)) {
    return null;
  }

  const action = metadata.action;
  if (isRecord(action) && typeof action.name === "string" && action.name.trim()) {
    return action.name;
  }
  if (typeof metadata.name === "string" && metadata.name.trim()) {
    return metadata.name;
  }
  if (typeof metadata.tool_name === "string" && metadata.tool_name.trim()) {
    return metadata.tool_name;
  }
  return null;
}

function formatToolDetail(fallback: string, metadata: unknown): string {
  const metadataText = formatMetadata(metadata);
  if (!metadataText) {
    return fallback;
  }
  return `${fallback}\n${metadataText}`;
}

function formatMetadata(metadata: unknown): string | null {
  if (metadata === null || metadata === undefined) {
    return null;
  }
  if (typeof metadata === "string") {
    return metadata;
  }
  try {
    return JSON.stringify(metadata, null, 2);
  } catch {
    return String(metadata);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function transcriptScrollKey(entries: RenderTranscriptEntry[]): string {
  return entries.map(renderedEntryScrollFingerprint).join("|");
}

function transcriptArrivalScrollKey(
  entries: RenderTranscriptEntry[],
  seenMessageIds: ReadonlySet<string>
): string {
  return entries
    .filter((entry) => shouldAnimateRenderedEntryArrivalForSeen(entry, renderedEntryMessageId(entry), seenMessageIds))
    .map(renderedEntryMessageId)
    .join("|");
}

function initialSeenArrivalMessageIds(entries: RenderTranscriptEntry[]): ReadonlySet<string> {
  return new Set(entries.map(renderedEntryMessageId));
}

function renderedEntryScrollFingerprint(entry: RenderTranscriptEntry): string {
  if (entry.kind === "entry") {
    return transcriptEntryScrollFingerprint(entry.entry);
  }
  if (entry.kind === "typing") {
    return entry.id;
  }
  if (entry.kind === "memory_marker") {
    return [
      entry.id,
      entry.extraction?.status ?? "",
      entry.extraction?.summary ?? "",
      entry.proposal?.id ?? ""
    ].join(":");
  }
  return [
    entry.id,
    entry.marker.call?.item.status ?? "",
    entry.marker.call?.item.summary ?? "",
    entry.marker.result?.item.status ?? "",
    entry.marker.result?.item.summary ?? ""
  ].join(":");
}

function renderedEntryMessageId(entry: RenderTranscriptEntry): string {
  if (entry.kind === "entry") {
    return transcriptEntryRenderId(entry.entry);
  }
  return entry.id;
}

function transcriptEntryRenderId(entry: TranscriptEntry): string {
  if ((entry.type === "assistant" || entry.type === "assistant_stream") && entry.streamId) {
    return entry.streamId;
  }
  return entry.id;
}

function transcriptEntryScrollFingerprint(entry: TranscriptEntry): string {
  const renderId = transcriptEntryRenderId(entry);

  if (entry.type === "user" || entry.type === "assistant" || entry.type === "assistant_stream") {
    return `${renderId}:${entry.text.length}`;
  }
  if (entry.type === "activity") {
    return `${renderId}:${entry.item.status}:${entry.item.summary ?? ""}`;
  }
  if (entry.type === "card") {
    return `${renderId}:${entry.item.schema}`;
  }
  return `${renderId}:${entry.message.length}`;
}

function isScrolledToBottom(element: HTMLElement) {
  return element.scrollHeight - element.scrollTop - element.clientHeight <= 8;
}

function TranscriptBottomFollower({
  arrivalScrollKey,
  followBottomRef,
  scrollKey
}: {
  arrivalScrollKey: string;
  followBottomRef: React.MutableRefObject<boolean>;
  scrollKey: string;
}) {
  const { scrollToEnd } = useMessageScroller();

  React.useLayoutEffect(() => {
    if (followBottomRef.current) {
      scrollToEnd({ behavior: "auto" });
    }
  }, [followBottomRef, scrollKey, scrollToEnd]);

  React.useLayoutEffect(() => {
    if (!arrivalScrollKey || !followBottomRef.current) {
      return;
    }

    let animationFrame: number | null = null;
    let startedAt: number | null = null;

    const followArrival = (timestamp: number) => {
      if (!followBottomRef.current) {
        return;
      }
      if (startedAt === null) {
        startedAt = timestamp;
      }

      scrollToEnd({ behavior: "auto" });

      const elapsedMs = timestamp - startedAt;
      if (elapsedMs < ARRIVAL_SCROLL_FOLLOW_DURATION_MS) {
        animationFrame = window.requestAnimationFrame(followArrival);
      }
    };

    animationFrame = window.requestAnimationFrame(followArrival);

    return () => {
      if (animationFrame !== null) {
        window.cancelAnimationFrame(animationFrame);
      }
    };
  }, [arrivalScrollKey, followBottomRef, scrollToEnd]);

  return null;
}

function RenderedTranscriptEntryFrame({
  animateArrival,
  children
}: {
  animateArrival: boolean;
  children: React.ReactNode;
}) {
  if (!animateArrival) {
    return <>{children}</>;
  }

  return (
    <div data-slot="message-arrival-content" className="w-full max-w-[760px] min-w-0">
      <div data-slot="message-arrival-inner" className="min-h-0">
        {children}
      </div>
    </div>
  );
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

export function shouldAnimateMessageText(entry: Extract<TranscriptEntry, { text: string }>): boolean {
  return entry.type !== "user" && entry.source !== "replay";
}

function TranscriptRow({
  lane,
  showAvatar = true,
  children
}: {
  lane: TranscriptLane;
  showAvatar?: boolean;
  children: React.ReactNode;
}) {
  const actorId = lane === "human" ? LOCAL_HUMAN_AVATAR_ID : LOCAL_AGENT_AVATAR_ID;
  const actorType = lane === "human" ? "human" : "agent";

  return (
    <MessagePrimitive align={lane === "human" ? "end" : "start"} className="max-w-[760px]">
      <MessageAvatar aria-hidden={!showAvatar} className={cn(!showAvatar && "invisible")}>
        <IdentityAvatar actorId={actorId} actorType={actorType} size="sm" />
      </MessageAvatar>
      <MessageContent>{children}</MessageContent>
    </MessagePrimitive>
  );
}

function Message({
  animate,
  role,
  text,
  showAvatar
}: {
  animate: boolean;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
}) {
  return (
    <TranscriptRow lane={role === "user" ? "human" : "assistant"} showAvatar={showAvatar}>
      <Bubble variant={role === "user" ? "default" : "muted"}>
        <BubbleContent className="leading-[1.7] whitespace-pre-wrap">
          <AnimatedMessageText animate={animate} text={text} />
        </BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}

function TypingMessage({ showAvatar }: { showAvatar: boolean }) {
  return (
    <TranscriptRow lane="assistant" showAvatar={showAvatar}>
      <Bubble variant="muted">
        <BubbleContent
          className="flex min-h-9 w-[58px] items-center justify-center gap-1.5 px-3 py-2"
          aria-label="Noema is typing"
          role="status"
        >
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.24s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.12s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70" />
        </BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}

function ActivityRow({
  item,
  open,
  onToggle
}: {
  item: Extract<TurnTranscriptItem, { kind: "activity" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const isMemorySave = item.activity_kind === "memory_save";
  const title = isMemorySave ? "Memory saved" : item.title;
  const status = statusLabel(item.status);

  return (
    <Attachment className="w-full max-w-full">
      <Button
        type="button"
        variant="ghost"
        className="grid h-auto w-full grid-cols-[28px_minmax(0,1fr)_auto] items-center gap-2.5 px-[13px] py-[11px] text-left whitespace-normal text-foreground"
        onClick={onToggle}
        aria-expanded={open}
      >
        <AttachmentMedia
          className={cn(
            "size-7 font-mono text-xs font-bold text-[var(--pine-700)]",
            item.status === "FAILED" && "text-[var(--red-700)]",
            item.status === "STARTED" && "text-[var(--blue-700)]"
          )}
        >
          {isMemorySave ? "M" : "A"}
        </AttachmentMedia>
        <span>
          <AttachmentTitle>{title}</AttachmentTitle>
          {item.summary ? <AttachmentDescription>{item.summary}</AttachmentDescription> : null}
        </span>
        <em className="font-mono text-[11px] text-muted-foreground not-italic">{status}</em>
      </Button>
      {open ? (
        <AttachmentContent className="px-[13px] pb-[13px] pl-[51px] max-[760px]:pl-[13px]">
          <dl className="m-0 grid gap-[9px] border-t border-[var(--border-subtle)] pt-2.5">
            <div className="grid gap-0.5">
              <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">What happened</dt>
              <dd className="m-0 text-[13px] text-muted-foreground">{item.summary || title}</dd>
            </div>
            <div className="grid gap-0.5">
              <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">Source</dt>
              <dd className="m-0 text-[13px] text-muted-foreground">
                {isMemorySave ? "You used an explicit remember request in this chat." : readableKind(item.activity_kind)}
              </dd>
            </div>
            <div className="grid gap-0.5">
              <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">Next step</dt>
              <dd className="m-0 text-[13px] text-muted-foreground">
                {isMemorySave ? "Continue chatting. Deeper memory settings come in the next slice." : "No action needed."}
              </dd>
            </div>
          </dl>
        </AttachmentContent>
      ) : null}
    </Attachment>
  );
}

function StructuredCard({
  item,
  open,
  onToggle
}: {
  item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
  open: boolean;
  onToggle: () => void;
}) {
  if (item.schema === "memory_proposals") {
    return <MemoryMarker id={item.id} proposal={item} open={open} onToggle={onToggle} />;
  }

  const memories = memoryCardsFromStructuredItem(item);
  if (memories) {
    return <MemoryStructuredCard schema={item.schema} memories={memories} />;
  }

  return (
    <Attachment className="max-w-full">
      <AttachmentContent>
        <AttachmentTitle>{item.schema}</AttachmentTitle>
        <AttachmentDescription>Structured card placeholder</AttachmentDescription>
      </AttachmentContent>
    </Attachment>
  );
}

function MemoryMarker({
  id,
  extraction,
  proposal,
  open,
  onToggle
}: {
  id: string;
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
  proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const memories = proposal ? memoryCardsFromStructuredItem(proposal) ?? [] : memoryCardsFromClaimOutcomes(extraction);
  const failed =
    extraction?.status === "FAILED" &&
    (memories.length === 0 || metadataCount(extraction.metadata, "failed_proposal_count") > 0);
  const started = extraction?.status === "STARTED";
  const label = memoryMarkerLabel(extraction);
  const tone = failed ? "error" : started ? "default" : "success";

  return (
    <div className="grid w-full max-w-full gap-2">
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${id}-details`}
        onClick={onToggle}
        tone={tone}
        pending={started}
        className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
      >
        <MarkerIcon>
          <BrainIcon />
        </MarkerIcon>
        <MarkerContent>{label}</MarkerContent>
      </Marker>
      {open ? (
        <MemoryDetailAttachment id={`${id}-details`} extraction={extraction} memories={memories} failed={failed} />
      ) : null}
    </div>
  );
}

function memoryClaimOutcomes(metadata: unknown): MemoryClaimOutcome[] {
  if (!metadata || typeof metadata !== "object") {
    return [];
  }
  const outcomes = (metadata as { claim_outcomes?: unknown }).claim_outcomes;
  if (!Array.isArray(outcomes)) {
    return [];
  }
  return outcomes.flatMap((outcome): MemoryClaimOutcome[] => {
    if (!outcome || typeof outcome !== "object") {
      return [];
    }
    const record = outcome as Record<string, unknown>;
    const claimId = typeof record.claim_id === "string" ? record.claim_id : "";
    const rawOutcome = record.outcome;
    if (!claimId || (rawOutcome !== "created" && rawOutcome !== "reinforced")) {
      return [];
    }
    return [
      {
        claimId,
        outcome: rawOutcome,
        factPreview: typeof record.fact_preview === "string" ? record.fact_preview : undefined,
        sensitivity: typeof record.sensitivity === "string" ? record.sensitivity : undefined
      }
    ];
  });
}

function metadataCount(metadata: unknown, key: string): number {
  if (!metadata || typeof metadata !== "object") {
    return 0;
  }
  const value = (metadata as Record<string, unknown>)[key];
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0;
}

function memoryMarkerLabel(extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>): string {
  if (!extraction) {
    return "Memory updated";
  }
  if (extraction.status === "STARTED") {
    return "Memory proposed";
  }

  const outcomes = memoryClaimOutcomes(extraction.metadata);
  const failedCount = metadataCount(extraction.metadata, "failed_proposal_count");
  if (outcomes.length === 0) {
    return extraction.status === "FAILED" ? "Memory update failed" : "Memory updated";
  }
  if (outcomes.length === 1 && failedCount === 0) {
    const outcome = outcomes[0];
    const verb = outcome.outcome === "reinforced" ? "Memory updated" : "Memory saved";
    return outcome.factPreview ? `${verb}: ${outcome.factPreview}` : verb;
  }

  const createdCount = metadataCount(extraction.metadata, "created_claim_count");
  const reinforcedCount = metadataCount(extraction.metadata, "reinforced_claim_count");
  const savedCount = createdCount + reinforcedCount || outcomes.length;
  const noun = savedCount === 1 ? "memory" : "memories";
  const prefix = createdCount > 0 ? "Memory saved" : "Memory updated";
  const failureSuffix = failedCount > 0 ? `; ${failedCount} failed` : "";
  return `${prefix}: ${savedCount} ${noun}${failureSuffix}`;
}

function memoryCardsFromClaimOutcomes(
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>
): MemoryCardData[] {
  return memoryClaimOutcomes(extraction?.metadata).map((outcome) => ({
    id: outcome.claimId,
    title: outcome.factPreview ?? outcome.claimId,
    content: outcome.factPreview ?? outcome.claimId,
    status: outcome.outcome,
    sensitivity: outcome.sensitivity
  }));
}

function ToolMarker({
  marker,
  open,
  onToggle
}: {
  marker: ToolMarkerGroup;
  open: boolean;
  onToggle: () => void;
}) {
  const tone = toolMarkerTone(marker);
  const pending = toolMarkerPending(marker);

  return (
    <div className="grid w-full max-w-full gap-2">
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${marker.id}-details`}
        onClick={onToggle}
        tone={tone}
        pending={pending}
        className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
      >
        <MarkerIcon>
          <WrenchIcon />
        </MarkerIcon>
        <MarkerContent>{toolMarkerLabel(marker)}</MarkerContent>
      </Marker>
      {open ? <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} /> : null}
    </div>
  );
}

function ToolDetailAttachment({ id, marker }: { id: string; marker: ToolMarkerGroup }) {
  const result = marker.result?.item;
  const call = marker.call?.item;
  const failed = result?.status === "FAILED";
  const title = toolMarkerLabel(marker);
  const description = result?.summary ?? call?.summary ?? statusLabel(result?.status ?? call?.status ?? "COMPLETED");

  return (
    <Attachment id={id} state={failed ? "error" : "done"} className="max-w-full">
      <AttachmentMedia className={failed ? "text-[var(--red-700)]" : "text-[var(--blue-700)]"}>
        <WrenchIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{description}</AttachmentDescription>
        <dl className="mt-3 grid gap-2 border-t border-[var(--border-subtle)] pt-2.5">
          {call ? <ToolDetailRow label="Call" value={formatToolDetail(call.title, call.metadata)} /> : null}
          {result ? (
            <ToolDetailRow label="Result" value={formatToolDetail(result.summary ?? result.title, result.metadata)} />
          ) : null}
          <ToolDetailRow label="Status" value={statusLabel(result?.status ?? call?.status ?? "COMPLETED")} />
        </dl>
      </AttachmentContent>
    </Attachment>
  );
}

function ToolDetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-0.5">
      <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">{label}</dt>
      <dd className="m-0 max-h-40 overflow-auto whitespace-pre-wrap break-words text-[13px] text-muted-foreground">
        {value}
      </dd>
    </div>
  );
}

function MemoryStructuredCard({ schema, memories }: { schema: string; memories: MemoryCardData[] }) {
  const count = memories.length;
  const title = count === 1 ? "Memory saved" : `${count} memories saved`;
  const source = schema === "memory_proposals" ? "Same-call proposal" : "Explicit request";

  return (
    <Attachment className="max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{source}</AttachmentDescription>
        <MemoryDetailList memories={memories} />
      </AttachmentContent>
    </Attachment>
  );
}

export function memoryDetailItems(memories: MemoryCardData[]): MemoryDetailItem[] {
  return memories.map((memory) => {
    const rows: MemoryDetailRowData[] = [{ label: "Memory", value: memory.content }];
    if (memory.memoryType) {
      rows.push({ label: "Type", value: memory.memoryType });
    }
    if (memory.sensitivity) {
      rows.push({ label: "Sensitivity", value: memory.sensitivity });
    }
    if (memory.status) {
      rows.push({ label: "Status", value: memory.status });
    }
    if (typeof memory.confidence === "number") {
      rows.push({ label: "Confidence", value: `${Math.round(memory.confidence * 100)}%` });
    }
    if (memory.evidenceExcerpt) {
      rows.push({ label: "Evidence", value: memory.evidenceExcerpt });
    }
    if (memory.id) {
      rows.push({ label: "Memory ID", value: memory.id });
    }
    return { title: memory.title, rows };
  });
}

function MemoryDetailAttachment({
  id,
  extraction,
  memories,
  failed
}: {
  id: string;
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
  memories: MemoryCardData[];
  failed: boolean;
}) {
  const memoryCount = memories.length;
  const title = failed && memoryCount === 0 ? "Memory update failed" : memoryDetailTitle(extraction, memoryCount);
  const status = extraction ? statusLabel(extraction.status) : null;
  const description = extraction?.summary ?? (status ? `Memory extraction ${status.toLowerCase()}` : "Memory proposal");

  return (
    <Attachment id={id} state={failed ? "error" : "done"} className="max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{description}</AttachmentDescription>
        <MemoryDetailList memories={memories} />
      </AttachmentContent>
    </Attachment>
  );
}

function memoryDetailTitle(
  extraction: Extract<TurnTranscriptItem, { kind: "activity" }> | undefined,
  memoryCount: number
): string {
  if (extraction && memoryCount > 0 && metadataCount(extraction.metadata, "failed_proposal_count") > 0) {
    return memoryMarkerLabel(extraction);
  }
  return memoryCount === 1 ? "Memory saved" : `${memoryCount} memories saved`;
}

function MemoryDetailList({ memories }: { memories: MemoryCardData[] }) {
  const items = memoryDetailItems(memories);
  if (items.length === 0) {
    return null;
  }

  return (
    <div className="mt-3 grid gap-3 border-t border-[var(--border-subtle)] pt-2.5">
      {items.map((item, index) => (
        <section className="grid gap-2" key={`${item.title}:${index}`}>
          {items.length > 1 ? (
            <p className="m-0 text-[13px] font-medium text-foreground [overflow-wrap:anywhere]">{item.title}</p>
          ) : null}
          <dl className="grid gap-2">
            {item.rows.map((row) => (
              <MemoryDetailRow key={row.label} label={row.label} value={row.value} />
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}

function MemoryDetailRow({ label, value }: MemoryDetailRowData) {
  return (
    <div className="grid gap-0.5">
      <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">{label}</dt>
      <dd className="m-0 max-h-40 overflow-auto whitespace-pre-wrap break-words text-[13px] text-muted-foreground">
        {value}
      </dd>
    </div>
  );
}

function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return <ErrorMarker message={message} label={recoverable ? "Notice" : "Error"} recoverable={recoverable} />;
}
