import type { ConversationAgentStatus, TranscriptEntry, TurnTranscriptItem } from "../../types";

type ActivityTranscriptItem = Extract<TurnTranscriptItem, { kind: "activity" }>;
type ActivityTranscriptEntry = Extract<TranscriptEntry, { type: "activity" }>;

export type ToolMarkerGroup = {
  id: string;
  call?: ActivityTranscriptEntry;
  result?: ActivityTranscriptEntry;
};

export type RenderTranscriptEntry =
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

export type TranscriptLane = "human" | "assistant";

type TranscriptEntryAnchorCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" }
  | { kind: "tool_marker" };

type RenderTranscriptLaneCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" }
  | { kind: "tool_marker" };

export function renderableTranscriptEntries(
  entries: TranscriptEntry[],
  pending: boolean,
  agentStatus: ConversationAgentStatus
): RenderTranscriptEntry[] {
  const typingContinuationRenderIds = typingContinuationAssistantRenderIds(entries);
  const renderedEntries = groupTranscriptMarkers(entries).map((entry): RenderTranscriptEntry => {
    if (entry.kind !== "entry" || !typingContinuationRenderIds.has(transcriptEntryRenderId(entry.entry))) {
      return entry;
    }
    return { ...entry, suppressArrival: true };
  });
  if (shouldShowTypingIndicator(entries, pending, agentStatus)) {
    renderedEntries.push({ kind: "typing", id: "typing-indicator" });
  }
  return renderedEntries;
}

export function shouldAnchorTranscriptEntry(entry: TranscriptEntryAnchorCandidate): boolean {
  switch (entry.kind) {
    case "entry":
    case "typing":
    case "memory_marker":
    case "tool_marker":
      return false;
  }
}

export function shouldAnchorRenderedEntry(entry: RenderTranscriptEntry): boolean {
  if (entry.kind === "entry") {
    return shouldAnchorTranscriptEntry({ kind: "entry", entryType: entry.entry.type });
  }
  return shouldAnchorTranscriptEntry({ kind: entry.kind });
}

export function shouldAnimateRenderedEntryArrivalForSeen(
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

export function shouldRevealRenderedEntryAfterArrival(
  entry: RenderTranscriptEntry,
  previousEntryAnimateArrival: boolean
): boolean {
  return entry.kind === "typing" && previousEntryAnimateArrival;
}

export function shouldAnimateRenderedEntryText(entry: RenderTranscriptEntry): boolean {
  return entry.kind === "entry" && isTextTranscriptEntry(entry.entry) && shouldAnimateMessageText(entry.entry);
}

export function shouldAnimateMessageText(entry: Extract<TranscriptEntry, { text: string }>): boolean {
  return entry.type !== "user" && entry.source !== "replay";
}

export function shouldAnimateRenderedEntryTextForSeen(
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

export function transcriptEntryLane(entryType: TranscriptEntry["type"]): TranscriptLane {
  return entryType === "user" ? "human" : "assistant";
}

export function renderedTranscriptLane(entry: RenderTranscriptLaneCandidate): TranscriptLane {
  if (entry.kind === "entry") {
    return transcriptEntryLane(entry.entryType);
  }
  return "assistant";
}

export function shouldCompactMarkerClusterSpacing(
  entry: RenderTranscriptEntry,
  previousEntry: RenderTranscriptEntry | undefined
): boolean {
  return (
    isMarkerRenderEntry(entry) &&
    !!previousEntry &&
    (isTextMessageRenderEntry(previousEntry) || isMarkerRenderEntry(previousEntry))
  );
}

export function shouldShowTypingIndicator(
  entries: TranscriptEntry[],
  pending: boolean,
  agentStatus: ConversationAgentStatus
): boolean {
  const lastUserIndex = latestUserEntryIndex(entries);
  if (lastUserIndex === -1) {
    return false;
  }

  const hasAssistantAfterLastUser = entries
    .slice(lastUserIndex + 1)
    .some((entry) => entry.type === "assistant" || entry.type === "assistant_stream");
  if (hasAssistantAfterLastUser) {
    return false;
  }

  if (pending || agentStatus === "THINKING" || agentStatus === "TOOL_RUNNING") {
    return true;
  }

  return entries.slice(lastUserIndex + 1).some((entry) => entry.type === "activity");
}

function sameTurn(left: TranscriptEntry, right: TranscriptEntry) {
  return !left.turnId || !right.turnId || left.turnId === right.turnId;
}

function transcriptGroupSource(...entries: TranscriptEntry[]): TranscriptEntry["source"] | undefined {
  return entries.every((entry) => entry.source === "replay") ? "replay" : undefined;
}

export function renderedEntryMessageId(entry: RenderTranscriptEntry): string {
  if (entry.kind === "entry") {
    return transcriptEntryRenderId(entry.entry);
  }
  return entry.id;
}

export function transcriptEntryRenderId(entry: TranscriptEntry): string {
  if ((entry.type === "assistant" || entry.type === "assistant_stream") && entry.streamId) {
    return entry.streamId;
  }
  return entry.id;
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

function isMarkerRenderEntry(entry: RenderTranscriptEntry): boolean {
  return entry.kind === "memory_marker" || entry.kind === "tool_marker";
}

function isTextMessageRenderEntry(entry: RenderTranscriptEntry): boolean {
  return (
    entry.kind === "entry" &&
    (entry.entry.type === "user" || entry.entry.type === "assistant" || entry.entry.type === "assistant_stream")
  );
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
