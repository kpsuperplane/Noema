import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Transcript,
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

describe("Transcript memory markers", () => {
  test("labels started memory extraction as proposed before it is updated", () => {
    const markup = renderToStaticMarkup(
      React.createElement(Transcript, {
        entries: [
          {
            id: "activity-1",
            type: "activity",
            itemId: "item-1",
            turnId: "turn-1",
            item: {
              kind: "activity",
              id: "memory_extraction:conversation_1:1",
              activity_kind: "memory_extraction",
              status: "STARTED",
              title: "Memory proposed",
              summary: "creating 1 memory candidate",
              metadata: { turn_index: 1, proposal_count: 1 }
            }
          }
        ],
        pending: false,
        expandedActivities: new Set<string>(),
        onToggleActivity: () => {}
      })
    );

    assert.match(markup, /Memory proposed/);
    assert.doesNotMatch(markup, /Memory updated/);
  });

  test("collapses replayed proposed and updated memory rows into one updated marker", () => {
    const markup = renderToStaticMarkup(
      React.createElement(Transcript, {
        entries: [
          memoryExtractionEntry("activity-start", "item-start", "STARTED", "Memory proposed"),
          memoryProposalCardEntry("card-1", "item-card"),
          memoryExtractionEntry("activity-done", "item-done", "COMPLETED", "Memory extraction completed")
        ],
        pending: false,
        expandedActivities: new Set<string>(),
        onToggleActivity: () => {}
      })
    );

    assert.doesNotMatch(markup, /Memory proposed/);
    assert.equal(memoryUpdatedCount(markup), 1);
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

function memoryExtractionEntry(
  id: string,
  itemId: string,
  status: "STARTED" | "COMPLETED",
  title: string
): TranscriptEntry {
  return {
    id,
    type: "activity",
    itemId,
    turnId: "turn-1",
    item: {
      kind: "activity",
      id: "memory_extraction:conversation_1:1",
      activity_kind: "memory_extraction",
      status,
      title,
      summary: status === "STARTED" ? "creating 1 memory candidate" : "created 1 memory candidate",
      metadata: { turn_index: 1, proposal_count: 1 }
    }
  };
}

function memoryProposalCardEntry(id: string, itemId: string): TranscriptEntry {
  return {
    id,
    type: "card",
    itemId,
    turnId: "turn-1",
    item: {
      kind: "a2ui_card",
      id: "memory_proposals:conversation_1:1",
      schema: "memory_proposals",
      payload: {
        turn_index: 1,
        source: "provider_structured_output",
        created_memory_ids: ["memory_1"],
        proposals: [
          {
            status: "active",
            proposal: {
              content: "Kevin prefers same-call memory proposals.",
              title: "Same-call memory proposal preference",
              memory_type: "preference",
              sensitivity: "low"
            }
          }
        ]
      }
    }
  };
}

function memoryUpdatedCount(markup: string): number {
  return markup.match(/Memory updated/g)?.length ?? 0;
}

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
