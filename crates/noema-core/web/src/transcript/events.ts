import type {
  ConversationEventsSubscription,
  ConversationTranscriptPageQuery
} from "@/generated/graphql";
import type { TranscriptEntry, TurnTranscriptItem } from "@/shared/types";

type ReplayItem = ConversationTranscriptPageQuery["conversationTranscriptPage"]["items"][number];
export type ConversationEvent = ConversationEventsSubscription["conversationEvents"];
type GraphqlTranscriptItem = ReplayItem["item"];

export function entriesFromReplay(items: ReplayItem[]): TranscriptEntry[] {
  return items.map(entryFromReplayItem).filter((entry) => entry !== null);
}

export function entryFromConversationEvent(event: ConversationEvent): TranscriptEntry | null {
  if (event.__typename !== "ConversationItemEvent") {
    return null;
  }

  markClientTurnEvent("client_conversation_event_received", clientEventFields(event));
  const entry = entryFromConversationItem(
    event.itemId,
    event.cursor,
    event.turnId ?? undefined,
    event.item,
    event.metadata
  );
  if (entry) {
    markClientTurnEvent("client_transcript_entry_scheduled", {
      ...clientEventFields(event),
      entry_type: entry.type
    });
  }
  return entry;
}

export function isTurnCompletedEvent(event: ConversationEvent): boolean {
  if (event.__typename === "TurnCompletedEvent") {
    markClientTurnEvent("client_conversation_event_received", clientEventFields(event));
    markClientTurnEvent("client_turn_completed_scheduled", clientEventFields(event));
    return true;
  }
  return false;
}

export function isAgentStatusEvent(
  event: ConversationEvent
): event is Extract<ConversationEvent, { __typename: "AgentStatusEvent" }> {
  if (event.__typename === "AgentStatusEvent") {
    markClientTurnEvent("client_conversation_event_received", clientEventFields(event));
    markClientTurnEvent("client_agent_status_scheduled", clientEventFields(event));
    return true;
  }
  return false;
}

export function isAssistantTextDeltaEvent(
  event: ConversationEvent
): event is Extract<ConversationEvent, { __typename: "AssistantTextDeltaEvent" }> {
  if (event.__typename === "AssistantTextDeltaEvent") {
    markClientTurnEvent("client_conversation_event_received", clientEventFields(event));
    markClientTurnEvent("client_assistant_delta_scheduled", clientEventFields(event));
    return true;
  }
  return false;
}

export function removeStaleStartedMemoryExtractions(current: TranscriptEntry[]): TranscriptEntry[] {
  return current.filter(
    (entry) =>
      !(
        entry.type === "activity" &&
        entry.item.activity_kind === "memory_extraction" &&
        entry.item.status === "STARTED"
      )
  );
}

export function appendAssistantTextDeltaEntry(
  current: TranscriptEntry[],
  event: { turnId: string; streamId: string; delta: string; conversationId: string }
): TranscriptEntry[] {
  void event.conversationId;
  const completedIndex = current.findIndex((candidate) => {
    if (candidate.type !== "assistant") {
      return false;
    }
    if (candidate.streamId === event.streamId) {
      return true;
    }
    return candidate.turnId === event.turnId;
  });
  if (completedIndex !== -1) {
    return current;
  }

  const index = current.findIndex(
    (candidate) => candidate.type === "assistant_stream" && candidate.streamId === event.streamId
  );
  if (index === -1) {
    return [
      ...current,
      {
        id: event.streamId,
        turnId: event.turnId,
        type: "assistant_stream",
        streamId: event.streamId,
        text: event.delta
      }
    ];
  }
  return current.map((candidate, candidateIndex) =>
    candidateIndex === index && candidate.type === "assistant_stream"
      ? { ...candidate, text: `${candidate.text}${event.delta}` }
      : candidate
  );
}

function entryFromReplayItem(item: ReplayItem): TranscriptEntry | null {
  const entry = entryFromConversationItem(
    item.itemId,
    item.cursor,
    item.turnId ?? undefined,
    item.item
  );
  return entry ? { ...entry, source: "replay" } : null;
}

