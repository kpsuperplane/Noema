#!/usr/bin/env bun

// Synthetic health-plan comparison service for PA-059.
// It cannot enroll a person, change coverage, submit a claim, or move money.

const port = Number(process.env.NOEMA_HEALTH_PLAN_COMPARISON_PORT ?? "3781");
const fixtureVersion = "2026-09-09-health-plan-comparison-api-v1";

const profile = {
  case_id: "health-plan-compare-001",
  patient_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  goal: "Compare two synthetic health plans for the supplied care and medication scenario.",
  household_size: 1,
  network_preference: "Use in-network care for the expected estimate.",
  scenario_period: "2027 calendar year",
  expected_cost_rule:
    "Expected annual member cost equals twelve months of premium plus covered service and prescription cost shares; apply the medical and prescription deductibles before coinsurance when a claim type uses them; add the supplied cash estimate for an excluded drug outside plan coverage.",
  worst_case_rule:
    "Worst-case annual exposure equals twelve months of premium plus the plan out-of-pocket maximum for covered in-network care; add excluded-drug cash cost because it is outside the plan maximum.",
  decision_boundary:
    "Do not enroll, cancel coverage, submit a claim, contact an insurer, give legal or medical advice, or choose a plan for the person.",
  source_locator: "plan://preferences/health-plan-compare-001",
};

const plans = [
  {
    plan_id: "plan-001",
    plan_name: "Harbor Standard PPO (synthetic)",
    premium_monthly_usd: 360,
    medical_deductible_usd: 1200,
    prescription_deductible_usd: 250,
    out_of_pocket_max_usd: 5000,
    network_scope: "in_network_only",
    out_of_network_cost_share: "not_covered",
    primary_care_copay_usd: 25,
    specialist_copay_usd: 60,
    urgent_care_copay_usd: 75,
    lab_coinsurance_percent: 20,
    generic_copay_usd: 10,
    preferred_brand_copay_usd: 35,
    specialty_copay_usd: 250,
    excluded_drug_id: "drug-003",
    excluded_drug_label: "Examplebiologic (synthetic)",
    excluded_drug_cash_price_usd: 1800,
    network_note:
      "The plan covers the expected estimate only for in-network care. Out-of-network care is not covered and may create full charges.",
    source_locator: "plan://plans/plan-001",
  },
  {
    plan_id: "plan-002",
    plan_name: "Harbor Choice EPO (synthetic)",
    premium_monthly_usd: 220,
    medical_deductible_usd: 2500,
    prescription_deductible_usd: 500,
    out_of_pocket_max_usd: 7500,
    network_scope: "in_network_with_no_out_of_network_benefit",
    out_of_network_cost_share: "not_covered",
    primary_care_copay_usd: 40,
    specialist_copay_usd: 45,
    urgent_care_copay_usd: 75,
    lab_coinsurance_percent: 30,
    generic_copay_usd: 5,
    preferred_brand_copay_usd: 25,
    specialty_copay_usd: 250,
    excluded_drug_id: "none",
    excluded_drug_label: "none",
    excluded_drug_cash_price_usd: 0,
    network_note:
      "The plan has no out-of-network benefit. The expected estimate assumes every service uses an in-network provider.",
    source_locator: "plan://plans/plan-002",
  },
];

