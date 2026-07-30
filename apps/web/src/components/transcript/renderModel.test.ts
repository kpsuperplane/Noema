import { describe, test } from "node:test";
import assert from "node:assert/strict";
import {
  renderedChatBubbleGroup,
  renderedEntryMessageId,
  renderableTranscriptEntries
} from "./renderModel";
import type { TranscriptEntry } from "@/shared/types";

describe("renderableTranscriptEntries", () => {
  test("correlates non-adjacent tool calls and results by invocation id", () => {
    const entries: TranscriptEntry[] = [
      {
        id: "call-entry",
        turnId: "turn:1",
        type: "activity",
        item: {
          kind: "activity",
          id: "tool_call:1",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: search_memory",
          metadata: {
            action: { id: "call_memory_1", name: "search_memory" },
            display: { name: "Search memory" }
          }
        }
      },
      {
        id: "assistant-entry",
        turnId: "turn:1",
        type: "assistant",
        text: "Checking memory."
      },
      {
        id: "result-entry",
        turnId: "turn:1",
        type: "activity",
        item: {
          kind: "activity",
          id: "tool_result:1",
          activity_kind: "tool_result",
          status: "COMPLETED",
          title: "Tool result: search_memory",
          metadata: {
            action: { call_id: "call_memory_1", name: "search_memory", success: true },
            display: { name: "Search memory", result: "Found 1 memory" }
          }
        }
      }
    ];

    const rendered = renderableTranscriptEntries(entries, false, "IDLE");
    const toolMarkers = rendered.filter((entry) => entry.kind === "tool_marker");

    assert.equal(toolMarkers.length, 1);
    assert.equal(toolMarkers[0]?.marker.call?.id, "call-entry");
    assert.equal(toolMarkers[0]?.marker.result?.id, "result-entry");
    assert.equal(rendered.some((entry) => entry.kind === "entry" && entry.entry.id === "assistant-entry"), true);
  });

  test("keeps stream ids for finalized assistant message render identity", () => {
    const rendered = renderableTranscriptEntries(
      [
        {
          id: "assistant:item:1",
          itemId: "assistant:item:1",
          turnId: "turn:1",
          type: "assistant",
          streamId: "assistant_stream:turn:1:initial:response:0",
          text: "first"
        },
        {
          id: "assistant:item:2",
          itemId: "assistant:item:2",
          turnId: "turn:1",
          type: "assistant",
          streamId: "assistant_stream:turn:1:initial:response:1",
          text: "second"
        }
      ],
      false,
      "IDLE"
    );

    assert.deepEqual(rendered.map((entry) => renderedEntryMessageId(entry)), [
      "assistant_stream:turn:1:initial:response:0",
      "assistant_stream:turn:1:initial:response:1"
    ]);
  });

  test("leaves memory extraction and proposal items on the generic transcript path", () => {
    const rendered = renderableTranscriptEntries(
      [
        {
          id: "memory-extraction-entry",
          turnId: "turn:memory",
          type: "activity",
          item: {
            kind: "activity",
            id: "memory-extraction",
            activity_kind: "memory_extraction",
            status: "COMPLETED",
            title: "Memory updated",
            summary: "Updated memory",
            metadata: {}
          }
        },
        {
          id: "memory-proposal-entry",
          turnId: "turn:memory",
          type: "a2ui_surface",
          item: {
            kind: "a2ui_surface",
            id: "memory-proposal",
            interaction_id: null,
            surface_id: "memory-proposals",
            version: "v0.9.1",
            revision: 1,
            interaction_revision: null,
            lifecycle: "completed",
            catalog: {},
            snapshot: {},
            has_actions: false
          }
        }
      ],
      false,
      "IDLE"
    );

    assert.deepEqual(
      rendered.map((entry) => entry.kind),
      ["entry", "entry"]
    );
  });
});

describe("renderedChatBubbleGroup", () => {
  test("groups adjacent assistant text bubbles from the same turn", () => {
    const rendered = renderableTranscriptEntries(
      [
        {
          id: "assistant:item:1",
          turnId: "turn:1",
          type: "assistant",
          text: "first"
        },
        {
          id: "assistant:item:2",
          turnId: "turn:1",
          type: "assistant",
          text: "second"
        },
        {
          id: "assistant:item:3",
          turnId: "turn:1",
          type: "assistant",
          text: "third"
        }
      ],
      false,
      "IDLE"
    );

    assert.deepEqual(
      rendered.map((entry, index) =>
        renderedChatBubbleGroup(entry, rendered[index - 1], rendered[index + 1])
      ),
      ["first", "middle", "last"]
    );
  });

  test("does not group text bubbles across non-text transcript entries", () => {
    const rendered = renderableTranscriptEntries(
      [
        {
          id: "assistant:item:1",
          turnId: "turn:1",
          type: "assistant",
          text: "first"
        },
        {
          id: "tool-call-entry",
          turnId: "turn:1",
          type: "activity",
          item: {
            kind: "activity",
            id: "tool_call:1",
            activity_kind: "tool_call",
            status: "COMPLETED",
            title: "Tool call: search_memory",
            metadata: {
              action: { id: "call_memory_1", name: "search_memory" }
            }
          }
        },
        {
          id: "assistant:item:2",
          turnId: "turn:1",
          type: "assistant",
          text: "second"
        }
      ],
      false,
      "IDLE"
    );

    assert.deepEqual(
      rendered.map((entry, index) =>
        renderedChatBubbleGroup(entry, rendered[index - 1], rendered[index + 1])
      ),
      [undefined, undefined, undefined]
    );
  });

  test("groups multiple choice prompts with adjacent assistant text bubbles", () => {
    const rendered = renderableTranscriptEntries(
      [
        {
          id: "assistant:item:1",
          turnId: "turn:choice",
          type: "assistant",
          text: "Pick one."
        },
        {
          id: "choice:item:1",
          turnId: "turn:choice",
          type: "multiple_choice_prompt",
          item: {
            kind: "multiple_choice_prompt",
            prompt: "Which approach?",
            selection_mode: "PICK_ONE",
            options: [
              { id: "a", label: "A" },
              { id: "b", label: "B" }
            ]
          }
        }
      ],
      false,
      "IDLE"
    );

    assert.deepEqual(
      rendered.map((entry, index) =>
        renderedChatBubbleGroup(entry, rendered[index - 1], rendered[index + 1])
      ),
      ["first", "last"]
    );
  });

  test("groups multiple choice selections with adjacent human text bubbles", () => {
    const rendered = renderableTranscriptEntries(
      [
        {
          id: "user:item:1",
          turnId: "turn:selection",
          type: "user",
          text: "I like this one"
        },
        {
          id: "choice:selection:1",
          turnId: "turn:selection",
          type: "multiple_choice_selection",
          item: {
            kind: "multiple_choice_selection",
            prompt_item_id: "choice:item:1",
            selection_mode: "PICK_ONE",
            selected_options: [{ id: "a", label: "A" }]
          }
        }
      ],
      false,
      "IDLE"
    );

    assert.deepEqual(
      rendered.map((entry, index) =>
        renderedChatBubbleGroup(entry, rendered[index - 1], rendered[index + 1])
      ),
      ["first", "last"]
    );
  });
});
