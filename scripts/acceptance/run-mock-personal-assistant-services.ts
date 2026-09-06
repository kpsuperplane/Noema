#!/usr/bin/env bun

const port = Number(process.env.NOEMA_PERSONAL_ASSISTANT_MOCK_PORT ?? "3742");
const marker = "PA-REPLAY-20260906";
const runs = new Map<string, { mutations: string[] }>();

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}
function runFor(url: URL) {
  const run = url.searchParams.get("run");
  if (!run || !new RegExp(`^${marker}-\\d{3}$`).test(run)) return undefined;
  let state = runs.get(run);
  if (!state) { state = { mutations: [] }; runs.set(run, state); }
  return { run, state };
}
function readOnly(service: string, run: string, items: unknown[]) {
  return { service, run, synthetic: true, marker, items, nextPageToken: null };
}

const server = Bun.serve({ hostname: "127.0.0.1", port, async fetch(request) {
  const url = new URL(request.url);
  if (url.pathname === "/health") return json({ ok: true, marker, services: ["gmail", "calendar", "notion", "files", "transactions"] });
  const current = runFor(url);
  if (!current) return json({ error: "run must be a case id" }, 400);
  const { run, state } = current;
  if (request.method !== "GET") {
    state.mutations.push(`${request.method} ${url.pathname}`);
    return json({ accepted: true, synthetic: true, marker, run, receipt: `${marker}-receipt-${state.mutations.length}` });
  }
  if (url.pathname === "/fixture/state") return json({ marker, run, mutations: state.mutations, readOnly: true });
  if (url.pathname.startsWith("/gmail/")) return json(readOnly("gmail", run, [{ id: `${run}-message-001`, subject: "Synthetic commitment", labelIds: ["INBOX"] }]));
  if (url.pathname.startsWith("/calendar/")) return json(readOnly("calendar", run, [{ id: `${run}-event-001`, summary: "Synthetic appointment", start: "2026-09-07T09:00:00Z", end: "2026-09-07T10:00:00Z" }]));
  if (url.pathname.startsWith("/notion/")) return json(readOnly("notion", run, [{ id: `${run}-page-001`, object: "page", properties: { title: "Synthetic project record" } }]));
  if (url.pathname.startsWith("/files/")) return json(readOnly("files", run, [{ id: `${run}-file-001`, name: "synthetic-record.txt", content: `${marker} ${run}` }]));
  if (url.pathname.startsWith("/transactions/")) return json(readOnly("transactions", run, [{ id: `${run}-receipt-001`, status: "preview_only", amount: 0 }]));
  return json({ error: "unknown fixture route", marker, run }, 404);
  }
});

console.log(`personal assistant mock services listening on http://127.0.0.1:${server.port}`);
