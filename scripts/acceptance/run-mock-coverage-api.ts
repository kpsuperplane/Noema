#!/usr/bin/env bun

// Synthetic coverage service for PA-044.
// It has no connection to an insurer, broker, claim system, or real policy.

const port = Number(process.env.NOEMA_COVERAGE_PORT ?? "3766");
const fixtureVersion = "2026-09-08-coverage-api-v1";

type Coverage = {
  coverage_name: string;
  limit: number;
  deductible: number;
  unit: "USD";
  source_locator: string;
};

type Policy = {
  policy_id: string;
  policy_type: "home" | "auto";
  provider_name: string;
  status: "active";
  effective_date: string;
  renewal_date: string;
  premium_annual: number;
  currency: "USD";
  coverages: Coverage[];
  exclusions: string[];
  source_locator: string;
};

type Asset = {
  asset_id: string;
  name: string;
  category: string;
  replacement_value: number;
  currency: "USD";
  policy_id: string;
  scheduled_on_policy: boolean;
  coverage_note: string;
  source_locator: string;
};

const profile = {
  account_id: "account-coverage-001",
  account_label: "Jordan Lee household (synthetic)",
  as_of: "2026-09-08",
  priorities: [
    "Keep liability protection adequate for the household.",
    "Identify coverage gaps before any renewal change.",
    "Ask a licensed insurance professional before binding or changing coverage.",
  ],
  source_locator: "coverage://accounts/account-coverage-001/profile",
};

const policies: Policy[] = [
  {
    policy_id: "policy-home-001",
    policy_type: "home",
    provider_name: "Northstar Mutual",
    status: "active",
    effective_date: "2026-01-15",
    renewal_date: "2027-01-15",
    premium_annual: 1680,
    currency: "USD",
    coverages: [
      { coverage_name: "dwelling", limit: 450000, deductible: 2500, unit: "USD", source_locator: "coverage://policies/policy-home-001/dwelling" },
      { coverage_name: "personal_property", limit: 225000, deductible: 2500, unit: "USD", source_locator: "coverage://policies/policy-home-001/personal-property" },
      { coverage_name: "personal_liability", limit: 300000, deductible: 2500, unit: "USD", source_locator: "coverage://policies/policy-home-001/liability" },
      { coverage_name: "water_backup", limit: 10000, deductible: 2500, unit: "USD", source_locator: "coverage://policies/policy-home-001/water-backup" },
    ],
    exclusions: ["flood", "earthquake", "home-business equipment"],
    source_locator: "coverage://policies/policy-home-001",
  },
  {
    policy_id: "policy-auto-001",
    policy_type: "auto",
    provider_name: "Harborline Auto",
    status: "active",
    effective_date: "2026-03-01",
    renewal_date: "2027-03-01",
    premium_annual: 1340,
    currency: "USD",
    coverages: [
      { coverage_name: "bodily_injury_liability", limit: 500000, deductible: 0, unit: "USD", source_locator: "coverage://policies/policy-auto-001/bodily-injury" },
      { coverage_name: "property_damage_liability", limit: 100000, deductible: 0, unit: "USD", source_locator: "coverage://policies/policy-auto-001/property-damage" },
      { coverage_name: "collision", limit: 42000, deductible: 1000, unit: "USD", source_locator: "coverage://policies/policy-auto-001/collision" },
      { coverage_name: "comprehensive", limit: 42000, deductible: 500, unit: "USD", source_locator: "coverage://policies/policy-auto-001/comprehensive" },
      { coverage_name: "uninsured_motorist", limit: 500000, deductible: 0, unit: "USD", source_locator: "coverage://policies/policy-auto-001/uninsured-motorist" },
    ],
    exclusions: ["commercial use", "rideshare without endorsement"],
    source_locator: "coverage://policies/policy-auto-001",
  },
];

const assets: Asset[] = [
  {
    asset_id: "asset-home-001",
    name: "Primary residence",
    category: "home",
    replacement_value: 450000,
    currency: "USD",
    policy_id: "policy-home-001",
    scheduled_on_policy: true,
    coverage_note: "Dwelling limit matches the policy record.",
    source_locator: "coverage://assets/asset-home-001",
  },
  {
    asset_id: "asset-camera-001",
    name: "Professional camera kit",
    category: "camera-equipment",
    replacement_value: 4200,
    currency: "USD",
    policy_id: "policy-home-001",
    scheduled_on_policy: false,
    coverage_note: "The household inventory lists this item, but the home policy has no scheduled camera endorsement.",
    source_locator: "coverage://assets/asset-camera-001",
  },
  {
    asset_id: "asset-auto-001",
    name: "2024 Nimbus hatchback",
    category: "vehicle",
    replacement_value: 42000,
    currency: "USD",
    policy_id: "policy-auto-001",
    scheduled_on_policy: true,
    coverage_note: "Vehicle is listed on the auto policy.",
    source_locator: "coverage://assets/asset-auto-001",
  },
];

const docs = `# Synthetic coverage API

Fixture version: ${fixtureVersion}

This deterministic service contains one synthetic household profile, one home
policy, one auto policy, and three household assets. It has no connection to an
insurer, broker, claim system, payment account, or real policy. All values are
test data in USD. The service is read-only.

## Operations

- GET /v1/profile returns account identity, review priorities, and as_of.
- GET /v1/policies returns the active home and auto policies. Each policy has
  policy_id, policy_type, provider_name, status, effective_date, renewal_date,
  premium_annual, currency, coverages, exclusions, and source_locator. Each
  coverage has coverage_name, limit, deductible, unit, and source_locator.
- GET /v1/assets returns household assets and whether each is scheduled on its
  related policy. Each asset has replacement_value, policy_id, coverage_note,
  and source_locator.

The professional camera kit is not scheduled on the home policy. The home
policy excludes flood, earthquake, and home-business equipment. The auto policy
excludes commercial use and rideshare without an endorsement. These are review
questions, not advice to bind or change a policy.

The API has no quote, bind, purchase, payment, claim, or policy-change route.
Do not propose or invent a write operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

const requests: Array<{ method: string; path: string }> = [];

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    requests.push({ method: request.method, path: `${url.pathname}${url.search}` });
    console.log(JSON.stringify(requests.at(-1)));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "coverage", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json(profile);
    }
    if (request.method === "GET" && url.pathname === "/v1/policies") {
      return json({ policies });
    }
    if (request.method === "GET" && url.pathname === "/v1/assets") {
      return json({ assets });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic coverage API listening on http://127.0.0.1:${server.port}`);
