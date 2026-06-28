import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  memoryDetailItems,
  renderedTranscriptLane,
  shouldAnchorTranscriptEntry,
  shouldAnimateMessageText,
  shouldShowTypingIndicator,
  transcriptEntryLane
} from "./Transcript";
import { messageTextAnimationTokens, visibleMessageTextAnimationTokens } from "./MessageTextAnimation";
import type { TranscriptEntry } from "../types";

describe("transcriptEntryLane", () => {
  test("places user entries in the human lane", () => {
    assert.equal(transcriptEntryLane("user"), "human");
  });

  test("places assistant entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("assistant"), "assistant");
  });

  test("places activity entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("activity"), "assistant");
  });

  test("places structured card entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("card"), "assistant");
  });

  test("places error entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("error"), "assistant");
  });
});

describe("renderedTranscriptLane", () => {
  test("places typing indicators in the assistant lane", () => {
    assert.equal(renderedTranscriptLane({ kind: "typing" }), "assistant");
  });

  test("places grouped memory markers in the assistant lane", () => {
    assert.equal(renderedTranscriptLane({ kind: "memory_marker" }), "assistant");
  });

  test("delegates normal entries to transcriptEntryLane", () => {
    assert.equal(renderedTranscriptLane({ kind: "entry", entryType: "user" }), "human");
    assert.equal(renderedTranscriptLane({ kind: "entry", entryType: "error" }), "assistant");
  });
});

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

describe("messageTextAnimationTokens", () => {
  test("splits message text into word tokens with preserved whitespace", () => {
    assert.deepEqual(messageTextAnimationTokens("Hello  Noema\nagain"), [
      { kind: "word", value: "Hello", wordIndex: 0 },
      { kind: "space", value: "  ", previousWordIndex: 0, nextWordIndex: 1 },
      { kind: "word", value: "Noema", wordIndex: 1 },
      { kind: "space", value: "\n", previousWordIndex: 1, nextWordIndex: 2 },
      { kind: "word", value: "again", wordIndex: 2 }
    ]);
  });

  test("omits words and spacing that have not reached their reveal time", () => {
    assert.deepEqual(visibleMessageTextAnimationTokens(messageTextAnimationTokens("Hello  Noema\nagain"), 1), [
      { kind: "word", value: "Hello", wordIndex: 0 }
    ]);

    assert.deepEqual(visibleMessageTextAnimationTokens(messageTextAnimationTokens("Hello  Noema\nagain"), 2), [
      { kind: "word", value: "Hello", wordIndex: 0 },
      { kind: "space", value: "  ", previousWordIndex: 0, nextWordIndex: 1 },
      { kind: "word", value: "Noema", wordIndex: 1 }
    ]);
  });
});

describe("shouldAnimateMessageText", () => {
  test("does not animate replayed messages", () => {
    assert.equal(shouldAnimateMessageText({ id: "existing", source: "replay", type: "assistant", text: "Loaded" }), false);
  });

  test("animates live assistant messages", () => {
    assert.equal(shouldAnimateMessageText({ id: "live", type: "assistant_stream", streamId: "stream", text: "New" }), true);
  });

  test("does not animate optimistic user messages", () => {
    assert.equal(shouldAnimateMessageText({ id: "optimistic", type: "user", text: "Hello" }), false);
  });
});

describe("memoryDetailItems", () => {
  test("includes created memory content and id in expanded memory marker details", () => {
    assert.deepEqual(
      memoryDetailItems([
        {
          id: "memory_123",
          title: "Kevin prefers CLI memory inspection",
          content: "Kevin prefers CLI memory inspection.",
          memoryType: "preference",
          sensitivity: "low",
          status: "confirmed"
        }
      ]),
      [
        {
          title: "Kevin prefers CLI memory inspection",
          rows: [
            { label: "Memory", value: "Kevin prefers CLI memory inspection." },
            { label: "Type", value: "preference" },
            { label: "Sensitivity", value: "low" },
            { label: "Status", value: "confirmed" },
            { label: "Memory ID", value: "memory_123" }
          ]
        }
      ]
    );
  });
});
