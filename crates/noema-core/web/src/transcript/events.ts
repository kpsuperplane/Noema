import type { Dispatch, SetStateAction } from "react";
import type {
  ConversationEventsSubscription,
  StartPrimaryConversationMutation
} from "@/generated/graphql";
import type { ConversationAgentStatus, TranscriptEntry, TurnTranscriptItem } from "@/shared/types";

type ReplayItem = StartPrimaryConversationMutation["startPrimaryConversation"]["replay"][number];
type ConversationEvent = ConversationEventsSubscription["conversationEvents"];
type GraphqlTranscriptItem = ReplayItem["item"];

export function entriesFromReplay(items: ReplayItem[]): TranscriptEntry[] {
  return items.map(entryFromReplayItem).filter((entry) => entry !== null);
}

export function handleConversationEvent(
  event: ConversationEvent,
  setters: {
    setTranscript: Dispatch<SetStateAction<TranscriptEntry[]>>;
    setPending: Dispatch<SetStateAction<boolean>>;
    setAgentStatus: Dispatch<SetStateAction<ConversationAgentStatus>>;
    setAwaitingAssistantTurn: Dispatch<SetStateAction<boolean>>;
  }
) {
  markClientTurnEvent("client_conversation_event_received", clientEventFields(event));
  if (event.__typename === "TurnCompletedEvent") {
    setters.setTranscript((current) => removeStaleStartedMemoryExtractions(current));
    markClientTurnEvent("client_turn_completed_scheduled", clientEventFields(event));
    setters.setPending(false);
    setters.setAwaitingAssistantTurn(false);
    setters.setAgentStatus("IDLE");
    return;
  }
  if (event.__typename === "AgentStatusEvent") {
    setters.setAgentStatus(event.status);
    markClientTurnEvent("client_agent_status_scheduled", clientEventFields(event));
    return;
  }
  if (event.__typename === "AssistantTextDeltaEvent") {
    setters.setAwaitingAssistantTurn(false);
    appendAssistantTextDelta(setters.setTranscript, {
      conversationId: event.conversationId,
      turnId: event.deltaTurnId,
      streamId: event.streamId,
      delta: event.delta
    });
    markClientTurnEvent("client_assistant_delta_scheduled", clientEventFields(event));
    return;
  }
  if (event.__typename !== "ConversationItemEvent") {
    return;
  }

  const entry = entryFromConversationItem(event.itemId, event.turnId ?? undefined, event.item, event.metadata);
  if (!entry) {
    return;
  }
  if (event.clientMessageId && entry.type === "user") {
    replaceOptimisticTranscriptEntry(setters.setTranscript, event.clientMessageId, entry);
  } else {
    upsertTranscriptEntry(setters.setTranscript, entry);
  }
  markClientTurnEvent("client_transcript_entry_scheduled", {
    ...clientEventFields(event),
    entry_type: entry.type
  });
}

export function pushTranscript(
  setTranscript: Dispatch<SetStateAction<TranscriptEntry[]>>,
  entry: TranscriptEntry
) {
  setTranscript((current) => [...current, entry]);
}

function replaceOptimisticTranscriptEntry(
  setTranscript: Dispatch<SetStateAction<TranscriptEntry[]>>,
  optimisticId: string,
  entry: TranscriptEntry
) {
  setTranscript((current) => {
    const index = current.findIndex((candidate) => candidate.id === optimisticId);
    if (index === -1) {
      return [...current, entry];
    }
    return current.map((candidate, candidateIndex) =>
      candidateIndex === index ? { ...entry, id: optimisticId } : candidate
    );
  });
}

function upsertTranscriptEntry(
  setTranscript: Dispatch<SetStateAction<TranscriptEntry[]>>,
  entry: TranscriptEntry
) {
  setTranscript((current) => upsertTranscriptEntryValue(current, entry));
}

function upsertTranscriptEntryValue(current: TranscriptEntry[], entry: TranscriptEntry): TranscriptEntry[] {
  const entryItemId = transcriptEntryItemId(entry);
  const entryRuntimeId = transcriptEntryRuntimeId(entry);
  const existingIndex = current.findIndex(
    (candidate) =>
      (entryItemId !== undefined && transcriptEntryItemId(candidate) === entryItemId) ||
      (entryRuntimeId !== undefined && transcriptEntryRuntimeId(candidate) === entryRuntimeId) ||
      candidate.id === entry.id
  );
  const streamIndex =
    entry.type === "assistant" && entry.streamId
      ? current.findIndex((candidate) => candidate.type === "assistant_stream" && candidate.streamId === entry.streamId)
      : -1;

  if (existingIndex !== -1) {
    return current.flatMap((candidate, candidateIndex) => {
      if (candidateIndex === existingIndex) {
        return [entry];
      }
      if (streamIndex !== -1 && candidateIndex === streamIndex) {
        return [];
      }
      return [candidate];
    });
  }

  if (streamIndex !== -1) {
    return current.map((candidate, candidateIndex) => (candidateIndex === streamIndex ? entry : candidate));
  }

  return [...current, entry];
}

function removeStaleStartedMemoryExtractions(current: TranscriptEntry[]): TranscriptEntry[] {
  return current.filter(
    (entry) =>
      !(
        entry.type === "activity" &&
        entry.item.activity_kind === "memory_extraction" &&
        entry.item.status === "STARTED"
      )
  );
}

function transcriptEntryItemId(entry: TranscriptEntry): string | undefined {
  return "itemId" in entry ? entry.itemId : undefined;
}

function transcriptEntryRuntimeId(entry: TranscriptEntry): string | undefined {
  if (entry.type === "activity") {
    return entry.item.id;
  }
  return undefined;
}

function appendAssistantTextDelta(
  setTranscript: Dispatch<SetStateAction<TranscriptEntry[]>>,
  event: { turnId: string; streamId: string; delta: string; conversationId: string }
) {
  void event.conversationId;
  setTranscript((current) => {
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
  });
}

function entryFromReplayItem(item: ReplayItem): TranscriptEntry | null {
  const entry = entryFromConversationItem(item.itemId, item.turnId ?? undefined, item.item);
  return entry ? { ...entry, source: "replay" } : null;
}

function entryFromConversationItem(
  itemId: string,
  turnId: string | undefined,
  item: GraphqlTranscriptItem,
  metadata?: unknown
): TranscriptEntry | null {
  const transcriptItem = transcriptItemFromGraphql(item);
  if (!transcriptItem) {
    return null;
  }
  if (transcriptItem.kind === "user_text") {
    return { id: itemId, itemId, turnId, type: "user", text: transcriptItem.text };
  }
  if (transcriptItem.kind === "assistant_text") {
    return {
      id: itemId,
      itemId,
      turnId,
      type: "assistant",
      streamId: streamIdFromMetadata(metadata),
      text: transcriptItem.text
    };
  }
  if (transcriptItem.kind === "activity") {
    return { id: itemId, itemId, turnId, type: "activity", item: transcriptItem };
  }
  if (transcriptItem.kind === "a2ui_card") {
    return { id: itemId, itemId, turnId, type: "card", item: transcriptItem };
  }
  if (transcriptItem.kind === "error_notice") {
    return {
      id: itemId,
      itemId,
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
