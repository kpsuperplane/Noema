import type { Dispatch, SetStateAction } from "react";
import type { TurnTranscriptItem, WebConversationItem, WebServerMessage as ServerMessage } from "./generated/noema";
import type { ConversationAgentStatus, TranscriptEntry } from "./types";

export function handleServerMessage(
  message: ServerMessage,
  setters: {
    setConversationId: Dispatch<SetStateAction<string | null>>;
    setTranscript: Dispatch<SetStateAction<TranscriptEntry[]>>;
    setPending: Dispatch<SetStateAction<boolean>>;
    setAgentStatus: Dispatch<SetStateAction<ConversationAgentStatus>>;
  }
) {
  if (message.type === "conversation_started") {
    setters.setConversationId(message.conversation_id);
    setters.setAgentStatus("idle");
    return;
  }
  if (message.type === "turn_completed") {
    setters.setPending(false);
    return;
  }
  if (message.type === "agent_status_changed") {
    setters.setAgentStatus(message.status);
    return;
  }
  if (message.type === "conversation_replay") {
    setters.setConversationId(message.conversation_id);
    setters.setTranscript(message.items.map(entryFromReplayItem).filter((entry) => entry !== null));
    setters.setPending(false);
    return;
  }
  if (message.type === "error") {
    setters.setPending(false);
    setters.setAgentStatus("closed");
    pushTranscript(setters.setTranscript, {
      id: crypto.randomUUID(),
      type: "error",
      message: message.message,
      recoverable: true
    });
    return;
  }
  if (message.type !== "conversation_item") {
    return;
  }

  const entry = entryFromConversationItem(message.item_id, message.turn_id, message.item);
  if (!entry) {
    return;
  }
  if (message.client_message_id && entry.type === "user") {
    replaceOptimisticTranscriptEntry(setters.setTranscript, message.client_message_id, entry);
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

function entryFromReplayItem(item: WebConversationItem): TranscriptEntry | null {
  return entryFromConversationItem(item.item_id, item.turn_id, item.item);
}

function entryFromConversationItem(
  itemId: string,
  turnId: string | undefined,
  item: TurnTranscriptItem
): TranscriptEntry | null {
  if (item.kind === "user_text") {
    return { id: itemId, itemId, turnId, type: "user", text: item.text };
  }
  if (item.kind === "assistant_text") {
    return { id: itemId, itemId, turnId, type: "assistant", text: item.text };
  }
  if (item.kind === "activity") {
    return { id: itemId, itemId, turnId, type: "activity", item };
  }
  if (item.kind === "a2ui_card") {
    return { id: itemId, itemId, turnId, type: "card", item };
  }
  if (item.kind === "error_notice") {
    return { id: itemId, itemId, turnId, type: "error", message: item.message, recoverable: item.recoverable };
  }
  return null;
}
