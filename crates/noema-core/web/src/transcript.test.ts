import assert from "node:assert/strict";
import { describe, test } from "node:test";
import type { SetStateAction } from "react";
import { handleConversationEvent } from "./transcript";
import type { ConversationAgentStatus, TranscriptEntry } from "./types";

type ConversationEvent = Parameters<typeof handleConversationEvent>[0];

describe("handleConversationEvent", () => {
  test("updates memory extraction activity by runtime id", () => {
    let transcript: TranscriptEntry[] = [];
    let pending = true;
    let agentStatus: ConversationAgentStatus = "THINKING";
    const setters = {
      setTranscript: (next: SetStateAction<TranscriptEntry[]>) => {
        transcript = typeof next === "function" ? next(transcript) : next;
      },
      setPending: (next: SetStateAction<boolean>) => {
        pending = typeof next === "function" ? next(pending) : next;
      },
      setAgentStatus: (next: SetStateAction<ConversationAgentStatus>) => {
        agentStatus = typeof next === "function" ? next(agentStatus) : next;
      }
    };

    handleConversationEvent(memoryExtractionEvent("item-start", "STARTED", "Memory proposed"), setters);
    handleConversationEvent(memoryExtractionEvent("item-done", "COMPLETED", "Memory extraction completed"), setters);

    assert.equal(transcript.length, 1);
    const [entry] = transcript;
    assert.equal(entry?.type, "activity");
    if (entry?.type !== "activity") {
      throw new Error("expected activity entry");
    }
    assert.equal(entry.itemId, "item-done");
    assert.equal(entry.item.status, "COMPLETED");
  });
});

function memoryExtractionEvent(itemId: string, status: "STARTED" | "COMPLETED", title: string): ConversationEvent {
  return {
    __typename: "GraphqlConversationItemEvent",
    conversationId: "conversation_1",
    clientMessageId: null,
    itemId,
    turnId: "turn_1",
    metadata: null,
    item: {
      __typename: "GraphqlActivity",
      id: "memory_extraction:conversation_1:1",
      activityKind: "memory_extraction",
      status,
      title,
      summary: null,
      metadata: null
    }
  } as ConversationEvent;
}
