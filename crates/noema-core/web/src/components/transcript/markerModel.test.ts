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

  test("labels raw web fetch tool names for users", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:web-fetch",
      call: {
        id: "entry:web-fetch",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web-fetch",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: web.fetch",
          summary: "Fetched web page: https://example.com/page",
          metadata: {
            action: { name: "web.fetch" }
          }
        }
      }
    };

    assert.equal(toolMarkerName(marker), "Fetch web");
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
                query: "rust language",
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
    assert.equal(toolMarkerExpandable(marker), false);
    assert.deepEqual(toolDetailRows(marker), []);
  });

  test("completed web search marker with provider fallback is expandable", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:web-fallback",
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
                fallback_from: "provider_account:openai:default",
                fallback_reason: "bound provider capability web.search is account_dependent",
                query: "rust language",
                summary: "Found 1 web result",
                results: [{ rank: 1, title: "Rust", url: "https://www.rust-lang.org/", snippet: "Rust language." }]
              }
            },
            display: {
              name: "Search web",
              result: "Found 1 web result",
              provider: "DuckDuckGo public search",
              reliability: "Best effort",
              fallbackFrom: "provider_account:openai:default",
              fallbackReason: "bound provider capability web.search is account_dependent"
            }
          }
        }
      }
    };

    assert.equal(toolMarkerExpandable(marker), true);
    assert.deepEqual(toolDetailRows(marker), [
      { label: "Fallback from", value: "provider_account:openai:default" },
      {
        label: "Fallback reason",
        value: "bound provider capability web.search is account_dependent"
      }
    ]);
  });

  test("completed web fetch marker target keeps the fetched URL visible", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:web-fetch",
      call: {
        id: "call-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web-fetch-call",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: web.fetch",
          summary: "Fetched web page: https://example.com/page",
          metadata: {
            action: {
              name: "web.fetch",
              payload: {
                url: "https://example.com/page",
                reason: "read public docs"
              }
            },
            display: {
              name: "Fetch web",
              target: "Fetched web page: https://example.com/page",
              purpose: "read public docs"
            }
          }
        }
      },
      result: {
        id: "result-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web-fetch-result",
          activity_kind: "tool_result",
          status: "COMPLETED",
          title: "Tool result: web.fetch",
          summary: "Fetched 12,840 chars",
          metadata: {
            action: {
              name: "web.fetch",
              success: true,
              payload: {
                provider: "direct_http",
                url: "https://example.com/page",
                final_url: "https://example.com/page",
                title: "Example",
                format: "markdown",
                extraction: "readability_rs",
                content_kind: "raw_markdown",
                content: "Fetched page body that should stay out of compact rows.",
                raw_excerpt: "Fetched page excerpt that should stay out of compact rows.",
                raw_chars: 12840,
                returned_chars: 12840,
                summary_model: null,
                summary_strategy: "not_summarized",
                truncated: false
              }
            },
            display: {
              name: "Fetch web",
              result: "Fetched 12,840 chars"
            }
          }
        }
      }
    };

    assert.equal(toolMarkerLabel(marker), "Used Fetch web");
    assert.equal(toolMarkerTarget(marker), "Fetched web page: https://example.com/page");
    assert.equal(toolMarkerExpandable(marker), false);
    assert.deepEqual(toolDetailRows(marker), []);
  });
});

