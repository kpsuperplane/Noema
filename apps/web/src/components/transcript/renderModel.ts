import type { ConversationAgentStatus, TranscriptEntry } from "@/shared/types";

type ActivityTranscriptEntry = Extract<TranscriptEntry, { type: "activity" }>;

export type ToolMarkerGroup = {
  id: string;
  call?: ActivityTranscriptEntry;
  result?: ActivityTranscriptEntry;
};

export type ToolMarkerCluster = {
  id: string;
  markers: ToolMarkerGroup[];
  source?: TranscriptEntry["source"];
  suppressArrival?: boolean;
};

export type RenderTranscriptEntry =
  | { kind: "entry"; id: string; entry: TranscriptEntry; suppressArrival?: boolean }
  | { kind: "typing"; id: string }
  | {
      kind: "tool_marker";
      id: string;
      source?: TranscriptEntry["source"];
      marker: ToolMarkerGroup;
      suppressArrival?: boolean;
    }
  | ({ kind: "tool_marker_group" } & ToolMarkerCluster);

export type TranscriptLane = "human" | "assistant";
export type ChatBubbleGroup = "first" | "middle" | "last";

type TranscriptEntryAnchorCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "tool_marker" }
  | { kind: "tool_marker_group" };

type RenderTranscriptLaneCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "tool_marker" }
  | { kind: "tool_marker_group" };

export function renderableTranscriptEntries(
  entries: TranscriptEntry[],
  pending: boolean,
  agentStatus: ConversationAgentStatus,
  collapseConsecutiveToolCalls = false
): RenderTranscriptEntry[] {
  const visibleEntries = attachWorkNotificationTasks(entries);
  const renderedEntries = collapseConsecutiveToolCalls
    ? collapseConsecutiveToolMarkers(groupTranscriptMarkers(visibleEntries))
    : groupTranscriptMarkers(visibleEntries);
  if (shouldShowTypingIndicator(entries, pending, agentStatus)) {
    renderedEntries.push({ kind: "typing", id: "typing-indicator" });
  }
  return renderedEntries;
}

function attachWorkNotificationTasks(entries: TranscriptEntry[]): TranscriptEntry[] {
  const references = new Map<string, Extract<TranscriptEntry, { type: "task" }>>();
  const messageIndexes = new Map<string, number>();

  entries.forEach((entry, index) => {
    const notificationId = workNotificationId(entry);
    if (!notificationId) return;
    if (entry.type === "task" && metadataString(entry.metadata, "notification_kind") !== "task_created") {
      references.set(notificationId, entry);
    } else if (
      entry.type === "assistant" &&
      metadataString(entry.metadata, "source") === "work_notification"
    ) {
      messageIndexes.set(notificationId, index);
    }
  });
  const notificationEntries = entries.flatMap((entry, index) => {
    const notificationId = workNotificationId(entry);
    if (
      entry.type === "task" &&
      notificationId &&
      metadataString(entry.metadata, "notification_kind") !== "task_created"
    ) {
      return [];
    }
    const reference = notificationId ? references.get(notificationId) : undefined;
    return entry.type === "assistant" && notificationId && reference && messageIndexes.get(notificationId) === index
      ? [{ ...entry, taskReferences: [reference.item] }]
      : [entry];
  });

  return notificationEntries.reduce<TranscriptEntry[]>((attached, entry) => {
    if (
      entry.type === "task" &&
      entry.turnId &&
      metadataString(entry.metadata, "notification_kind") === "task_created"
    ) {
      const previous = attached.at(-1);
      if (previous?.type === "assistant" && previous.turnId === entry.turnId) {
        attached[attached.length - 1] = {
          ...previous,
          taskReferences: [...(previous.taskReferences ?? []), entry.item]
        };
        return attached;
      }
    }
    attached.push(entry);
    return attached;
  }, []);
}

function workNotificationId(entry: TranscriptEntry): string | undefined {
  return entry.type === "assistant" || entry.type === "task"
    ? metadataString(entry.metadata, "notification_id")
    : undefined;
}