const usage = [
  {
    usage_id: "visit-001",
    kind: "primary_care_visit",
    label: "Primary-care visit",
    units: 4,
    allowed_unit_cost_usd: 150,
    cost_share_rule: "primary_care_copay",
    drug_id: "none",
    drug_tier: "none",
    source_locator: "plan://scenario/visit-001",
  },
  {
    usage_id: "visit-002",
    kind: "specialist_visit",
    label: "Specialist visit",
    units: 2,
    allowed_unit_cost_usd: 220,
    cost_share_rule: "specialist_copay",
    drug_id: "none",
    drug_tier: "none",
    source_locator: "plan://scenario/visit-002",
  },
  {
    usage_id: "visit-003",
    kind: "urgent_care_visit",
    label: "Urgent-care visit",
    units: 1,
    allowed_unit_cost_usd: 300,
    cost_share_rule: "urgent_care_copay",
    drug_id: "none",
    drug_tier: "none",
    source_locator: "plan://scenario/visit-003",
  },
  {
    usage_id: "lab-001",
    kind: "lab_panel",
    label: "Laboratory panel",
    units: 2,
    allowed_unit_cost_usd: 100,
    cost_share_rule: "medical_deductible_then_coinsurance",
    drug_id: "none",
    drug_tier: "none",
    source_locator: "plan://scenario/lab-001",
  },
  {
    usage_id: "drug-001",
    kind: "prescription",
    label: "Examplestatin (synthetic)",
    units: 12,
    allowed_unit_cost_usd: 120,
    cost_share_rule: "prescription_generic",
    drug_id: "drug-001",
    drug_tier: "generic",
    source_locator: "plan://scenario/drug-001",
  },
  {
    usage_id: "drug-002",
    kind: "prescription",
    label: "Exampleinhaler (synthetic)",
    units: 12,
    allowed_unit_cost_usd: 210,
    cost_share_rule: "prescription_preferred_brand",
    drug_id: "drug-002",
    drug_tier: "preferred_brand",
    source_locator: "plan://scenario/drug-002",
  },
  {
    usage_id: "drug-003",
    kind: "prescription",
    label: "Examplebiologic (synthetic)",
    units: 4,
    allowed_unit_cost_usd: 1800,
    cost_share_rule: "prescription_specialty",
    drug_id: "drug-003",
    drug_tier: "specialty",
    source_locator: "plan://scenario/drug-003",
  },
];

const docs = `# Synthetic health-plan comparison API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot enroll or cancel coverage,
submit a claim, contact an insurer, give legal or medical advice, or move
money. All names, premiums, benefits, prices, and usage records are test data.
Every route is read-only.

## Profile and calculation rules

- GET /v1/profile returns the case, person label, scenario period, network
  preference, reproducible expected-cost rule, worst-case rule, decision
  boundary, and source locator.
- Expected annual member cost is twelve months of premium plus each covered
  service and prescription cost share. For a deductible-then-coinsurance item,
  apply the deductible first and the stated coinsurance to any remaining
  allowed amount. Add the supplied cash estimate for a drug excluded by a
  plan; that cash amount is not insurer cost sharing.
- Worst-case annual exposure is twelve months of premium plus the plan
  out-of-pocket maximum for covered in-network care. Add excluded-drug cash
  cost because an excluded drug is outside that maximum.

## Plan and scenario records

- GET /v1/plans returns two complete plan records. Preserve premiums,
  deductibles, out-of-pocket maxima, network scopes, copays, coinsurance,
  prescription tiers, excluded-drug fields, network notes, and locators.
- GET /v1/usage returns seven scenario records: four medical service records
  and three prescriptions. Preserve units, allowed costs, cost-share rules,
  drug IDs and tiers, and locators.
- The scenario uses in-network care. Harbor Standard PPO excludes
  Examplebiologic. Harbor Choice EPO covers it at its specialty copay but has
  no out-of-network benefit. Do not treat a network field as a guarantee of
  clinical suitability or a final enrollment recommendation.

There are no other routes. Do not propose enrollment, cancellation, claims,
insurer contact, payment, diagnosis, treatment, or undocumented operations.
`;

const requests: Array<{ method: string; path: string }> = [];

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
    const entry = { method: request.method, path: `${url.pathname}${url.search}` };
    requests.push(entry);
    console.log(JSON.stringify(entry));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "health-plan-comparison", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/plans") return json({ plans });
    if (request.method === "GET" && url.pathname === "/v1/usage") return json({ usage });
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic health-plan comparison API listening on http://127.0.0.1:${server.port}`);
