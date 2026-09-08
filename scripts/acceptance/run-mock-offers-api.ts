#!/usr/bin/env bun

// Synthetic offer service for PA-030.
// It has no connection to an employer, recruiter, payroll system, or bank.

const port = Number(process.env.NOEMA_OFFERS_PORT ?? "3748");
const fixtureVersion = "2026-09-08-offers-api-v1";

type Offer = {
  id: string;
  employer: string;
  role: string;
  baseSalary: number;
  bonus: { amount: number; status: "target_not_guaranteed" | "none" };
  equity: { description: string; estimatedValue: number | null; status: "uncertain" | "none" };
  commute: { mode: "hybrid" | "onsite"; oneWayMinutes: number; daysPerWeek: number };
  paidLeaveDays: number;
  remoteDaysPerWeek: number;
  sourceUrl: string;
};

const profile = {
  preferences: [
    "Prefer time outside work and predictable benefits over uncertain upside.",
    "Keep one-way commuting under 45 minutes when possible.",
    "Treat non-guaranteed bonus and unpriced equity as uncertain, not guaranteed compensation.",
  ],
  referenceWeeksPerYear: 48,
};

const offers: Offer[] = [
  {
    id: "offer-001",
    employer: "Harbor Systems",
    role: "Platform Engineer",
    baseSalary: 100000,
    bonus: { amount: 10000, status: "target_not_guaranteed" },
    equity: { description: "Estimated grant value is $30,000, subject to valuation and vesting.", estimatedValue: 30000, status: "uncertain" },
    commute: { mode: "hybrid", oneWayMinutes: 35, daysPerWeek: 3 },
    paidLeaveDays: 25,
    remoteDaysPerWeek: 2,
    sourceUrl: "https://offers.example.test/harbor/offer-001",
  },
  {
    id: "offer-002",
    employer: "Mosaic Health",
    role: "Customer Operations Program Manager",
    baseSalary: 108000,
    bonus: { amount: 0, status: "none" },
    equity: { description: "No equity grant is listed in the offer.", estimatedValue: null, status: "none" },
    commute: { mode: "onsite", oneWayMinutes: 15, daysPerWeek: 5 },
    paidLeaveDays: 15,
    remoteDaysPerWeek: 0,
    sourceUrl: "https://offers.example.test/mosaic/offer-002",
  },
];

const docs = `# Synthetic offers API

Fixture version: ${fixtureVersion}

This deterministic service contains two synthetic job offers and one synthetic
preference profile. It has no connection to an employer, recruiter, payroll
system, bank, or hiring system. Every record is test data. The base URL is the
URL that served this document.

## Operations

- \`GET /v1/profile\`: return the person's comparison preferences and the
  reference value of 48 working weeks per year.
- \`GET /v1/offers\`: return both offers. Each offer includes base salary,
  bonus status, equity uncertainty, commute pattern, paid leave, remote days,
  and a source URL. Values are annual unless stated otherwise.

The target bonus on Harbor Systems is not guaranteed. Its estimated equity
value is uncertain because the valuation and vesting outcome are not supplied.
Do not add either value to guaranteed compensation. Mosaic Health lists no
bonus or equity. The service is read-only and returns JSON with an explicit
null next-page token.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function offerResponse(offer: Offer) {
  return {
    id: offer.id,
    employer: offer.employer,
    role: offer.role,
    base_salary: offer.baseSalary,
    bonus: { amount: offer.bonus.amount, status: offer.bonus.status },
    equity: {
      description: offer.equity.description,
      estimated_value: offer.equity.estimatedValue,
      status: offer.equity.status,
    },
    commute: {
      mode: offer.commute.mode,
      one_way_minutes: offer.commute.oneWayMinutes,
      days_per_week: offer.commute.daysPerWeek,
    },
    paid_leave_days: offer.paidLeaveDays,
    remote_days_per_week: offer.remoteDaysPerWeek,
    source_url: offer.sourceUrl,
  };
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "offers", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, { headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" } });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json({ preferences: profile.preferences, reference_weeks_per_year: profile.referenceWeeksPerYear });
    }
    if (request.method === "GET" && url.pathname === "/v1/offers") {
      return json({ offers: offers.map(offerResponse), next_page_token: null });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic offers API listening on http://127.0.0.1:${server.port}`);