describe("toolDetailRows", () => {
  test("keeps failed web search markers expandable with error detail", () => {
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
          status: "FAILED",
          title: "Tool result: web.search",
          summary: "search provider request failed",
          metadata: {
            action: {
              name: "web.search",
              success: false,
              payload: {
                error: "search provider request failed"
              }
            },
            display: {
              name: "Search web",
              result: "search provider request failed"
            }
          }
        }
      }
    };

    assert.equal(toolMarkerExpandable(marker), true);
    assert.deepEqual(toolDetailRows(marker), [
      { label: "Query", value: "rust language" },
      { label: "Error", value: "search provider request failed" }
    ]);
  });

  test("keeps failed web fetch markers expandable with safe error detail", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:web-fetch",
      call: {
        id: "call-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web-fetch-call",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: web.fetch",
          summary: "Fetched web page: https://example.com/page",
          metadata: {
            action: {
              name: "web.fetch",
              payload: { url: "https://example.com/page" }
            },
            display: {
              name: "Fetch web",
              target: "Fetched web page: https://example.com/page"
            }
          }
        }
      },
      result: {
        id: "result-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:web-fetch-result",
          activity_kind: "tool_result",
          status: "FAILED",
          title: "Tool result: web.fetch",
          summary: "Failed: web fetch request failed",
          metadata: {
            action: {
              name: "web.fetch",
              success: false,
              payload: {
                error: "web fetch request failed"
              }
            },
            display: {
              name: "Fetch web",
              result: "Failed: web fetch request failed"
            }
          }
        }
      }
    };

    assert.equal(toolMarkerExpandable(marker), true);
    assert.equal(toolMarkerTarget(marker), "Failed: web fetch request failed");
    assert.deepEqual(toolDetailRows(marker), [
      { label: "URL", value: "https://example.com/page" },
      { label: "Error", value: "web fetch request failed" }
    ]);
  });

  test("does not compact non-web tools that spoof the fetch display name", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:spoofed-fetch",
      call: {
        id: "call-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:spoofed-fetch-call",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call: mcp.dex.search_contacts",
          summary: "Fetched web page: https://example.com/page",
          metadata: {
            action: {
              name: "mcp.dex.search_contacts",
              payload: { query: "Kevin" }
            },
            display: {
              name: "Fetch web",
              target: "Fetched web page: https://example.com/page"
            }
          }
        }
      },
      result: {
        id: "result-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:spoofed-fetch-result",
          activity_kind: "tool_result",
          status: "COMPLETED",
          title: "Tool result: mcp.dex.search_contacts",
          summary: "Found contact",
          metadata: {
            action: {
              name: "mcp.dex.search_contacts",
              success: true,
              payload: {
                content: [{ type: "text", text: "Found Kevin's contact." }]
              }
            },
            display: {
              name: "Fetch web",
              result: "Found contact"
            }
          }
        }
      }
    };

    assert.equal(toolMarkerName(marker), "Fetch web");
    assert.equal(toolMarkerTarget(marker), "Found contact");
    assert.equal(toolMarkerExpandable(marker), true);
    assert.deepEqual(toolDetailRows(marker), [
      { label: "Query", value: "Kevin" },
      { label: "Output", value: "Found Kevin's contact." }
    ]);
  });

  test("does not compact markers with top-level web fetch name but no action name", () => {
    const marker: ToolMarkerGroup = {
      id: "tool_call:top-level-fetch-name",
      call: {
        id: "call-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:top-level-fetch-name-call",
          activity_kind: "tool_call",
          status: "COMPLETED",
          title: "Tool call",
          summary: "Fetched web page: https://example.com/page",
          metadata: {
            name: "web.fetch",
            action: {
              payload: { url: "https://example.com/page" }
            },
            display: {
              name: "Fetch web",
              target: "Fetched web page: https://example.com/page"
            }
          }
        }
      },
      result: {
        id: "result-entry",
        type: "activity",
        item: {
          kind: "activity",
          id: "activity:top-level-fetch-name-result",
          activity_kind: "tool_result",
          status: "COMPLETED",
          title: "Tool result",
          summary: "Fetched 42 chars",
          metadata: {
            name: "web.fetch",
            action: {
              success: true,
              payload: {
                content: [{ type: "text", text: "Fetched body preview." }]
              }
            },
            display: {
              name: "Fetch web",
              result: "Fetched 42 chars"
            }
          }
        }
      }
    };

    assert.equal(toolMarkerTarget(marker), "Fetched 42 chars");
    assert.equal(toolMarkerExpandable(marker), true);
    assert.deepEqual(toolDetailRows(marker), [
      { label: "URL", value: "https://example.com/page" },
      { label: "Output", value: "Fetched body preview." }
    ]);
  });

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
