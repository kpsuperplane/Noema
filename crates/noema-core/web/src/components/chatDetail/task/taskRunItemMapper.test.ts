import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  mapTaskRunItem,
  taskRunItemsToTranscriptEntries,
  type TaskRunItemSource
} from "./taskRunItemMapper";
import type { TranscriptEntry } from "@/shared/types";

describe("taskRunItemsToTranscriptEntries", () => {
  test("renders the initial executor context as a neutral system message", () => {
    const entries = transcriptEntries([
      source({
        itemId: "input:initial",
        kind: "model_input",
        contentText: "Task ID: task:1\n\nResearch the request."
      })
    ]);

    assert.deepEqual(entries, [
      {
        id: "input:initial",
        source: "replay",
        turnId: "run:1:0:default",
        type: "system",
        text: "Task ID: task:1\n\nResearch the request."
      }
    ]);
  });

  test("omits model inputs that only replay recorded tool results", () => {
    const entries = transcriptEntries([
      source({
        itemId: "input:tools",
        kind: "model_input",
        roundIndex: 1,
        contentText: JSON.stringify([
          {
            call_id: "call:1",
            name: "web.search",
            success: true,
            arguments: { query: "bear populations" },
            payload: { results: [] }
          }
        ])
      })
    ]);

    assert.deepEqual(entries, []);
  });

  test("omits compacted evidence model inputs and context checkpoints", () => {
    const entries = transcriptEntries([
      source({
        itemId: "input:bounded",
        kind: "model_input",
        roundIndex: 6,
        contentText: JSON.stringify({
          type: "NOEMA_BOUNDED_TASK_EVIDENCE",
          older_evidence_checkpoint: "saved evidence",
          recent_results: {
            type: "NOEMA_RECENT_TOOL_RESULTS",
            results: [{ call_id: "call:2", name: "web.fetch" }]
          }
        })
      }),
      source({
        itemId: "checkpoint:1",
        kind: "context_checkpoint",
        roundIndex: 6,
        contentText: "a large internal evidence checkpoint"
      })
    ]);

    assert.deepEqual(entries, []);
  });

  test("projects persisted tool envelopes into the shared chat tool activity contract", () => {
    const entries = transcriptEntries([
      source({
        itemId: "tool:call",
        kind: "tool_call",
        correlationId: "call:search",
        contentText: "web.search",
        payload: {
          output_index: 0,
          provider_call_id: "provider:call",
          arguments: { query: "bear populations", max_results: 5 }
        }
      }),
      source({
        itemId: "tool:result",
        kind: "tool_result",
        correlationId: "call:search",
        parentItemId: "tool:call",
        contentText: "web.search",
        payload: {
          provider_call_id: "provider:call",
          name: "web.search",
          arguments: { query: "bear populations", max_results: 5 },
          success: true,
          payload: { query: "bear populations", results: [{ title: "Bear report" }] }
        }
      })
    ]);

    assert.equal(entries.length, 2);
    assert.deepEqual(activityAction(entries[0]), {
      id: "call:search",
      name: "web.search",
      payload: { query: "bear populations", max_results: 5 }
    });
    assert.deepEqual(activityAction(entries[1]), {
      call_id: "call:search",
      name: "web.search",
      payload: { query: "bear populations", results: [{ title: "Bear report" }] },
      success: true
    });
  });
});

function source(overrides: Partial<TaskRunItemSource>): TaskRunItemSource {
  return {
    itemId: "item:1",
    runId: "run:1",
    sequenceIndex: 1,
    roundIndex: 0,
    kind: "model_input",
    status: "completed",
    createdAt: "2026-07-13T12:00:00.000Z",
    ...overrides
  };
}

function transcriptEntries(sources: TaskRunItemSource[]) {
  const items = sources.map((item) => mapTaskRunItem(item, "executor"));
  return taskRunItemsToTranscriptEntries(items);
}

function activityAction(entry: TranscriptEntry | undefined): unknown {
  assert.ok(entry);
  assert.equal(entry.type, "activity");
  if (entry.type !== "activity") {
    return null;
  }
  const metadata = entry.item.metadata;
  assert.ok(typeof metadata === "object" && metadata !== null && "action" in metadata);
  return (metadata as { action: unknown }).action;
}
