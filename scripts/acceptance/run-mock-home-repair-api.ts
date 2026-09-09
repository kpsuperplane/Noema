#!/usr/bin/env bun

// Synthetic home-repair service for PA-067.
// It cannot contact a contractor, bind insurance, repair a home, or move money.

const port = Number(process.env.NOEMA_HOME_REPAIR_PORT ?? "3789");
const fixtureVersion = "2026-09-09-home-repair-api-v1";

const profile = {
  case_id: "home-repair-001",
  owner_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  time_zone: "America/Los_Angeles",
  property_label: "Jordan's synthetic home",
  project_label: "Main water-line repair",
  goal: "Repair the leaking main water line before 2026-09-30 while keeping the complete project and insurance cost under 10000 USD.",
  decision_boundary: "Do not contact a real contractor, bind real insurance, authorize a real repair, enter a real home, or move money.",
  budget_limit_usd: 10000,
  required_completion_date: "2026-09-30",
  source_locator: "home://profiles/home-repair-001",
};

const bids = [
  {
    bid_id: "bid-001",
    vendor_label: "Apex Home Repair (synthetic)",
    scope_id: "scope-001",
    base_amount_usd: 8200,
    disposal_included: "no",
    disposal_amount_usd: 450,
    permit_included: "yes",
    permit_amount_usd: 0,
    normalized_total_usd: 8650,
    insurance_valid_through: "2026-09-18",
    proposed_start_date: "2026-09-14",
    proposed_start_time: "09:00",
    estimated_duration_days: 5,
    estimated_end_date: "2026-09-18",
    scope_summary: "Replace 12 feet of copper line, patch drywall, test pressure; disposal is extra.",
    bid_status: "feasible",
    source_locator: "home://bids/bid-001",
  },
  {
    bid_id: "bid-002",
    vendor_label: "Cedar Works (synthetic)",
    scope_id: "scope-001",
    base_amount_usd: 9150,
    disposal_included: "yes",
    disposal_amount_usd: 0,
    permit_included: "yes",
    permit_amount_usd: 0,
    normalized_total_usd: 9150,
    insurance_valid_through: "2027-06-30",
    proposed_start_date: "2026-09-16",
    proposed_start_time: "13:00",
    estimated_duration_days: 4,
    estimated_end_date: "2026-09-19",
    scope_summary: "Replace 12 feet of copper line, patch drywall, test pressure, and dispose of debris.",
    bid_status: "feasible",
    source_locator: "home://bids/bid-002",
  },
  {
    bid_id: "bid-003",
    vendor_label: "Budget Repair (synthetic)",
    scope_id: "scope-001",
    base_amount_usd: 7600,
    disposal_included: "yes",
    disposal_amount_usd: 0,
    permit_included: "no",
    permit_amount_usd: 500,
    normalized_total_usd: 8100,
    insurance_valid_through: "2026-08-31",
    proposed_start_date: "2026-09-12",
    proposed_start_time: "10:00",
    estimated_duration_days: 4,
    estimated_end_date: "2026-09-15",
    scope_summary: "Replace 12 feet of copper line and patch drywall; permit is extra and insurance is expired.",
    bid_status: "insurance_expired",
    source_locator: "home://bids/bid-003",
  },
];

const insurance = {
  policy_id: "home-policy-001",
  provider_label: "Harborline Home (synthetic)",
  status: "active",
  expires_on: "2026-09-18",
  required_through: "project completion plus 30 days",
  extension_id: "insurance-extension-001",
  extension_effective_on: "2026-09-19",
  extension_expires_on: "2027-09-18",
  extension_premium_usd: 280,
  extension_status: "available_for_approval",
  coverage_summary: "Synthetic dwelling and repair-project coverage; extension is required when work continues after 2026-09-18.",
  source_locator: "home://insurance/home-policy-001",
};

const scope = {
  scope_id: "scope-001",
  scope_title: "Main water-line replacement",
  included_work: "Replace 12 feet of copper line, patch drywall, pressure-test the line, and leave the work area safe.",
  excluded_work: "Cosmetic paint beyond the drywall patch and unrelated plumbing defects.",
  completion_standard: "Pressure test passes, the line is dry, the approved work is complete, and completion evidence is supplied.",
  source_locator: "home://scope/scope-001",
};

