import type { Dispatch, SetStateAction } from "react";
import type {
  ConversationEventsSubscription,
  StartPrimaryConversationMutation
} from "./generated/graphql";
import type { ConversationAgentStatus, TranscriptEntry, TurnTranscriptItem } from "./types";

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
  }
) {
  if (event.__typename === "GraphqlTurnCompletedEvent") {
    setters.setPending(false);
    setters.setAgentStatus("IDLE");
    return;
  }
  if (event.__typename === "GraphqlAgentStatusEvent") {
    setters.setAgentStatus(event.status);
    return;
  }
  if (event.__typename !== "GraphqlConversationItemEvent") {
    return;
  }

  const entry = entryFromConversationItem(event.itemId, event.turnId ?? undefined, event.item);
  if (!entry) {
    return;
  }
  if (event.clientMessageId && entry.type === "user") {
    replaceOptimisticTranscriptEntry(setters.setTranscript, event.clientMessageId, entry);
  } else {
    upsertTranscriptEntry(setters.setTranscript, entry);
  }
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
    return current.map((candidate, candidateIndex) => (candidateIndex === index ? entry : candidate));
  });
}

function upsertTranscriptEntry(
  setTranscript: Dispatch<SetStateAction<TranscriptEntry[]>>,
  entry: TranscriptEntry
) {
  setTranscript((current) => {
    const index = current.findIndex((candidate) => candidate.itemId === entry.itemId || candidate.id === entry.id);
    if (index === -1) {
      return [...current, entry];
    }
    return current.map((candidate, candidateIndex) => (candidateIndex === index ? entry : candidate));
  });
}

function entryFromReplayItem(item: ReplayItem): TranscriptEntry | null {
  return entryFromConversationItem(item.itemId, item.turnId ?? undefined, item.item);
}

function entryFromConversationItem(
  itemId: string,
  turnId: string | undefined,
  item: GraphqlTranscriptItem
): TranscriptEntry | null {
  const transcriptItem = transcriptItemFromGraphql(item);
  if (!transcriptItem) {
    return null;
  }
  if (transcriptItem.kind === "user_text") {
    return { id: itemId, itemId, turnId, type: "user", text: transcriptItem.text };
  }
  if (transcriptItem.kind === "assistant_text") {
    return { id: itemId, itemId, turnId, type: "assistant", text: transcriptItem.text };
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

function transcriptItemFromGraphql(item: GraphqlTranscriptItem): TurnTranscriptItem | null {
  if (item.__typename === "GraphqlUserText") {
    return { kind: "user_text", text: item.text };
  }
  if (item.__typename === "GraphqlAssistantText") {
    return { kind: "assistant_text", text: item.text };
  }
  if (item.__typename === "GraphqlActivity") {
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
  if (item.__typename === "GraphqlA2UiCard") {
    return { kind: "a2ui_card", id: item.id, schema: item.schema, payload: item.payload };
  }
  if (item.__typename === "GraphqlErrorNotice") {
    return { kind: "error_notice", message: item.message, recoverable: item.recoverable };
  }
  return null;
}
