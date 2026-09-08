#!/usr/bin/env bun

// Synthetic topic-monitoring service for PA-033.
// It has no connection to a utility, regulator, publisher, or real account.

const port = Number(process.env.NOEMA_MONITOR_PORT ?? "3752");
const stateFile = process.env.NOEMA_MONITOR_STATE_FILE ?? "/var/tmp/noema-pa033-state";
const fixtureVersion = "2026-09-08-topic-monitor-api-v1";

type Update = {
  id: string;
  title: string;
  publisher: string;
  publishedAt: string;
  updatedAt?: string;
  status: "current" | "commentary";
  summary: string;
  keyPoints: string[];
  sourceUrl: string;
  canonicalUrl: string;
};

const profile = {
  topic: "Riverbend residential electricity rate plan",
  household: "Apartment with electric heat",
  monthlyKwh: 900,
  alertThresholdUsd: 10,
  preference: "Alert only when a confirmed change could raise the monthly bill by at least $10. Ignore repeated coverage and opinion.",
};

const baselineUpdates: Update[] = [
  {
    id: "update-001",
    title: "Riverbend winter residential rate plan",
    publisher: "Riverbend Energy Office",
    publishedAt: "2026-09-01",
    status: "current",
    summary: "The proposed winter residential supply rate is 12 cents per kWh, effective October 1, with no change to the delivery charge.",
    keyPoints: [
      "The residential supply rate is 12 cents per kWh.",
      "The rate is scheduled to apply from 2026-10-01 through 2027-03-31.",
      "The delivery charge and low-income credit are unchanged.",
    ],
    sourceUrl: "https://updates.example.test/riverbend/winter-rate-plan",
    canonicalUrl: "https://updates.example.test/riverbend/winter-rate-plan",
  },
  {
    id: "update-002",
    title: "What the Riverbend winter rate means for residents",
    publisher: "Riverbend Daily",
    publishedAt: "2026-09-02",
    status: "current",
    summary: "A local news summary repeats the 12-cent supply rate and October 1 start date.",
    keyPoints: [
      "The article repeats a 12-cent per kWh residential supply rate.",
      "It describes the October 1 start date from the Energy Office plan.",
      "It does not report a change to delivery charges or credits.",
    ],
    sourceUrl: "https://updates.example.test/riverbend/daily-rate-summary",
    canonicalUrl: "https://updates.example.test/riverbend/daily-rate-summary",
  },
  {
    id: "update-003",
    title: "Riverbend bill-credit FAQ",
    publisher: "Riverbend Energy Office",
    publishedAt: "2026-09-03",
    status: "current",
    summary: "The office FAQ says the existing low-income bill credit remains available during the winter rate period.",
    keyPoints: [
      "The existing low-income bill credit remains available.",
      "The FAQ does not change the residential supply rate.",
    ],
    sourceUrl: "https://updates.example.test/riverbend/bill-credit-faq",
    canonicalUrl: "https://updates.example.test/riverbend/bill-credit-faq",
  },
];

async function readPhase() {
  const file = Bun.file(stateFile);
  const text = (await file.exists()) ? await file.text() : "baseline";
  const phase = text.trim();
  return phase === "duplicate" || phase === "changed" ? phase : "baseline";
}

function currentUpdates(phase: string): Update[] {
  if (phase === "baseline") return baselineUpdates;

  const duplicate: Update = {
    ...baselineUpdates[1],
    id: "update-004",
    title: "What the Riverbend winter rate means for residents",
    publishedAt: "2026-09-02",
    updatedAt: "2026-09-04",
  };
  if (phase === "duplicate") {
    return [baselineUpdates[0], baselineUpdates[1], duplicate, baselineUpdates[2]];
  }

  const changedOfficial: Update = {
    ...baselineUpdates[0],
    updatedAt: "2026-09-05",
    summary: "The Energy Office approved a 14 cents per kWh residential supply rate, effective October 1, with no change to the delivery charge.",
    keyPoints: [
      "The approved residential supply rate is 14 cents per kWh.",
      "The rate is scheduled to apply from 2026-10-01 through 2027-03-31.",
      "The delivery charge and low-income credit are unchanged.",
    ],
  };
  return [changedOfficial, baselineUpdates[1], duplicate, baselineUpdates[2]];
}

const docs = `# Synthetic topic monitoring API

Fixture version: ${fixtureVersion}

This deterministic service contains a synthetic topic profile and source
updates. It has no connection to a utility, regulator, publisher, or real
account. Every record is test data. The base URL is the URL that served this
document.

## Operations

- \`GET /v1/profile\`: return the person's topic, household context, usage, and alert preference.
- \`GET /v1/updates\`: return dated source updates with identity, publisher, status, summaries, key points, and canonical URLs.

Sources can repeat coverage of the same canonical article. Source records can
also receive an updated timestamp and revised facts. Compare identity,
canonical URL, dates, status, and content before reporting a material change.
The service is read-only and has no notification or write operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function updateResponse(update: Update) {
  return {
    id: update.id,
    title: update.title,
    publisher: update.publisher,
    published_at: update.publishedAt,
    ...(update.updatedAt ? { updated_at: update.updatedAt } : {}),
    status: update.status,
    summary: update.summary,
    key_points: update.keyPoints,
    source_url: update.sourceUrl,
    canonical_url: update.canonicalUrl,
  };
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "topic-monitor", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json({
        topic: profile.topic,
        household: profile.household,
        monthly_kwh: profile.monthlyKwh,
        alert_threshold_usd: profile.alertThresholdUsd,
        preference: profile.preference,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/updates") {
      const phase = await readPhase();
      return json({ updates: currentUpdates(phase).map(updateResponse), next_page_token: null });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic topic-monitor API listening on http://127.0.0.1:${server.port}`);
