#!/usr/bin/env bun

// Synthetic home-maintenance service for PA-066.
// It cannot dispatch a real technician, charge money, or change a real home.

const port = Number(process.env.NOEMA_HOME_MAINTENANCE_PORT ?? "3788");
const fixtureVersion = "2026-09-09-home-maintenance-api-v1";

const profile = {
  case_id: "home-maintenance-001",
  owner_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  time_zone: "America/Los_Angeles",
  property_label: "Jordan's synthetic home",
  goal: "Keep the HVAC system on its manual interval, inside a 300 USD service budget, and outside the heat-advisory window.",
  decision_boundary: "Do not dispatch a real technician, charge money, authorize a real repair, or contact a real provider.",
  service_budget_usd: 300,
  source_locator: "home://profiles/home-maintenance-001",
};

const asset = {
  asset_id: "hvac-001",
  system_label: "Northwind QuietCool 24 (synthetic)",
  manufacturer: "Northwind",
  model: "QuietCool 24",
  serial_number: "NW-QC24-2023-4488",
  installed_on: "2023-09-12",
  source_locator: "home://assets/hvac-001",
};

const manual = {
  manual_id: "manual-hvac-001",
  asset_id: "hvac-001",
  manufacturer: "Northwind",
  model: "QuietCool 24",
  service_interval_days: 180,
  filter_interval_days: 90,
  recommended_months: "March and September",
  service_duration_minutes: 90,
  safety_note: "Use a qualified technician for electrical and refrigerant work; this synthetic fixture does not dispatch one.",
  source_locator: "home://manuals/manual-hvac-001",
};

const warranty = {
  warranty_id: "warranty-hvac-001",
  asset_id: "hvac-001",
  provider_label: "Northwind warranty desk (synthetic)",
  starts_on: "2023-09-12",
  expires_on: "2027-09-12",
  coverage_summary: "Annual tune-up labor and covered parts; replacement filters are excluded.",
  serial_match: "exact",
  source_locator: "home://warranties/warranty-hvac-001",
};

const lastService = {
  receipt_id: "service-receipt-000",
  asset_id: "hvac-001",
  service_date: "2026-03-12",
  vendor_label: "CoolAir Service (synthetic)",
  service_type: "seasonal_tune_up",
  amount_usd: 160,
  notes: "Synthetic baseline service receipt; no open defect was recorded.",
  source_locator: "home://service-receipts/service-receipt-000",
};

const constraints = {
  safe_service_window_start: "2026-09-12",
  safe_service_window_end: "2026-09-30",
  avoid_dates: "2026-09-10, 2026-09-11",
  avoid_reason: "Synthetic heat-advisory window",
  quiet_hours: "20:00-08:00 America/Los_Angeles",
  access_window: "Weekdays 09:00-17:00 America/Los_Angeles",
  budget_limit_usd: 300,
  budget_scope: "The 300 USD limit applies to this planned service; the March baseline receipt is historical context.",
  source_locator: "home://constraints/home-maintenance-001",
};

const quotes = [
  {
    quote_id: "quote-001",
    vendor_label: "CoolAir Service (synthetic)",
    asset_id: "hvac-001",
    offered_date: "2026-09-12",
    offered_start_time: "09:00",
    duration_minutes: 90,
    service_type: "seasonal_tune_up",
    amount_usd: 185,
    includes_filter: "yes",
    warranty_eligible: "yes",
    quote_status: "feasible",
    source_locator: "home://quotes/quote-001",
  },
  {
    quote_id: "quote-002",
    vendor_label: "Northwind Certified (synthetic)",
    asset_id: "hvac-001",
    offered_date: "2026-09-15",
    offered_start_time: "13:00",
    duration_minutes: 90,
    service_type: "seasonal_tune_up",
    amount_usd: 260,
    includes_filter: "no",
    warranty_eligible: "yes",
    quote_status: "feasible",
    source_locator: "home://quotes/quote-002",
  },
  {
    quote_id: "quote-003",
    vendor_label: "Budget HVAC (synthetic)",
    asset_id: "hvac-001",
    offered_date: "2026-09-11",
    offered_start_time: "10:00",
    duration_minutes: 90,
    service_type: "seasonal_tune_up",
    amount_usd: 140,
    includes_filter: "yes",
    warranty_eligible: "unknown",
    quote_status: "outside_safe_window",
    source_locator: "home://quotes/quote-003",
  },
];

let serviceReceipt:
  | {
      receipt_id: string;
      asset_id: string;
      service_date: string;
      vendor_label: string;
      service_type: string;
      amount_usd: number;
      warranty_id: string;
      notes: string;
      submission_count: number;
      status: string;
      next_due_date: string;
      source_locator: string;
    }
  | undefined;

const requests: Array<{ method: string; path: string; body?: unknown }> = [];
const expectedNotes = "Synthetic service receipt only; no real dispatch or payment.";