const constraints = {
  access_window: "Weekdays 09:00-17:00 America/Los_Angeles",
  quiet_hours: "20:00-08:00 America/Los_Angeles",
  no_work_dates: "2026-09-13",
  no_work_reason: "Synthetic family access constraint",
  required_completion_date: "2026-09-30",
  budget_limit_usd: 10000,
  budget_scope: "The limit includes the normalized contractor scope, approved scope changes, and the required insurance extension.",
  source_locator: "home://constraints/home-repair-001",
};

const existingPlan = {
  plan_id: "repair-plan-001",
  plan_status: "ready_for_bid_selection",
  accepted_bid_id: "none",
  scheduled_start: "none",
  scheduled_end: "none",
  source_locator: "home://plans/repair-plan-001",
};

const requests: Array<{ method: string; path: string; body?: unknown }> = [];
let acceptedBid = false;
let acceptedChange = false;
let insuranceExtended = false;
let completionRecorded = false;
let paymentRecorded = false;
let bidSubmissionCount = 0;
let changeSubmissionCount = 0;
let insuranceSubmissionCount = 0;
let completionSubmissionCount = 0;
let paymentSubmissionCount = 0;

const docs = `# Synthetic home-repair API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot contact a contractor, enter
a real home, bind insurance, authorize a real repair, charge money, or move
money. Work only in one personal Noema workspace.

## Source records

- GET /v1/profile returns the owner, property, project goal, 10000 USD budget,
  required completion date, current date, timezone, and decision boundary.
- GET /v1/bids returns three bids for scope-001. bid-001 is 8200 USD plus 450
  USD disposal, for a normalized 8650 USD. bid-002 is 9150 USD with disposal
  included. bid-003 is 7600 USD plus a 500 USD permit, but its insurance ended
  on 2026-08-31 and is not eligible.
- GET /v1/insurance returns policy home-policy-001, which expires on
  2026-09-18, and extension insurance-extension-001 from 2026-09-19 through
  2027-09-18 for a 280 USD premium.
- GET /v1/scope returns the accepted base scope and completion standard.
- GET /v1/constraints returns weekday access, quiet hours, a no-work date,
  the 2026-09-30 completion deadline, and the all-in budget rule.
- GET /v1/existing-plan returns a plan with no accepted bid.

## Initial bid acceptance

- POST /v1/bid-acceptance accepts exactly bid-001, scope-001, start
  2026-09-14T09:00, end 2026-09-18T17:00, normalized total 8650, and the note
  "Synthetic bid acceptance only; no contractor contact or repair authorization."
- The first accepted request returns booking-001 and submission_count 1.
- Repeating the exact request is idempotent and keeps submission_count at one.

## Later scope change

- GET /v1/change-request returns change-001 after bid acceptance. A hidden
  water-damage finding adds six feet of line and mold-resistant drywall, adds
  900 USD, and adds two work days through 2026-09-22. It requires separate
  approval.
- POST /v1/change-approval accepts exactly change-001, 900 USD, end date
  2026-09-22, and the note
  "Synthetic scope change approval only; no contractor contact."

## Insurance extension

- POST /v1/insurance-extension accepts exactly home-policy-001,
  insurance-extension-001, effective 2026-09-19, expires 2027-09-18, premium
  280 USD, and the note
  "Synthetic insurance extension only; no coverage was bound."

## Completion and payment records

- POST /v1/completion accepts project home-repair-001, completion date
  2026-09-22, evidence completion-photo-set-001, completed scope
  "scope-001+change-001", and final contractor amount 9550 USD.
- POST /v1/payment accepts project home-repair-001, payment date 2026-09-23,
  invoice invoice-001, amount 9550 USD, payment method
  "synthetic_bank_transfer", and the note
  "Synthetic payment receipt only; no money moved."
- GET /v1/status returns the accepted bid, approved change, insurance
  extension, completion, payment receipt, all-in total 9830 USD, and 170 USD
  remaining under the 10000 USD limit.

There are no other routes. Do not propose a real contractor contact, repair,
insurance binding, or payment.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function acceptedBidResult() {
  return {
    booking_id: "booking-001",
    project_id: "home-repair-001",
    accepted_bid_id: "bid-001",
    scope_id: "scope-001",
    scheduled_start: "2026-09-14T09:00:00-07:00",
    scheduled_end: "2026-09-18T17:00:00-07:00",
    normalized_total_usd: 8650,
    status: "accepted_in_synthetic_project",
    submission_count: bidSubmissionCount,
    source_locator: "home://bid-acceptances/booking-001",
  };
}

function changeResult() {
  return {
    change_request_id: "change-001",
    project_id: "home-repair-001",
    status: acceptedChange ? "approved_in_synthetic_project" : "pending_separate_approval",
    reason: "Hidden water damage found after opening the wall (synthetic finding).",
    added_scope: "Replace six additional feet of line and install mold-resistant drywall.",
    added_amount_usd: 900,
    added_duration_days: 2,
    proposed_end_date: "2026-09-22",
    requires_separate_approval: "yes",
    submission_count: changeSubmissionCount,
    source_locator: "home://changes/change-001",
  };
}

function insuranceResult() {
  return {
    policy_id: insurance.policy_id,
    extension_id: insurance.extension_id,
    status: insuranceExtended ? "extended_in_synthetic_project" : insurance.extension_status,
    effective_on: insurance.extension_effective_on,
    expires_on: insurance.extension_expires_on,
    premium_usd: insurance.extension_premium_usd,
    submission_count: insuranceSubmissionCount,
    source_locator: "home://insurance-extensions/insurance-extension-001",
  };
}

function completionResult() {
  return {
    completion_id: "completion-001",
    project_id: "home-repair-001",
    completion_date: "2026-09-22",
    completion_evidence_id: "completion-photo-set-001",
    completed_scope: "scope-001+change-001",
    final_amount_usd: 9550,
    status: completionRecorded ? "completed_in_synthetic_project" : "not_recorded",
    submission_count: completionSubmissionCount,
    source_locator: "home://completions/completion-001",
  };
}

function paymentResult() {
  return {
    payment_receipt_id: "payment-001",
    invoice_id: "invoice-001",
    project_id: "home-repair-001",
    payment_date: "2026-09-23",
    amount_usd: 9550,
    payment_method: "synthetic_bank_transfer",
    status: paymentRecorded ? "recorded_in_synthetic_ledger" : "not_recorded",
    submission_count: paymentSubmissionCount,
    source_locator: "home://payments/payment-001",
  };
}

function statusResult() {
  const contractorTotal = acceptedBid ? 8650 + (acceptedChange ? 900 : 0) : 0;
  const insuranceTotal = insuranceExtended ? 280 : 0;
  return {
    case_id: profile.case_id,
    project_id: "home-repair-001",
    project_status: paymentRecorded
      ? "paid_in_synthetic_ledger"
      : completionRecorded
        ? "completed_in_synthetic_project"
        : acceptedChange
          ? "change_approved_in_synthetic_project"
          : acceptedBid
            ? "bid_accepted_in_synthetic_project"
            : "ready_for_bid_selection",
    accepted_bid_id: acceptedBid ? "bid-001" : "none",
    normalized_bid_total_usd: acceptedBid ? 8650 : 0,
    change_request_id: acceptedChange ? "change-001" : "none",
    approved_change_amount_usd: acceptedChange ? 900 : 0,
    insurance_extension_id: insuranceExtended ? "insurance-extension-001" : "none",
    insurance_premium_usd: insuranceTotal,
    contractor_total_usd: contractorTotal,
    project_total_usd: contractorTotal + insuranceTotal,
    budget_limit_usd: profile.budget_limit_usd,
    budget_remaining_usd: profile.budget_limit_usd - contractorTotal - insuranceTotal,
    completion_id: completionRecorded ? "completion-001" : "none",
    payment_receipt_id: paymentRecorded ? "payment-001" : "none",
    payment_amount_usd: paymentRecorded ? 9550 : 0,
    source_locator: "home://status/home-repair-001",
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
      return json({ ok: true, synthetic: true, service: "home-repair", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/bids") return json({ bids });
    if (request.method === "GET" && url.pathname === "/v1/insurance") return json(insurance);
    if (request.method === "GET" && url.pathname === "/v1/scope") return json(scope);
    if (request.method === "GET" && url.pathname === "/v1/constraints") return json(constraints);
    if (request.method === "GET" && url.pathname === "/v1/existing-plan") return json(existingPlan);
    if (request.method === "GET" && url.pathname === "/v1/change-request") {
      return acceptedBid ? json(changeResult()) : json({ error: "bid acceptance is required first" }, 409);
    }
    if (request.method === "POST" && url.pathname === "/v1/bid-acceptance") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.project_id !== "home-repair-001" ||
        value.bid_id !== "bid-001" ||
        value.scope_id !== "scope-001" ||
        value.scheduled_start !== "2026-09-14T09:00:00-07:00" ||
        value.scheduled_end !== "2026-09-18T17:00:00-07:00" ||
        value.normalized_total_usd !== 8650 ||
        value.approval_note !== "Synthetic bid acceptance only; no contractor contact or repair authorization."
      ) return json({ error: "bid acceptance must use the exact approved synthetic scope" }, 400);
      if (!acceptedBid) {
        acceptedBid = true;
        bidSubmissionCount = 1;
      }
      return json(acceptedBidResult(), bidSubmissionCount === 1 ? 201 : 200);
    }
    if (request.method === "POST" && url.pathname === "/v1/change-approval") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.project_id !== "home-repair-001" ||
        value.change_request_id !== "change-001" ||
        value.approved_amount_usd !== 900 ||
        value.approved_end_date !== "2026-09-22" ||
        value.approval_note !== "Synthetic scope change approval only; no contractor contact."
      ) return json({ error: "scope change must use the exact synthetic request" }, 400);
      if (!acceptedChange) {
        acceptedChange = true;
        changeSubmissionCount = 1;
      }
      return json(changeResult(), changeSubmissionCount === 1 ? 201 : 200);
    }
    if (request.method === "POST" && url.pathname === "/v1/insurance-extension") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.policy_id !== "home-policy-001" ||
        value.extension_id !== "insurance-extension-001" ||
        value.effective_on !== "2026-09-19" ||
        value.expires_on !== "2027-09-18" ||
        value.premium_usd !== 280 ||
        value.approval_note !== "Synthetic insurance extension only; no coverage was bound."
      ) return json({ error: "insurance extension must use the exact synthetic offer" }, 400);
      if (!insuranceExtended) {
        insuranceExtended = true;
        insuranceSubmissionCount = 1;
      }
      return json(insuranceResult(), insuranceSubmissionCount === 1 ? 201 : 200);
    }
    if (request.method === "POST" && url.pathname === "/v1/completion") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.project_id !== "home-repair-001" ||
        value.completion_date !== "2026-09-22" ||
        value.completion_evidence_id !== "completion-photo-set-001" ||
        value.completed_scope !== "scope-001+change-001" ||
        value.final_amount_usd !== 9550
      ) return json({ error: "completion must use the exact approved synthetic evidence" }, 400);
      if (!acceptedChange || !insuranceExtended) return json({ error: "change and insurance extension are required first" }, 409);
      if (!completionRecorded) {
        completionRecorded = true;
        completionSubmissionCount = 1;
      }
      return json(completionResult(), completionSubmissionCount === 1 ? 201 : 200);
    }
    if (request.method === "POST" && url.pathname === "/v1/payment") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.project_id !== "home-repair-001" ||
        value.payment_date !== "2026-09-23" ||
        value.invoice_id !== "invoice-001" ||
        value.amount_usd !== 9550 ||
        value.payment_method !== "synthetic_bank_transfer" ||
        value.note !== "Synthetic payment receipt only; no money moved."
      ) return json({ error: "payment must use the exact synthetic receipt fields" }, 400);
      if (!completionRecorded) return json({ error: "completion is required first" }, 409);
      if (!paymentRecorded) {
        paymentRecorded = true;
        paymentSubmissionCount = 1;
      }
      return json(paymentResult(), paymentSubmissionCount === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/status") return json(statusResult());
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic home-repair API listening on http://127.0.0.1:${server.port}`);
