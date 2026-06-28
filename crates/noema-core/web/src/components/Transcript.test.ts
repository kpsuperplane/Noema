import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { shouldAnchorTranscriptEntry, shouldShowTypingIndicator } from "./Transcript";
import type { TranscriptEntry } from "../types";

describe("shouldShowTypingIndicator", () => {
  test("shows while a user turn is pending and no assistant answer has arrived", () => {
    const entries: TranscriptEntry[] = [{ id: "user-1", type: "user", text: "Hello" }];

    assert.equal(shouldShowTypingIndicator(entries, true), true);
  });

  test("stays visible through processing activity until assistant text arrives", () => {
    const entries: TranscriptEntry[] = [
      { id: "user-1", type: "user", text: "Remember this" },
      {
        id: "activity-1",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity-1",
          activity_kind: "memory_extraction",
          status: "STARTED",
          title: "Memory update",
          metadata: null
        }
      }
    ];

    assert.equal(shouldShowTypingIndicator(entries, true), true);
  });

  test("hides when assistant text has arrived after the latest user turn", () => {
    const entries: TranscriptEntry[] = [
      { id: "user-1", type: "user", text: "Hello" },
      { id: "assistant-1", type: "assistant", text: "Hi there." }
    ];

    assert.equal(shouldShowTypingIndicator(entries, true), false);
  });

  test("does not show when no turn is pending", () => {
    const entries: TranscriptEntry[] = [{ id: "user-1", type: "user", text: "Hello" }];

    assert.equal(shouldShowTypingIndicator(entries, false), false);
  });
});

describe("shouldAnchorTranscriptEntry", () => {
  test("does not use live message anchors for user messages", () => {
    assert.equal(shouldAnchorTranscriptEntry({ kind: "entry", entryType: "user" }), false);
  });

  test("does not use live message anchors for the typing indicator", () => {
    assert.equal(shouldAnchorTranscriptEntry({ kind: "typing" }), false);
  });

  test("does not use live message anchors for assistant responses", () => {
    assert.equal(shouldAnchorTranscriptEntry({ kind: "entry", entryType: "assistant" }), false);
  });
});
