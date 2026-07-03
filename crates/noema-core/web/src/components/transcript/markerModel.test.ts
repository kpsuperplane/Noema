import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { formatToolDetail, memoryCardsFromClaimOutcomes, memoryDetailItems, toolMarkerName } from "./markerModel";
import type { TurnTranscriptItem } from "@/shared/types";
import type { ToolMarkerGroup } from "./renderModel";

describe("formatToolDetail", () => {
  test("shows user-facing tool details without provider or call identifiers", () => {
    const detail = formatToolDetail("Tool call: search_memory", {
      provider: "codex",
      action: {
        id: "call_123",
        name: "search_memory",
        payload: {
          query: "project notes",
          scope_ids: ["human:local"],
          purpose: "answer_human_question"
        }
      },
      display: {
        name: "Search memory",
        purpose: "Answer the question from saved memories",
        access: "Reads memory",
        scope: "human:local",
        result: "Found 2 memories"
      }
    });

    assert.match(detail, /Search memory/);
    assert.match(detail, /Purpose: Answer the question from saved memories/);
    assert.match(detail, /Access: Reads memory/);
    assert.match(detail, /Scope: human:local/);
    assert.match(detail, /Result: Found 2 memories/);
    assert.doesNotMatch(detail, /Provider:/);
    assert.doesNotMatch(detail, /Call ID:/);
    assert.doesNotMatch(detail, /call_123/);
  });
});

describe("toolMarkerName", () => {
  test("prefers the user-facing display name over raw tool names", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:1",
      call: {
        id: "entry:1",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:1",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: search_memory",
          summary: "provider id call_123",
          metadata: {
            action: { name: "search_memory" },
            display: { name: "Search memory" }
          }
        }
      }
    };

    assert.equal(toolMarkerName(marker), "Search memory");
  });
});

describe("memory detail model", () => {
  test("keeps review-only memory outcomes visible without claim ids", () => {
    const extraction: Extract<TurnTranscriptItem, { kind: "activity" }> = {
      kind: "activity",
      id: "memory:1",
      activity_kind: "memory_extraction",
      status: "COMPLETED",
      title: "Memory needs review",
      summary: "stored 1 memory for review",
      metadata: {
        claim_outcomes: [
          {
            outcome: "needs_review",
            fact_preview: "Kevin may prefer concise inspection output.",
            sensitivity: "normal",
            status: "candidate"
          }
        ],
        needs_review_claim_count: 1
      }
    };

    const memories = memoryCardsFromClaimOutcomes(extraction);
    assert.equal(memories.length, 1);
    assert.equal(memories[0]?.title, "Kevin may prefer concise inspection output.");
    assert.equal(memories[0]?.status, "Needs review");

    const rows = memoryDetailItems(memories)[0]?.rows ?? [];
    assert.deepEqual(
      rows.map((row) => row.label),
      ["Memory", "Sensitivity", "Status"]
    );
  });
});
