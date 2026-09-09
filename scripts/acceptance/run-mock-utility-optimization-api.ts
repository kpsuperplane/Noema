#!/usr/bin/env bun

// Synthetic utility and internet service for PA-068.
// It cannot contact a provider, switch a real service, or move money.

const port = Number(process.env.NOEMA_UTILITY_OPTIMIZATION_PORT ?? "3790");
const fixtureVersion = "2026-09-09-utility-optimization-api-v1";

const profile = {
  case_id: "utility-optimization-001",
  owner_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  time_zone: "America/Los_Angeles",
  service_address: "Jordan's synthetic home",
  project_label: "Electricity and internet plan switch",
  goal: "Compare three bundled electricity and internet plans for a full year and switch only when the plan meets the reliability preference and stays within the budget.",
  decision_boundary: "Do not contact a real utility or internet provider, change a real service, expose a real address, authorize a real charge, or move money.",
  budget_limit_usd: 2600,
  required_switch_date: "2026-10-01",
  reliability_preference: "Require at least 99.95% uptime, no data cap, and predictable bills; do not trade these requirements for a lower price.",
  source_locator: "utility://profiles/utility-optimization-001",
};

const usage = [
  ["2025-10", 540], ["2025-11", 590], ["2025-12", 640],
  ["2026-01", 650], ["2026-02", 610], ["2026-03", 580],
  ["2026-04", 520], ["2026-05", 500], ["2026-06", 560],
  ["2026-07", 690], ["2026-08", 710], ["2026-09", 620],
].map(([month, kwh]) => ({
  month,
  kwh,
  source_locator: `utility://usage/${month}`,
}));

const plans = [
  {
    plan_id: "plan-001",
    plan_name: "CivicGrid Secure + Fiber (synthetic)",
    provider_label: "CivicGrid Electric and HomeNet (synthetic)",
    electric_rate_usd_per_kwh: 0.19,
    electric_base_monthly_usd: 22,
    internet_standard_monthly_usd: 70,
    internet_intro_monthly_usd: 45,
    internet_intro_months: 3,
    intro_expires_on: "2026-12-31",
    equipment_charge_usd: 0,
    reliability_uptime_percent: 99.95,
    data_cap_label: "none",
    billing_note: "Fixed internet price after the introductory period; router included.",
    plan_status: "available",
    source_locator: "utility://plans/plan-001",
  },
  {
    plan_id: "plan-002",
    plan_name: "BudgetSpark + StreamNet (synthetic)",
    provider_label: "BudgetSpark Electric and StreamNet (synthetic)",
    electric_rate_usd_per_kwh: 0.15,
    electric_base_monthly_usd: 18,
    internet_standard_monthly_usd: 60,
    internet_intro_monthly_usd: 35,
    internet_intro_months: 6,
    intro_expires_on: "2027-03-31",
    equipment_charge_usd: 180,
    reliability_uptime_percent: 99.5,
    data_cap_label: "1000 GB per month",
    billing_note: "The introductory price ends after six months; equipment is a one-time charge.",
    plan_status: "available",
    source_locator: "utility://plans/plan-002",
  },
  {
    plan_id: "plan-003",
    plan_name: "Cobalt Flex + FiberPlus (synthetic)",
    provider_label: "Cobalt Electric and FiberPlus (synthetic)",
    electric_rate_usd_per_kwh: 0.17,
    electric_base_monthly_usd: 25,
    internet_standard_monthly_usd: 65,
    internet_intro_monthly_usd: 65,
    internet_intro_months: 0,
    intro_expires_on: "none",
    equipment_charge_usd: 75,
    reliability_uptime_percent: 99.9,
    data_cap_label: "none",
    billing_note: "No introductory rate; equipment is a one-time charge.",
    plan_status: "available",
    source_locator: "utility://plans/plan-003",
  },
];

const currentService = {
  current_provider_label: "MetroCurrent Electric and HomeNet (synthetic)",
  current_plan_id: "current-001",
  current_monthly_total_usd: 214,
  early_exit_fee_usd: 120,
  projected_prorated_final_service_usd: 48.5,
  current_contract_end: "2027-02-28",
  switch_date: profile.required_switch_date,
  source_locator: "utility://current/current-001",
};

const constraints = {
  minimum_uptime_percent: 99.95,
  data_cap_requirement: "none",
  predictable_billing_required: "yes",
  budget_limit_usd: profile.budget_limit_usd,
  comparison_period_months: 12,
  usage_unit: "kWh",
  required_switch_date: profile.required_switch_date,
  source_locator: "utility://constraints/utility-optimization-001",
};

const requests: Array<{ method: string; path: string; body?: unknown }> = [];
let switched = false;
let switchSubmissionCount = 0;