function metadataString(metadata: unknown, key: string): string | undefined {
  if (!metadata || typeof metadata !== "object" || Array.isArray(metadata)) return undefined;
  const value = (metadata as Record<string, unknown>)[key];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

export function shouldAnchorTranscriptEntry(entry: TranscriptEntryAnchorCandidate): boolean {
  switch (entry.kind) {
    case "entry":
    case "typing":
    case "tool_marker":
    case "tool_marker_group":
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

export function shouldContinueRenderedEntryTextAnimation(entry: RenderTranscriptEntry): boolean {
  return (
    entry.kind === "entry" &&
    entry.entry.type === "assistant_stream" &&
    shouldAnimateMessageText(entry.entry)
  );
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
  if (entry.kind !== "entry" || !isTextTranscriptEntry(entry.entry) || !shouldAnimateMessageText(entry.entry)) {
    return false;
  }
  if (entry.entry.type === "assistant_stream") {
    return !seenMessageIds.has(messageId) || textAnimatingMessageIds.has(messageId);
  }
  if (entry.entry.type === "assistant" && textAnimatingMessageIds.has(messageId)) {
    return false;
  }
  return !seenMessageIds.has(messageId);
}

export function transcriptEntryLane(entryType: TranscriptEntry["type"]): TranscriptLane {
  return entryType === "user" || entryType === "input" || entryType === "multiple_choice_selection"
    ? "human"
    : "assistant";
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

export function renderedChatBubbleGroup(
  entry: RenderTranscriptEntry,
  previousEntry: RenderTranscriptEntry | undefined,
  nextEntry: RenderTranscriptEntry | undefined
): ChatBubbleGroup | undefined {
  if (!isGroupableChatBubbleRenderEntry(entry)) {
    return undefined;
  }
  const hasPrevious = isAdjacentChatBubble(previousEntry, entry);
  const hasNext = isAdjacentChatBubble(nextEntry, entry);
  if (hasPrevious && hasNext) {
    return "middle";
  }
  if (hasNext) {
    return "first";
  }
  if (hasPrevious) {
    return "last";
  }
  return undefined;
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

  return pending || isActiveAgentStatus(agentStatus);
}

function isActiveAgentStatus(agentStatus: ConversationAgentStatus): boolean {
  switch (agentStatus) {
    case "INPUT_RECEIVED":
    case "THINKING":
    case "TOOL_RUNNING":
    case "WAITING_FOR_PREVIOUS_TURN_COMPLETION":
    case "INTERRUPTING":
      return true;
    case "IDLE":
    case "ERROR":
    case "connecting":
    case "closed":
      return false;
  }
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
  if ((entry.type === "assistant_stream" || entry.type === "assistant") && entry.streamId) {
    return entry.streamId;
  }
  return entry.id;
}

function shouldAnimateRenderedEntryArrival(entry: RenderTranscriptEntry): boolean {
  if (entry.kind === "entry" && entry.suppressArrival) {
    return false;
  }
  if ((entry.kind === "tool_marker" || entry.kind === "tool_marker_group") && entry.suppressArrival) {
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

function isMarkerRenderEntry(entry: RenderTranscriptEntry): boolean {
  return entry.kind === "tool_marker" || entry.kind === "tool_marker_group";
}

function isTextMessageRenderEntry(entry: RenderTranscriptEntry): boolean {
  return (
    entry.kind === "entry" &&
    (entry.entry.type === "user" ||
      entry.entry.type === "input" ||
      entry.entry.type === "assistant" ||
      entry.entry.type === "assistant_stream")
  );
}

function isChatBubbleRenderEntry(entry: RenderTranscriptEntry): boolean {
  return (
    isTextMessageRenderEntry(entry) ||
    (entry.kind === "entry" &&
      (entry.entry.type === "multiple_choice_prompt" || entry.entry.type === "multiple_choice_selection"))
  );
}

function isGroupableChatBubbleRenderEntry(
  entry: RenderTranscriptEntry | undefined
): entry is Extract<RenderTranscriptEntry, { kind: "entry" }> {
  return !!entry && isChatBubbleRenderEntry(entry);
}

function isAdjacentChatBubble(
  candidate: RenderTranscriptEntry | undefined,
  entry: Extract<RenderTranscriptEntry, { kind: "entry" }>
): boolean {
  if (!isGroupableChatBubbleRenderEntry(candidate)) {
    return false;
  }
  if (transcriptEntryLane(candidate.entry.type) !== transcriptEntryLane(entry.entry.type)) {
    return false;
  }
  return sameConcreteTurn(candidate.entry, entry.entry);
}

function sameConcreteTurn(left: TranscriptEntry, right: TranscriptEntry): boolean {
  return !!left.turnId && left.turnId === right.turnId;
}

function latestUserEntryIndex(entries: TranscriptEntry[]): number {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    if (entries[index]?.type === "user") {
      return index;
    }
    if (entries[index]?.type === "multiple_choice_selection") {
      return index;
    }
  }
  return -1;
}

function groupTranscriptMarkers(entries: TranscriptEntry[]): RenderTranscriptEntry[] {
  const rendered: RenderTranscriptEntry[] = [];
  const pendingToolMarkers = new Map<string, Extract<RenderTranscriptEntry, { kind: "tool_marker" }>>();

  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index];
    const nextEntry = entries[index + 1];

    if (entry.type === "activity" && entry.item.activity_kind === "tool_call") {
      const correlationId = toolActivityCorrelationId(entry);
      if (
        nextEntry &&
        nextEntry.type === "activity" &&
        nextEntry.item.activity_kind === "tool_result" &&
        sameTurn(entry, nextEntry) &&
        (!correlationId || correlationId === toolActivityCorrelationId(nextEntry))
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

      const marker: Extract<RenderTranscriptEntry, { kind: "tool_marker" }> = {
        kind: "tool_marker",
        id: entry.id,
        source: transcriptGroupSource(entry),
        marker: { id: entry.id, call: entry }
      };
      rendered.push(marker);
      if (correlationId) {
        pendingToolMarkers.set(correlationId, marker);
      }
      continue;
    }

    if (entry.type === "activity" && entry.item.activity_kind === "tool_result") {
      const correlationId = toolActivityCorrelationId(entry);
      const pendingMarker = correlationId ? pendingToolMarkers.get(correlationId) : undefined;
      const pendingCall = pendingMarker?.marker.call;
      if (correlationId && pendingMarker && pendingCall && sameTurn(pendingCall, entry)) {
        pendingMarker.marker.result = entry;
        pendingMarker.source = transcriptGroupSource(pendingCall, entry);
        pendingToolMarkers.delete(correlationId);
        continue;
      }

      rendered.push({
        kind: "tool_marker",
        id: entry.id,
        source: transcriptGroupSource(entry),
        marker: { id: entry.id, result: entry }
      });
      continue;
    }

    rendered.push({
      kind: "entry",
      id: entry.id,
      entry,
      suppressArrival: entry.type === "assistant_stream"
    });
  }

  return rendered;
}

function collapseConsecutiveToolMarkers(entries: RenderTranscriptEntry[]): RenderTranscriptEntry[] {
  const collapsed: RenderTranscriptEntry[] = [];

  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index];
    if (entry.kind !== "tool_marker") {
      collapsed.push(entry);
      continue;
    }

    const markers = [entry];
    while (true) {
      const next = entries[index + 1];
      if (next?.kind !== "tool_marker" || !sameToolMarkerOwner(entry.marker, next.marker)) {
        break;
      }
      index += 1;
      markers.push(next);
    }

    collapsed.push({
      kind: "tool_marker_group",
      id: markers[0].id,
      source: markers.every((marker) => marker.source === "replay") ? "replay" : undefined,
      markers: markers.map((marker) => marker.marker),
      suppressArrival: markers.every((marker) => marker.suppressArrival)
    });
  }

  return collapsed;
}

function sameToolMarkerOwner(left: ToolMarkerGroup, right: ToolMarkerGroup): boolean {
  const leftEntry = left.call ?? left.result;
  const rightEntry = right.call ?? right.result;
  if (!leftEntry?.turnId || !rightEntry?.turnId || leftEntry.turnId !== rightEntry.turnId) {
    return false;
  }

  return toolMarkerAgentKey(left) === toolMarkerAgentKey(right);
}

function toolMarkerAgentKey(marker: ToolMarkerGroup): string | undefined {
  const metadata = (marker.call ?? marker.result)?.item.metadata;
  if (!isRecord(metadata)) {
    return undefined;
  }

  for (const key of ["agent_id", "agentId", "instance_name", "instanceName"]) {
    const value = stringValue(metadata[key]);
    if (value) {
      return value;
    }
  }
  return undefined;
}

function toolActivityCorrelationId(entry: ActivityTranscriptEntry): string | undefined {
  const action = recordValue(entry.item.metadata)?.action;
  if (!isRecord(action)) {
    return undefined;
  }
  const id = stringValue(action.id) ?? stringValue(action.call_id);
  if (!id) {
    return undefined;
  }
  return [entry.turnId ?? "", id].join(":");
}

function recordValue(value: unknown): Record<string, unknown> | null {
  return isRecord(value) ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}
