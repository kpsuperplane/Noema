#!/usr/bin/env bun

// Synthetic research service for PA-032.
// It has no connection to a publisher, regulator, or real research database.

const port = Number(process.env.NOEMA_RESEARCH_PORT ?? "3750");
const fixtureVersion = "2026-09-08-research-api-v1";

type Source = {
  id: string;
  title: string;
  publisher: string;
  sourceType: "primary_report" | "correction" | "summary" | "dissent" | "outdated_article";
  publishedAt: string;
  updatedAt?: string;
  status: "current" | "superseded" | "commentary";
  summary: string;
  keyPoints: string[];
  corrects?: string;
  sourceUrl: string;
};

const profile = {
  topic: "Northstar heat-pump rebate program",
  questions: [
    "What rules are current?",
    "Which source is authoritative when reports conflict?",
    "What is fact, interpretation, or still uncertain?",
  ],
  preference: "Prioritize the latest official source, then identify disagreement and remaining uncertainty.",
};

const sources: Source[] = [
  {
    id: "source-001",
    title: "Northstar Heat-Pump Rebate Program — Final Rules",
    publisher: "Northstar Energy Office",
    sourceType: "primary_report",
    publishedAt: "2026-08-20",
    status: "superseded",
    summary: "The final-rules report describes eligibility, the application window, and a rebate cap table that was later corrected.",
    keyPoints: [
      "Owner-occupied homes may claim 40% of eligible equipment cost.",
      "The application window is 2026-09-01 through 2026-12-31.",
      "The printed table lists a $2,000 maximum rebate for every income tier.",
      "Applications require an invoice from an approved installer.",
    ],
    sourceUrl: "https://research.example.test/northstar/final-rules",
  },
  {
    id: "source-002",
    title: "Correction to the Northstar Rebate Cap Table",
    publisher: "Northstar Energy Office",
    sourceType: "correction",
    publishedAt: "2026-08-25",
    updatedAt: "2026-08-25",
    status: "current",
    summary: "The Energy Office corrected one table in the final-rules report and left the other eligibility rules unchanged.",
    keyPoints: [
      "For households at or below 80% of area median income, the maximum rebate is $2,500.",
      "For households above 80% of area median income, the maximum rebate is $1,500.",
      "The 40% equipment-cost rate and 2026-09-01 through 2026-12-31 window are unchanged.",
      "The correction replaces only the printed $2,000 cap table; it does not expand eligibility.",
    ],
    corrects: "source-001",
    sourceUrl: "https://research.example.test/northstar/correction-cap-table",
  },
  {
    id: "source-003",
    title: "What the Northstar Rebate Means for Homeowners",
    publisher: "Gridline Trade Journal",
    sourceType: "summary",
    publishedAt: "2026-08-26",
    status: "current",
    summary: "A trade summary repeats the original $2,000 cap and describes the program as a broad electrification incentive.",
    keyPoints: [
      "The summary says owner-occupied homes can receive 40% of eligible equipment cost.",
      "It reports a $2,000 maximum rebate without mentioning the official correction.",
      "It describes contractor demand as likely to rise, which is an interpretation rather than a rule.",
    ],
    sourceUrl: "https://research.example.test/northstar/gridline-summary",
  },
  {
    id: "source-004",
    title: "Northstar Rebate Opens September 1 After Cap Clarification",
    publisher: "Northstar City Ledger",
    sourceType: "summary",
    publishedAt: "2026-08-27",
    status: "current",
    summary: "A local news summary reports the corrected income-tier caps and the unchanged application dates.",
    keyPoints: [
      "The story reports a $2,500 cap at or below 80% of area median income and $1,500 above it.",
      "It says applications open 2026-09-01 and close 2026-12-31.",
      "It quotes an installer who expects a rush, which is a forecast rather than a confirmed program rule.",
    ],
    sourceUrl: "https://research.example.test/northstar/city-ledger-summary",
  },
  {
    id: "source-005",
    title: "Why the Northstar Rebate Still Falls Short",
    publisher: "Consumer Energy Watch",
    sourceType: "dissent",
    publishedAt: "2026-08-28",
    status: "commentary",
    summary: "A consumer group argues that the rebate is too small for many households and may not reach renters.",
    keyPoints: [
      "The group argues that installation costs can exceed the rebate by a wide margin.",
      "It estimates that many moderate-income households will still face more than $5,000 out of pocket.",
      "It recommends a larger cap and renter eligibility; these are policy recommendations, not current rules.",
    ],
    sourceUrl: "https://research.example.test/northstar/consumer-dissent",
  },
  {
    id: "source-006",
    title: "Draft Northstar Rebate Would Cap Support at $1,000",
    publisher: "Archive Home News",
    sourceType: "outdated_article",
    publishedAt: "2026-07-30",
    status: "superseded",
    summary: "An article about the draft proposal describes a $1,000 cap before the final rules were published.",
    keyPoints: [
      "The draft proposed a $1,000 maximum rebate.",
      "The draft used a 30% equipment-cost rate.",
      "The article predates the final report and the official correction and is not current evidence.",
    ],
    sourceUrl: "https://research.example.test/northstar/draft-archive",
  },
];

const docs = `# Synthetic research API

Fixture version: ${fixtureVersion}

This deterministic service contains one synthetic topic profile and six dated
synthetic sources. It has no connection to a publisher, regulator, research
index, or real account. Every record is test data. The base URL is the URL that
served this document.

## Operations

- \`GET /v1/profile\`: return the topic and the questions the reader needs answered.
- \`GET /v1/sources\`: return all six dated sources with source type, status,
  summary, key points, correction link, and source URL.

The source records provide dates, source types, status labels, summaries, key
points, correction links, and URLs. Use those fields to separate current facts,
earlier material, summaries, and commentary when sources disagree.

Responses use JSON with an explicit null next-page token. The service is
read-only and has no write operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function sourceResponse(source: Source) {
  return {
    id: source.id,
    title: source.title,
    publisher: source.publisher,
    source_type: source.sourceType,
    published_at: source.publishedAt,
    ...(source.updatedAt ? { updated_at: source.updatedAt } : {}),
    status: source.status,
    summary: source.summary,
    key_points: source.keyPoints,
    ...(source.corrects ? { corrects: source.corrects } : {}),
    source_url: source.sourceUrl,
  };
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "research", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json(profile);
    }
    if (request.method === "GET" && url.pathname === "/v1/sources") {
      return json({ sources: sources.map(sourceResponse), next_page_token: null });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic research API listening on http://127.0.0.1:${server.port}`);
