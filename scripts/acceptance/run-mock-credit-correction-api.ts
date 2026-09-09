#!/usr/bin/env bun

// Synthetic credit-record correction service for PA-048.
// It cannot contact a bureau, change a real credit file, or move money.

const port = Number(process.env.NOEMA_CREDIT_PORT ?? "3770");
const fixtureVersion = "2026-09-09-credit-correction-api-v1";

const profile = {
  account_id: "credit-account-001",
  account_label: "Jordan Lee (synthetic)",
  review_goal: "Correct an inaccurate late-payment report using the lender receipt.",
  decision_boundary: "Do not contact a real bureau, change a real credit file, or move money.",
  source_locator: "credit://accounts/credit-account-001/profile",
};

const reports = [
  {
    report_id: "credit-report-001",
    account_id: profile.account_id,
    creditor: "Northstar Card Services (synthetic)",
    account_reference: "NS-4401",
    reported_status: "late_payment",
    reported_amount: 145,
    report_date: "2026-09-06",
    source_locator: "credit://reports/credit-report-001",
  },
];

const scheduledPayments = [
  {
    schedule_id: "schedule-145-2026-09",
    account_id: profile.account_id,
    amount: 145,
    currency: "USD",
    due_date: "2026-09-05",
    schedule_status: "scheduled",
    posted: false,
    source_locator: "credit://schedules/schedule-145-2026-09",
  },
];

const lenderPayments = [
  {
    payment_id: "payment-145-2026-09",
    schedule_id: "schedule-145-2026-09",
    account_id: profile.account_id,
    amount: 145,
    currency: "USD",
    received_at: "2026-09-04T14:30:00Z",
    payment_status: "settled",
    receipt_id: "receipt-145-2026-09",
    source_locator: "credit://payments/payment-145-2026-09",
  },
];

let dispute:
  | {
      dispute_id: string;
      report_id: string;
      payment_id: string;
      reason: string;
      status: "submitted";
      submission_count: number;
      submitted_at: string;
      source_locator: string;
    }
  | undefined;

const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic credit-correction API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot contact a credit bureau,
change a real credit file, or move money. All records and amounts are test
data in US dollars. Read routes are safe. The dispute route records one mock
dispute and requires human approval in the client.

## Records

- GET /v1/profile returns the account, review goal, and decision boundary.
- GET /v1/credit-reports returns the bureau report. It contains report_id,
  account_id, creditor, account_reference, reported_status, reported_amount,
  report_date, and source_locator. The initial status is late_payment.
- GET /v1/payment-schedules returns scheduled_payments. Each record contains
  schedule_id, account_id, amount, currency, due_date, schedule_status, posted,
  and source_locator. The $145 payment is scheduled and not posted.
- GET /v1/lender-payments returns payments. Each record contains payment_id,
  schedule_id, account_id, amount, currency, received_at, payment_status,
  receipt_id, and source_locator. The lender receipt proves the $145 payment
  settled on September 4, before the September 5 due date.

## Dispute and later report

- POST /v1/disputes records one mock dispute. Its JSON body must contain
  report_id, payment_id, and reason. Use report_id credit-report-001 and
  payment_id payment-145-2026-09. Repeating the same report returns the same
  dispute and does not increase submission_count. This route never contacts a
  bureau.
- GET /v1/disputes/{dispute_id} returns the mock submission receipt and status.
- GET /v1/credit-reports/{report_id}/corrected returns the later report. It is
  available after the dispute and reports correction_status corrected and
  reported_status current, with the dispute_id and source_locator.

There is no real bureau, creditor, payment, account, or financial-action
endpoint. Do not propose or expose an operator-only route.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function pathId(path: string, prefix: string) {
  return path.startsWith(prefix) ? path.slice(prefix.length) : undefined;
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    let body: unknown;
    if (request.method !== "GET") body = await request.json().catch(() => undefined);
    requests.push({
      method: request.method,
      path: `${url.pathname}${url.search}`,
      ...(body === undefined ? {} : { body }),
    });
    console.log(JSON.stringify(requests.at(-1)));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "credit-correction", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/credit-reports") return json({ reports });
    if (request.method === "GET" && url.pathname === "/v1/payment-schedules") {
      return json({ scheduled_payments: scheduledPayments });
    }
    if (request.method === "GET" && url.pathname === "/v1/lender-payments") return json({ payments: lenderPayments });
    if (request.method === "POST" && url.pathname === "/v1/disputes") {
      const input = (body ?? {}) as Record<string, unknown>;
      const reportId = String(input.report_id ?? "");
      const paymentId = String(input.payment_id ?? "");
      const reason = typeof input.reason === "string" ? input.reason : "";
      if (reportId !== reports[0].report_id || paymentId !== lenderPayments[0].payment_id || reason.length < 8) {
        return json({ error: "invalid_dispute", message: "Use the exact report and lender payment IDs with a reason." }, 422);
      }
      if (!dispute) {
        dispute = {
          dispute_id: "dispute-credit-001",
          report_id: reportId,
          payment_id: paymentId,
          reason,
          status: "submitted",
          submission_count: 1,
          submitted_at: "2026-09-09T00:00:00Z",
          source_locator: "credit://disputes/dispute-credit-001",
        };
      }
      return json(dispute, 201);
    }
    const disputeId = pathId(url.pathname, "/v1/disputes/");
    if (request.method === "GET" && disputeId) {
      if (!dispute || dispute.dispute_id !== disputeId) return json({ error: "not_found" }, 404);
      return json(dispute);
    }
    const correctedReportId = pathId(url.pathname, "/v1/credit-reports/");
    if (request.method === "GET" && correctedReportId?.endsWith("/corrected")) {
      const reportId = correctedReportId.slice(0, -"/corrected".length);
      if (!dispute || reportId !== reports[0].report_id) return json({ error: "not_ready" }, 404);
      return json({
        report_id: reportId,
        account_id: profile.account_id,
        correction_status: "corrected",
        reported_status: "current",
        dispute_id: dispute.dispute_id,
        corrected_at: "2026-09-10T00:00:00Z",
        source_locator: "credit://reports/credit-report-001/corrected",
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic credit-correction API listening on http://127.0.0.1:${server.port}`);
