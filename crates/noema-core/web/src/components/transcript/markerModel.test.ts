import { describe, test } from "node:test";
import assert from "node:assert/strict";
import {
  formatToolDetail,
  memoryCardsFromClaimOutcomes,
  memoryDetailItems,
  toolDetailRows,
  toolMarkerExpandable,
  toolMarkerLabel,
  toolMarkerName,
  toolMarkerTarget
} from "./markerModel";
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

  test("shows web search display metadata with visible query details", () => {
    const detail = formatToolDetail("Tool call: web.search", {
      action: {
        name: "web.search",
        payload: {
          query: "rust language",
          reason: "answer current question"
        }
      },
      display: {
        name: "Search web",
        access: "Searches public web",
        target: "Web search: rust language",
        provider: "DuckDuckGo public search",
        reliability: "Best effort"
      }
    });

    assert.match(detail, /Search web/);
    assert.match(detail, /Access: Searches public web/);
    assert.match(detail, /Target: Web search: rust language/);
    assert.match(detail, /Provider: DuckDuckGo public search/);
    assert.match(detail, /Reliability: Best effort/);
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

  test("labels raw web search tool names for users", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:web",
      call: {
        id: "entry:web",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: web.search",
          summary: "Web search: rust language",
          metadata: {
            action: { name: "web.search" }
          }
        }
      }
    };

    assert.equal(toolMarkerName(marker), "Search web");
  });
});

describe("toolMarkerLabel", () => {
  test("completed web search marker target keeps the query visible", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:web",
      call: {
        id: "call-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web-call",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: web.search",
          summary: "Web search: rust language",
          metadata: {
            action: {
              name: "web.search",
              payload: { query: "rust language" }
            },
            display: {
              name: "Search web",
              target: "Web search: rust language"
            }
          }
        }
      },
      result: {
        id: "result-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web-result",
          activity_kind: "tool_result",
          status: "COMPLETED",
          title: "Tool result: web.search",
          summary: "Found 1 web result",
          metadata: {
            action: {
              name: "web.search",
              success: true,
              payload: {
                provider: "duckduckgo_public",
                provider_contract: "best_effort_public",
                summary: "Found 1 web result",
                results: [{ rank: 1, title: "Rust", url: "https://www.rust-lang.org/", snippet: "Rust language." }]
              }
            },
            display: {
              name: "Search web",
              result: "Found 1 web result",
              provider: "DuckDuckGo public search",
              reliability: "Best effort"
            }
          }
        }
      }
    };

    assert.equal(toolMarkerLabel(marker), "Used Search web");
    assert.equal(toolMarkerTarget(marker), "Web search: rust language");
  });
});

describe("toolDetailRows", () => {
  test("shows concrete tool input and output without generic display filler", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:1",
      call: {
        id: "call-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:1",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: mcp.dex.search_contacts",
          summary: null,
          metadata: {
            provider: "codex",
            action: {
              id: "call_123",
              name: "mcp.dex.search_contacts",
              payload: {
                query: "Gautam"
              }
            },
            display: {
              name: "Dex search contacts",
              purpose: "Use an enabled connected tool",
              access: "Uses a connected tool"
            }
          }
        }
      },
      result: {
        id: "result-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:2",
          activity_kind: "tool_result",
          status: "COMPLETED",
          title: "Tool result: mcp.dex.search_contacts",
          summary: "Completed",
          metadata: {
            action: {
              id: "call_123",
              name: "mcp.dex.search_contacts",
              success: true,
              payload: {
                content: [{ type: "text", text: "Found Gautam's contact." }]
              }
            },
            display: {
              name: "Dex search contacts",
              access: "Uses a connected tool",
              result: "Completed"
            }
          }
        }
      }
    };

    const rows = toolDetailRows(marker);

    assert.deepEqual(rows, [
      { label: "Query", value: "Gautam" },
      { label: "Output", value: "Found Gautam's contact." }
    ]);
    assert.equal(rows[1]?.value, "Found Gautam's contact.");
    assert.doesNotMatch(rows.map((row) => row.value).join("\n"), /call_123/);
    assert.doesNotMatch(rows.map((row) => row.value).join("\n"), /Use an enabled connected tool/);
    assert.doesNotMatch(rows.map((row) => row.value).join("\n"), /Uses a connected tool/);
  });

  test("treats generic completed tool metadata as non-expandable", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:1",
      call: {
        id: "call-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:1",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: mcp.dex.search_contacts",
          summary: null,
          metadata: {
            action: {
              name: "mcp.dex.search_contacts",
              payload: {}
            },
            display: {
              name: "Dex search contacts",
              purpose: "Use an enabled connected tool",
              access: "Uses a connected tool"
            }
          }
        }
      },
      result: {
        id: "result-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:2",
          activity_kind: "tool_result",
          status: "COMPLETED",
          title: "Tool result: mcp.dex.search_contacts",
          summary: "Completed",
          metadata: {
            action: {
              name: "mcp.dex.search_contacts",
              success: true,
              payload: {}
            },
            display: {
              name: "Dex search contacts",
              access: "Uses a connected tool",
              result: "Completed"
            }
          }
        }
      }
    };

    assert.deepEqual(toolDetailRows(marker), []);
    assert.equal(toolMarkerExpandable(marker), false);
    assert.equal(toolMarkerTarget(marker), undefined);
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
