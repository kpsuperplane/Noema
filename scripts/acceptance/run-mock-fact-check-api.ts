#!/usr/bin/env bun

// Synthetic fact-check service for PA-035.
// It has no connection to a real regulator, utility, or household.

const port = Number(process.env.NOEMA_FACT_CHECK_PORT ?? "3756");
const fixtureVersion = "2026-09-08-fact-check-api-v1";

type Claim = {
  id: string;
  text: string;
  sourceIds: string[];
};

type PrimarySource = {
  id: string;
  title: string;
  publisher: string;
  publishedAt: string;
  status: "current" | "superseded";
  summary: string;
  keyPoints: string[];
  sourceUrl: string;
};

const profile = {
  question: "Check five claims about the Northstar 2026 heat-pump rebate program and explain what is uncertain.",
  scope: "Current rules, historical primary documents, program definitions, and published monitoring plans.",
  claim_ids: ["claim-001", "claim-002", "claim-003", "claim-004", "claim-005"],
  review_requirements: [
    "Classify each claim only when the primary records support that classification.",
    "Cite the primary source ID and publication date for each conclusion.",
    "Keep an imprecise definition visible when the records do not define it.",
    "Do not treat an absent forecast or result as proof that the claim is false.",
  ],
};

const claims: Claim[] = [
  {
    id: "claim-001",
    text: "The Northstar heat-pump rebate application window runs from September 1 through December 31, 2026.",
    sourceIds: ["source-001"],
  },
  {
    id: "claim-002",
    text: "Every Northstar household can receive a $2,000 rebate.",
    sourceIds: ["source-001", "source-002"],
  },
  {
    id: "claim-003",
    text: "The Northstar rebate pays 30% of eligible equipment cost.",
    sourceIds: ["source-001", "source-003"],
  },
  {
    id: "claim-004",
    text: "The Northstar rebate is available to low-income households.",
    sourceIds: ["source-001", "source-004"],
  },
  {
    id: "claim-005",
    text: "The Northstar rebate will reduce regional electricity demand by 15% in 2026.",
    sourceIds: ["source-001", "source-005"],
  },
];

const sources: PrimarySource[] = [
  {
    id: "source-001",
    title: "Northstar Heat-Pump Rebate Program — Final Rules",
    publisher: "Northstar Energy Office",
    publishedAt: "2026-08-20",
    status: "current",
    summary: "The final rules set the current application window, equipment-cost rate, and income-tier caps for the rebate program.",
    keyPoints: [
      "Applications open 2026-09-01 and close 2026-12-31.",
      "The rebate rate is 40% of eligible equipment cost.",
      "Households at or below 80% of area median income have a $2,500 maximum; households above 80% have a $1,500 maximum.",
      "The rules describe administration and eligibility but provide no regional electricity-demand forecast.",
    ],
    sourceUrl: "https://factcheck.example.test/northstar/final-rules",
  },
  {
    id: "source-002",
    title: "Correction to the Northstar Rebate Cap Table",
    publisher: "Northstar Energy Office",
    publishedAt: "2026-08-25",
    status: "current",
    summary: "The Energy Office corrected the printed cap table and stated which parts of the final rules remain unchanged.",
    keyPoints: [
      "The corrected maximum is $2,500 at or below 80% of area median income and $1,500 above it.",
      "There is no single $2,000 maximum for every household.",
      "The 40% rate and application window remain unchanged.",
    ],
    sourceUrl: "https://factcheck.example.test/northstar/correction-cap-table",
  },
  {
    id: "source-003",
    title: "Northstar Rebate — Draft Proposal",
    publisher: "Northstar Energy Office",
    publishedAt: "2026-07-30",
    status: "superseded",
    summary: "This earlier primary proposal preceded the final rules and is retained to identify statements that are no longer current.",
    keyPoints: [
      "The draft proposed a 30% equipment-cost rate.",
      "The draft proposed a $1,000 maximum rebate.",
      "The final rules replaced this proposal before applications opened.",
    ],
    sourceUrl: "https://factcheck.example.test/northstar/draft-proposal",
  },
  {
    id: "source-004",
    title: "Northstar Rebate Eligibility Glossary",
    publisher: "Northstar Energy Office",
    publishedAt: "2026-08-22",
    status: "current",
    summary: "The glossary defines the income measure used by the program and explains that common descriptive labels are not eligibility fields.",
    keyPoints: [
      "The program uses household area median income bands, including a band at or below 80% of area median income.",
      "The phrase `low-income` is not defined as a program field in this glossary.",
      "A reviewer must state which definition is being used before treating `low-income` as a precise eligibility claim.",
    ],
    sourceUrl: "https://factcheck.example.test/northstar/eligibility-glossary",
  },
  {
    id: "source-005",
    title: "Northstar Rebate Program Monitoring Plan",
    publisher: "Northstar Energy Office",
    publishedAt: "2026-08-20",
    status: "current",
    summary: "The monitoring plan lists participation and installation measures but does not publish a regional electricity-demand forecast.",
    keyPoints: [
      "The plan tracks applications, approved installations, and rebate amounts.",
      "It does not estimate a 2026 regional electricity-demand reduction.",
      "The absence of a demand forecast does not establish whether a future reduction will or will not occur.",
    ],
    sourceUrl: "https://factcheck.example.test/northstar/monitoring-plan",
  },
];

const docs = `# Synthetic fact-check API

Fixture version: ${fixtureVersion}

This deterministic service contains five claims and five dated primary
records about a synthetic rebate program. It has no connection to a regulator,
utility, household, or real account. Every record is test data. The base URL
is the URL that served this document.

## Operations

- \`GET /v1/profile\`: return the review question, scope, claim IDs, and review requirements.
- \`GET /v1/claims\`: return the five claims and the primary source IDs linked to each claim.
- \`GET /v1/sources\`: return the dated primary records, source status, key points, and source URLs.

The claims and source records are bounded. Responses use JSON with an explicit
null next-page token. Source dates and status fields are part of the records.
The service is read-only and has no write operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "fact-check", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json(profile);
    }
    if (request.method === "GET" && url.pathname === "/v1/claims") {
      return json({
        claims: claims.map((claim) => ({ claim_id: claim.id, claim: claim.text, source_ids: claim.sourceIds })),
        next_page_token: null,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/sources") {
      return json({
        sources: sources.map((source) => ({
          source_id: source.id,
          title: source.title,
          publisher: source.publisher,
          published_at: source.publishedAt,
          status: source.status,
          summary: source.summary,
          key_points: source.keyPoints,
          source_url: source.sourceUrl,
        })),
        next_page_token: null,
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic fact-check API listening on http://127.0.0.1:${server.port}`);
