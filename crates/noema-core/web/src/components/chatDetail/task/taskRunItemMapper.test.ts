import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { toolDetailRows } from "@/components/transcript/markerModel";
import { renderableTranscriptEntries } from "@/components/transcript/renderModel";
import {
  mapTaskRunItem,
  taskRunItemsToTranscriptEntries,
  type TaskRunItemSource
} from "./taskRunItemMapper";

const createdAt = "2026-07-13T12:00:00Z";

describe("taskRunItemsToTranscriptEntries", () => {
  test("pairs persisted tools into the shared marker with complete inspectable data", () => {
    const longValue = "x".repeat(400);
    const items = [
      source({
        itemId: "call",
        kind: "tool_call",
        status: "completed",
        correlationId: "provider-call-1",
        contentText: "web.search",
        payload: {
          call_id: "local-call-1",
          provider_call_id: "provider-call-1",
          arguments: { query: longValue }
        }
      }),
      source({
        itemId: "result",
        kind: "tool_result",
        status: "completed",
        correlationId: "provider-call-1",
        contentText: "web.search",
        payload: {
          call_id: "local-call-1",
          provider_call_id: "provider-call-1",
          name: "web.search",
          arguments: { query: longValue },
          success: true,
          payload: { matches: [{ title: "Complete result", body: longValue }] }
        }
      }),
      source({
        itemId: "result-input",
        kind: "model_input",
        contentText: JSON.stringify([
          {
            call_id: "provider-call-1",
            name: "web.search",
            success: true,
            arguments: { query: longValue },
            payload: { matches: [] }
          }
        ])
      })
    ].map((item) => mapTaskRunItem(item, "executor"));

    const entries = taskRunItemsToTranscriptEntries(items);
    assert.equal(entries.length, 2, "the duplicated tool-result model input should be omitted");
    const [marker] = renderableTranscriptEntries(entries, false, "IDLE");
    assert.equal(marker?.kind, "tool_marker");
    if (marker?.kind !== "tool_marker") {
      return;
    }

    assert.equal(marker.marker.call?.id, "call");
    assert.equal(marker.marker.result?.id, "result");
    assert.deepEqual(toolDetailRows(marker.marker), [
      { label: "Correlation", value: "provider-call-1" },
      { label: "Input", value: JSON.stringify({ query: longValue }, null, 2) },
      { label: "Success", value: "true" },
      {
        label: "Output",
        value: JSON.stringify({ matches: [{ title: "Complete result", body: longValue }] }, null, 2)
      }
    ]);
  });

  test("keeps genuine model input as a system message", () => {
    const [entry] = taskRunItemsToTranscriptEntries([
      mapTaskRunItem(
        source({
          itemId: "model-input",
          kind: "model_input",
          contentText: "user: Review the evidence against every criterion."
        }),
        "reviewer"
      )
    ]);

    assert.deepEqual(entry, {
      id: "model-input",
      source: "replay",
      turnId: "run-1:0:default",
      type: "system",
      text: "user: Review the evidence against every criterion."
    });
  });

  test("renders bounded evidence and context checkpoints as expandable neutral activities", () => {
    const evidence = mapTaskRunItem(
      source({
        itemId: "evidence",
        kind: "model_input",
        contentText: JSON.stringify({
          type: "NOEMA_BOUNDED_TASK_EVIDENCE",
          older_evidence_checkpoint: "retained fact",
          recent_results: [{ name: "web.fetch", payload: { content: "complete evidence" } }]
        }),
        payload: { model: "gpt-5.6-luna" }
      }),
      "executor"
    );
    const checkpoint = mapTaskRunItem(
      source({
        itemId: "checkpoint",
        kind: "context_checkpoint",
        contentText: "The population evidence is retained in full.",
        payload: { progress: { source: "semantic_compaction", criteria_satisfied: ["population"] } }
      }),
      "executor"
    );

    const entries = taskRunItemsToTranscriptEntries([evidence, checkpoint]);
    assert.deepEqual(
      entries.map((entry) => entry.type === "activity" ? entry.item.title : entry.type),
      ["Evidence checkpoint", "Context compacted"]
    );
    for (const entry of entries) {
      assert.equal(entry.type, "activity");
      if (entry.type !== "activity") {
        continue;
      }
      const metadata = entry.item.metadata as {
        detail: string;
        presentation: { tone: string };
      };
      assert.equal(metadata.presentation.tone, "neutral");
      assert.match(metadata.detail, entry.id === "evidence" ? /complete evidence/ : /criteria_satisfied/);
    }
  });
});

function source(overrides: Partial<TaskRunItemSource> & Pick<TaskRunItemSource, "itemId" | "kind">): TaskRunItemSource {
  return {
    runId: "run-1",
    roundIndex: 0,
    createdAt,
    ...overrides
  };
}