const docs = `# Synthetic utility and internet optimization API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot contact a utility or
internet provider, change a real service, expose a real address, authorize a
real charge, or move money. Work only in one personal Noema workspace.

## Source records

- GET /v1/profile returns the owner, date, timezone, synthetic address,
  reliability preference, 2600 USD budget, switch date, and boundary.
- GET /v1/usage returns twelve monthly electricity readings from 2025-10
  through 2026-09. Their total is 7210 kWh.
- GET /v1/plans returns three bundled electricity and internet plans. Plan-001
  has a three-month introductory internet price, 99.95% uptime, and no cap.
  Plan-002 has a six-month introductory price, a 180 USD equipment charge,
  99.5% uptime, and a 1000 GB cap. Plan-003 has no introductory price, a 75
  USD equipment charge, 99.9% uptime, and no cap.
- GET /v1/current-service returns the current synthetic provider, a 120 USD
  early-exit fee, and a projected 48.50 USD prorated final service amount.
- GET /v1/constraints returns the 99.95% uptime minimum, no-cap requirement,
  predictable billing requirement, 12-month comparison period, and budget.

## Synthetic switch

- POST /v1/switch accepts only plan-001, switch date 2026-10-01, annual new-plan
  service total 2398.90 USD, projected old-provider final bill 168.50 USD
  (48.50 prorated service plus the 120 USD early-exit fee), annual switch total
  2567.40 USD, and the note "Synthetic switch only; no provider contact or real
  service change.".
- The first accepted request returns activation-001 and submission_count 1.
- Repeating the exact request is idempotent and keeps submission_count at one.

## Verification

- GET /v1/final-bill returns 409 until the synthetic switch is recorded. After
  switching, it returns final-bill-001 for the old provider, 48.50 USD
  prorated service, 120 USD early-exit fee, 168.50 USD final amount, and status
  finalized_in_synthetic_ledger.
- GET /v1/status returns activation-001, plan-001, the 2026-10-01 activation
  date, annual new-plan total 2398.90 USD, old-provider final bill 168.50 USD,
  annual switch total 2567.40 USD, 32.60 USD remaining under the 2600 USD
  budget, and status activated_in_synthetic_account.

There are no other routes. Do not propose a real provider switch, account
change, charge, or payment.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function switchResult() {
  return {
    activation_id: "activation-001",
    case_id: profile.case_id,
    selected_plan_id: "plan-001",
    activation_date: profile.required_switch_date,
    annual_new_plan_total_usd: 2398.9,
    old_provider_final_bill_usd: 168.5,
    annual_switch_total_usd: 2567.4,
    status: "activated_in_synthetic_account",
    submission_count: switchSubmissionCount,
    source_locator: "utility://switches/activation-001",
  };
}

function finalBillResult() {
  return {
    bill_id: "final-bill-001",
    provider_label: currentService.current_provider_label,
    current_plan_id: currentService.current_plan_id,
    bill_date: "2026-09-30",
    prorated_service_usd: 48.5,
    early_exit_fee_usd: 120,
    final_amount_usd: 168.5,
    status: switched ? "finalized_in_synthetic_ledger" : "pending_synthetic_switch",
    submission_count: switched ? 1 : 0,
    source_locator: "utility://bills/final-bill-001",
  };
}

function statusResult() {
  return {
    case_id: profile.case_id,
    activation_id: switched ? "activation-001" : "none",
    selected_plan_id: switched ? "plan-001" : "none",
    activation_date: switched ? profile.required_switch_date : "none",
    annual_new_plan_total_usd: switched ? 2398.9 : 0,
    old_provider_bill_id: switched ? "final-bill-001" : "none",
    old_provider_final_bill_usd: switched ? 168.5 : 0,
    annual_switch_total_usd: switched ? 2567.4 : 0,
    budget_limit_usd: profile.budget_limit_usd,
    budget_remaining_usd: switched ? 32.6 : profile.budget_limit_usd,
    project_status: switched ? "activated_in_synthetic_account" : "ready_for_comparison",
    switch_submission_count: switchSubmissionCount,
    source_locator: "utility://status/utility-optimization-001",
  };
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
      return json({ ok: true, synthetic: true, service: "utility-optimization", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/usage") return json({ months: usage });
    if (request.method === "GET" && url.pathname === "/v1/plans") return json({ plans });
    if (request.method === "GET" && url.pathname === "/v1/current-service") return json(currentService);
    if (request.method === "GET" && url.pathname === "/v1/constraints") return json(constraints);
    if (request.method === "POST" && url.pathname === "/v1/switch") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.case_id !== "utility-optimization-001" ||
        value.selected_plan_id !== "plan-001" ||
        value.switch_date !== "2026-10-01" ||
        value.annual_new_plan_total_usd !== 2398.9 ||
        value.old_provider_final_bill_usd !== 168.5 ||
        value.annual_switch_total_usd !== 2567.4 ||
        value.approval_note !== "Synthetic switch only; no provider contact or real service change."
      ) return json({ error: "switch must use the exact approved synthetic plan and totals" }, 400);
      if (!switched) {
        switched = true;
        switchSubmissionCount = 1;
      }
      return json(switchResult(), switchSubmissionCount === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/final-bill") {
      if (!switched) return json({ error: "synthetic switch is required first" }, 409);
      return json(finalBillResult());
    }
    if (request.method === "GET" && url.pathname === "/v1/status") return json(statusResult());
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic utility-optimization API listening on http://127.0.0.1:${server.port}`);
