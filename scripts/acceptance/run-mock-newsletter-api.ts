#!/usr/bin/env bun

// Synthetic newsletter service for PA-038.
// It has no subscription, mailbox, publisher, or real user account.

const port = Number(process.env.NOEMA_NEWSLETTER_PORT ?? "3760");
const fixtureVersion = "2026-09-08-newsletter-api-v1";

type Edition = "baseline" | "updated";
type Story = {
  newsletter_id: string;
  story_id: string;
  edition: Edition;
  title: string;
  topic: string;
  summary: string;
  reading_minutes: number;
  published_at: string;
  source_url: string;
};

const profile = {
  newsletter_name: "Daily Signal newsletters",
  editions: ["baseline", "updated"],
  preferred_topics: ["technology", "design"],
  reading_budget_minutes: 10,
  deduplication_rule:
    "Rows with the same story_id are one story. Keep the newest summary and one original source link.",
  ranking_rule:
    "Prioritize preferred topics, then newer publication dates, while keeping the selected stories within the reading budget.",
  change_rule:
    "For the updated edition, report only new stories or stories whose title, summary, or source changed. Suppress unchanged stories.",
};

const baseline: Story[] = [
  {
    newsletter_id: "signal-morning",
    story_id: "story-tech-001",
    edition: "baseline",
    title: "Small models move more work onto personal devices",
    topic: "technology",
    summary: "A field report compares compact language models that run locally with lower latency and lower data exposure.",
    reading_minutes: 2,
    published_at: "2026-09-01T07:00:00Z",
    source_url: "https://news.example.test/story-tech-001",
  },
  {
    newsletter_id: "signal-weekly",
    story_id: "story-tech-002",
    edition: "baseline",
    title: "Open protocols make personal data easier to move",
    topic: "technology",
    summary: "A standards review explains how portable permissions and export formats reduce lock-in between personal tools.",
    reading_minutes: 3,
    published_at: "2026-09-02T08:00:00Z",
    source_url: "https://news.example.test/story-tech-002",
  },
  {
    newsletter_id: "design-roundup",
    story_id: "story-design-001",
    edition: "baseline",
    title: "Dense product screens can remain accessible",
    topic: "design",
    summary: "A design study shows how grouping, keyboard order, and clear status text support dense work interfaces.",
    reading_minutes: 2,
    published_at: "2026-09-02T09:00:00Z",
    source_url: "https://news.example.test/story-design-001",
  },
  {
    newsletter_id: "wellbeing-letter",
    story_id: "story-health-001",
    edition: "baseline",
    title: "What consistent sleep timing changes",
    topic: "health",
    summary: "A five-minute evidence review describes the limits and benefits of keeping a regular sleep schedule.",
    reading_minutes: 5,
    published_at: "2026-09-02T10:00:00Z",
    source_url: "https://news.example.test/story-health-001",
  },
  {
    newsletter_id: "money-brief",
    story_id: "story-finance-001",
    edition: "baseline",
    title: "Why short-term rates stayed higher",
    topic: "finance",
    summary: "An explainer separates the latest rate decision from longer-term borrowing cost forecasts.",
    reading_minutes: 4,
    published_at: "2026-09-03T07:30:00Z",
    source_url: "https://news.example.test/story-finance-001",
  },
  {
    newsletter_id: "roaming-notes",
    story_id: "story-travel-001",
    edition: "baseline",
    title: "Night trains add a practical route",
    topic: "travel",
    summary: "A route guide compares overnight rail connections and the tradeoff between time, cost, and transfers.",
    reading_minutes: 3,
    published_at: "2026-09-03T09:00:00Z",
    source_url: "https://news.example.test/story-travel-001",
  },
  {
    newsletter_id: "signal-evening",
    story_id: "story-tech-001",
    edition: "baseline",
    title: "Small models move more work onto personal devices",
    topic: "technology",
    summary: "A field report compares compact language models that run locally with lower latency and lower data exposure.",
    reading_minutes: 2,
    published_at: "2026-09-01T07:00:00Z",
    source_url: "https://news.example.test/story-tech-001-copy",
  },
  {
    newsletter_id: "builders-letter",
    story_id: "story-tech-003",
    edition: "baseline",
    title: "Local-first sync without silent conflicts",
    topic: "technology",
    summary: "A practical article describes conflict displays and recovery steps for apps that work offline first.",
    reading_minutes: 2,
    published_at: "2026-09-04T08:00:00Z",
    source_url: "https://news.example.test/story-tech-003",
  },
  {
    newsletter_id: "design-roundup",
    story_id: "story-design-001",
    edition: "baseline",
    title: "Dense product screens can remain accessible",
    topic: "design",
    summary: "A design study shows how grouping, keyboard order, and clear status text support dense work interfaces.",
    reading_minutes: 2,
    published_at: "2026-09-02T09:00:00Z",
    source_url: "https://news.example.test/story-design-001-copy",
  },
  {
    newsletter_id: "design-roundup",
    story_id: "story-design-002",
    edition: "baseline",
    title: "Why calm defaults improve form completion",
    topic: "design",
    summary: "A usability test links clear defaults and short error messages to fewer abandoned forms.",
    reading_minutes: 3,
    published_at: "2026-09-04T09:30:00Z",
    source_url: "https://news.example.test/story-design-002",
  },
  {
    newsletter_id: "money-brief",
    story_id: "story-finance-001",
    edition: "baseline",
    title: "Why short-term rates stayed higher",
    topic: "finance",
    summary: "An explainer separates the latest rate decision from longer-term borrowing cost forecasts.",
    reading_minutes: 4,
    published_at: "2026-09-03T07:30:00Z",
    source_url: "https://news.example.test/story-finance-001-copy",
  },
  {
    newsletter_id: "wellbeing-letter",
    story_id: "story-health-002",
    edition: "baseline",
    title: "Small breaks protect afternoon focus",
    topic: "health",
    summary: "A workplace study reports modest focus gains from short planned breaks, with no claim of a universal schedule.",
    reading_minutes: 4,
    published_at: "2026-09-04T11:00:00Z",
    source_url: "https://news.example.test/story-health-002",
  },
];

