#!/usr/bin/env bun

// Synthetic provider-comparison service for PA-058.
// It cannot diagnose, select care, contact a clinician, or book an appointment.

const port = Number(process.env.NOEMA_PROVIDER_COMPARISON_PORT ?? "3780");
const fixtureVersion = "2026-09-09-provider-comparison-api-v1";

const profile = {
  case_id: "provider-compare-001",
  current_date: "2026-09-09",
  goal: "Find suitable synthetic primary-care providers using explicit access, network, language, fee, and timing needs.",
  specialty: "Primary care (synthetic)",
  plan_name: "Harbor Health PPO (synthetic)",
  required_language: "Spanish",
  wheelchair_access_required: "accessible",
  max_fee_usd: 180,
  max_travel_minutes: 45,
  appointment_window: "2026-09-15 through 2026-09-30",
  decision_boundary:
    "Do not diagnose, select a clinician for the person, contact a provider, book an appointment, or make a payment.",
  source_locator: "provider://preferences/provider-compare-001",
};

const providers = [
  {
    provider_id: "provider-001",
    provider_name: "Northstar Family Clinic (synthetic)",
    specialty: "Primary care (synthetic)",
    network_status: "in_network",
    language_support: "Spanish, English",
    wheelchair_access: "accessible",
    fee_usd: 150,
    travel_minutes: 25,
    availability_status: "confirmed",
    next_available_date: "2026-09-16",
    quality_note: "Directory record has a current quality note; no clinical ranking is assigned.",
    source_locator: "provider://directory/provider-001",
  },
  {
    provider_id: "provider-002",
    provider_name: "Harbor Community Practice (synthetic)",
    specialty: "Primary care (synthetic)",
    network_status: "out_of_network",
    language_support: "Spanish, English",
    wheelchair_access: "accessible",
    fee_usd: 120,
    travel_minutes: 15,
    availability_status: "confirmed",
    next_available_date: "2026-09-15",
    quality_note: "Directory record has a current quality note; no clinical ranking is assigned.",
    source_locator: "provider://directory/provider-002",
  },
  {
    provider_id: "provider-003",
    provider_name: "Summit Primary Rooms (synthetic)",
    specialty: "Primary care (synthetic)",
    network_status: "in_network",
    language_support: "Spanish, English",
    wheelchair_access: "stairs_only",
    fee_usd: 160,
    travel_minutes: 20,
    availability_status: "confirmed",
    next_available_date: "2026-09-17",
    quality_note: "Directory record has a current quality note; no clinical ranking is assigned.",
    source_locator: "provider://directory/provider-003",
  },
  {
    provider_id: "provider-004",
    provider_name: "Riverside Access Clinic (synthetic)",
    specialty: "Primary care (synthetic)",
    network_status: "in_network",
    language_support: "Spanish, English",
    wheelchair_access: "accessible",
    fee_usd: 175,
    travel_minutes: 40,
    availability_status: "unknown",
    next_available_date: "not_verified",
    quality_note: "Directory record has a current quality note; no clinical ranking is assigned.",
    source_locator: "provider://directory/provider-004",
  },
];

const availability = {
  provider_id: "provider-004",
  availability_status: "confirmed",
  earliest_available_date: "2026-09-18",
  appointment_window: "2026-09-15 through 2026-09-30",
  verification_note: "The synthetic scheduling index confirmed one opening in the requested window.",
  source_locator: "provider://availability/provider-004",
};

const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic provider-comparison API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot diagnose, recommend a
clinician, contact a provider, book an appointment, change insurance, or move
money. All names, fees, dates, access labels, and availability are test data.
Every route is read-only.

## Search profile and constraints

- GET /v1/profile returns the specialty, plan, required language, wheelchair
  access requirement, maximum fee, travel limit, requested appointment window,
  decision boundary, and source locator.
- GET /v1/providers returns four provider records. Provider-002 is out of
  network and provider-003 has stairs only. Those are hard-constraint failures.
  Provider-001 is in network, Spanish-speaking, accessible, within the fee and
  travel limits, and has a confirmed date. Provider-004 is in network,
  Spanish-speaking, accessible, within the limits, but its availability is
  initially unknown.
- GET /v1/providers/{provider_id}/availability verifies one provider's
  availability. Use provider-004 after filtering the directory. The fixture
  confirms an opening on 2026-09-18 inside the requested window.

The fee is a directory estimate, not a bill. Network and availability are
source fields, not guarantees of clinical suitability. Leave the final choice
with the person.

There are no other routes. Do not propose booking, diagnosis, treatment,
provider contact, insurance changes, payment, or undocumented operations.
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
  async fetch(request) {
    const url = new URL(request.url);
    let body: unknown;
    if (request.method !== "GET") body = await request.json().catch(() => undefined);
    const entry = {
      method: request.method,
      path: `${url.pathname}${url.search}`,
      ...(body === undefined ? {} : { body }),
    };
    requests.push(entry);
    console.log(JSON.stringify(entry));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "provider-comparison", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/providers") return json({ providers });
    if (request.method === "GET" && url.pathname.startsWith("/v1/providers/") && url.pathname.endsWith("/availability")) {
      const providerId = url.pathname.slice("/v1/providers/".length, -"/availability".length);
      if (providerId !== availability.provider_id) return json({ error: "availability_not_available" }, 404);
      return json(availability);
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic provider-comparison API listening on http://127.0.0.1:${server.port}`);