function entryFromConversationItem(
  itemId: string,
  cursor: string | null | undefined,
  turnId: string | undefined,
  item: GraphqlTranscriptItem,
  metadata?: unknown
): TranscriptEntry | null {
  const transcriptItem = transcriptItemFromGraphql(item);
  if (!transcriptItem) {
    return null;
  }
  if (transcriptItem.kind === "user_text") {
    return { id: itemId, itemId, cursor, turnId, type: "user", text: transcriptItem.text };
  }
  if (transcriptItem.kind === "assistant_text") {
    return {
      id: itemId,
      itemId,
      cursor,
      turnId,
      type: "assistant",
      streamId: streamIdFromMetadata(metadata),
      text: transcriptItem.text
    };
  }
  if (transcriptItem.kind === "activity") {
    return { id: itemId, itemId, cursor, turnId, type: "activity", item: transcriptItem };
  }
  if (transcriptItem.kind === "a2ui_card") {
    return { id: itemId, itemId, cursor, turnId, type: "card", item: transcriptItem };
  }
  if (transcriptItem.kind === "error_notice") {
    return {
      id: itemId,
      itemId,
      cursor,
      turnId,
      type: "error",
      message: transcriptItem.message,
      recoverable: transcriptItem.recoverable
    };
  }
  return null;
}

function streamIdFromMetadata(metadata: unknown): string | undefined {
  if (!isRecord(metadata)) {
    return undefined;
  }
  return typeof metadata.stream_id === "string" ? metadata.stream_id : undefined;
}

function markClientTurnEvent(event: string, fields: Record<string, unknown>) {
  if (!clientTurnTimingEnabled()) {
    return;
  }
  const payload = {
    category: "turn_timing_client",
    event,
    unix_ms: Date.now(),
    ...fields
  };
  const globalScope = globalThis as typeof globalThis & {
    __NOEMA_TURN_TIMINGS__?: Array<Record<string, unknown>>;
  };
  globalScope.__NOEMA_TURN_TIMINGS__ = globalScope.__NOEMA_TURN_TIMINGS__ ?? [];
  globalScope.__NOEMA_TURN_TIMINGS__.push(payload);
  console.debug("[noema_turn_timing_client]", JSON.stringify(payload));
}

function clientTurnTimingEnabled(): boolean {
  const explicitValue = browserTurnTimingFlagValue();
  if (explicitValue === null) {
    return false;
  }
  return timingFlagEnabled(explicitValue);
}

function browserTurnTimingFlagValue(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  const paramsValue = new URLSearchParams(window.location.search).get("noema_turn_timing");
  return paramsValue ?? window.localStorage.getItem("NOEMA_TURN_TIMING");
}

function timingFlagEnabled(value: string): boolean {
  return ["1", "true", "yes", "on"].includes(value.trim().toLowerCase());
}

function clientEventFields(event: ConversationEvent): Record<string, unknown> {
  if (event.__typename === "ConversationItemEvent") {
    return {
      graphql_event: event.__typename,
      conversation_id: event.conversationId,
      client_message_id: event.clientMessageId,
      item_id: event.itemId,
      turn_id: event.turnId,
      item_kind: event.item.__typename
    };
  }
  if (event.__typename === "AssistantTextDeltaEvent") {
    return {
      graphql_event: event.__typename,
      conversation_id: event.conversationId,
      turn_id: event.deltaTurnId,
      stream_id: event.streamId,
      delta_chars: event.delta.length
    };
  }
  if (event.__typename === "AgentStatusEvent") {
    return {
      graphql_event: event.__typename,
      conversation_id: event.conversationId,
      status: event.status
    };
  }
  if (event.__typename === "TurnCompletedEvent") {
    return {
      graphql_event: event.__typename,
      conversation_id: event.conversationId,
      client_message_id: event.clientMessageId
    };
  }
  return {
    graphql_event: event.__typename,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function transcriptItemFromGraphql(item: GraphqlTranscriptItem): TurnTranscriptItem | null {
  if (item.__typename === "UserText") {
    return { kind: "user_text", text: item.text };
  }
  if (item.__typename === "AssistantText") {
    return { kind: "assistant_text", text: item.text };
  }
  if (item.__typename === "Activity") {
    return {
      kind: "activity",
      id: item.id,
      activity_kind: item.activityKind,
      status: item.status,
      title: item.title,
      summary: item.summary,
      metadata: item.metadata
    };
  }
  if (item.__typename === "A2UiCard") {
    return { kind: "a2ui_card", id: item.id, schema: item.schema, payload: item.payload };
  }
  if (item.__typename === "ErrorNotice") {
    return { kind: "error_notice", message: item.message, recoverable: item.recoverable };
  }
  return null;
}