const updated: Story[] = [
  ...baseline.map((story) =>
    story.story_id === "story-tech-002"
      ? {
          ...story,
          edition: "updated" as const,
          summary:
            "A standards review now reports a ratified permission-transfer format and two tools that support it, reducing lock-in between personal tools.",
          published_at: "2026-09-08T08:00:00Z",
        }
      : { ...story, edition: "updated" as const },
  ),
  {
    newsletter_id: "design-roundup",
    story_id: "story-design-003",
    edition: "updated",
    title: "Design tokens now carry contrast intent",
    topic: "design",
    summary: "A new case study shows how semantic contrast tokens keep themes readable as products change.",
    reading_minutes: 2,
    published_at: "2026-09-08T09:30:00Z",
    source_url: "https://news.example.test/story-design-003",
  },
];

const docs = `# Synthetic newsletter API

Fixture version: ${fixtureVersion}

This deterministic, read-only service contains twelve baseline newsletter rows
covering five topics. Several rows repeat a story across newsletters. The
updated edition repeats the unchanged rows, revises one story, and adds one
new story. It has no subscription, mailbox, publisher, or real account.

## Operations

- \`GET /v1/profile\`: return the preferred topics, ten-minute reading budget,
  comparison editions, deduplication rule, ranking rule, and change rule.
- \`GET /v1/newsletters?edition=baseline\` or
  \`GET /v1/newsletters?edition=updated\`: return the requested edition in a
  \`stories\` array. Each story has newsletter_id, stable story_id, edition,
  title, topic, summary, reading_minutes, published_at, and source_url.

The baseline has twelve rows and nine unique story IDs. Rows with the same
story_id are duplicates even when their newsletter or source URL differs. The
preferred topics are technology and design. Select stories in preference and
recency order without exceeding ten minutes, and link each selected original.

The updated edition contains every baseline story plus a revised summary and
publication date for story-tech-002 and one new story, story-design-003. All
other story content is unchanged. When checking the updated edition, report
only those changed or new stories. The service has no write operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "newsletter", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/newsletters") {
      const edition = url.searchParams.get("edition");
      if (edition === "baseline") return json({ stories: baseline, next_page_token: null });
      if (edition === "updated") return json({ stories: updated, next_page_token: null });
      return json({ error: "edition must be baseline or updated" }, 400);
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic newsletter API listening on http://127.0.0.1:${server.port}`);