const docs = `# Synthetic home-maintenance API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot dispatch a technician,
authorize a repair, charge money, contact a provider, or change a real home's
maintenance state. Work only in one personal Noema workspace.

## Source records

- GET /v1/profile returns the property, current date, timezone, service budget,
  goal, decision boundary, and source locator.
- GET /v1/manual returns the HVAC asset's service interval, filter interval,
  seasonal recommendation, duration, and safety note.
- GET /v1/warranty returns the exact serial-matched warranty and its coverage
  dates. It expires on 2027-09-12.
- GET /v1/service-history returns the last service on 2026-03-12 for 160 USD.
  Add 180 days to derive the 2026-09-08 due date.
- GET /v1/constraints returns the safe service window, heat-advisory dates,
  quiet hours, access hours, and the 300 USD planned-service limit.
- GET /v1/quotes returns two feasible quotes and one cheaper quote outside the
  safe window. quote-001 is the first feasible safe slot at 185 USD and
  includes a filter; quote-002 is 260 USD and excludes the filter.
- GET /v1/existing-plan returns the absence of a native recurring Task. The
  test operator will create one after the agent supplies a verified proposal.

## Record one later service receipt

- POST /v1/service-receipts accepts asset_id, service_date, vendor_label,
  service_type, amount_usd, warranty_id, and notes. The only valid synthetic
  write uses asset-001, 2026-09-12, CoolAir Service (synthetic),
  seasonal_tune_up, 185, warranty-hvac-001, and the exact note:
  ${expectedNotes}
- The first accepted request returns service-receipt-001, submission_count 1,
  and next_due_date 2027-03-11. Repeating it returns the same receipt and
  keeps submission_count at one.

## Verify the maintenance state

- GET /v1/status returns the due date, selected quote, receipt, amount, budget
  remaining, next due date, and one submission count after the write.

There are no other routes. Do not propose a real dispatch, payment, repair,
provider contact, or undocumented operation.
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
      return json({ ok: true, synthetic: true, service: "home-maintenance", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/manual") return json(manual);
    if (request.method === "GET" && url.pathname === "/v1/warranty") return json(warranty);
    if (request.method === "GET" && url.pathname === "/v1/service-history") return json(lastService);
    if (request.method === "GET" && url.pathname === "/v1/constraints") return json(constraints);
    if (request.method === "GET" && url.pathname === "/v1/quotes") return json({ quotes });
    if (request.method === "GET" && url.pathname === "/v1/existing-plan") {
      return json({
        plan_id: "maintenance-plan-001",
        recurring_task_id: "none",
        plan_status: "needs_native_task",
        next_due_date: "2026-09-08",
        source_locator: "home://plans/maintenance-plan-001",
      });
    }
    if (request.method === "POST" && url.pathname === "/v1/service-receipts") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.asset_id !== "hvac-001" ||
        value.service_date !== "2026-09-12" ||
        value.vendor_label !== "CoolAir Service (synthetic)" ||
        value.service_type !== "seasonal_tune_up" ||
        value.amount_usd !== 185 ||
        value.warranty_id !== "warranty-hvac-001" ||
        value.notes !== expectedNotes
      ) {
        return json({ error: "service receipt must use the exact approved synthetic quote and note" }, 400);
      }
      if (!serviceReceipt) {
        serviceReceipt = {
          receipt_id: "service-receipt-001",
          asset_id: "hvac-001",
          service_date: "2026-09-12",
          vendor_label: "CoolAir Service (synthetic)",
          service_type: "seasonal_tune_up",
          amount_usd: 185,
          warranty_id: "warranty-hvac-001",
          notes: expectedNotes,
          submission_count: 1,
          status: "recorded_in_synthetic_maintenance",
          next_due_date: "2027-03-11",
          source_locator: "home://service-receipts/service-receipt-001",
        };
      }
      return json(serviceReceipt, serviceReceipt.submission_count === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/status") {
      return json({
        case_id: profile.case_id,
        asset_id: asset.asset_id,
        derived_due_date: "2026-09-08",
        selected_quote_id: serviceReceipt ? "quote-001" : "none",
        selected_amount_usd: serviceReceipt?.amount_usd ?? 0,
        budget_limit_usd: constraints.budget_limit_usd,
        budget_remaining_usd: serviceReceipt ? constraints.budget_limit_usd - serviceReceipt.amount_usd : constraints.budget_limit_usd,
        service_receipt_id: serviceReceipt?.receipt_id ?? "none",
        service_submission_count: serviceReceipt?.submission_count ?? 0,
        service_status: serviceReceipt?.status ?? "not_recorded",
        next_due_date: serviceReceipt?.next_due_date ?? "2026-09-08",
        source_locator: "home://status/home-maintenance-001",
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic home-maintenance API listening on http://127.0.0.1:${server.port}`);
