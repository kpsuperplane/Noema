import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { renderedEntryMessageId, renderableTranscriptEntries } from "./renderModel";
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
          type: "card",
          item: {
            kind: "a2ui_card",
            id: "memory-proposal",
            schema: "memory_proposals",
            payload: { proposals: [] }
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
