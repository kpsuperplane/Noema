#!/usr/bin/env bun

// Synthetic retirement-records service for PA-047.
// It cannot move retirement money or make an investment decision.

const port = Number(process.env.NOEMA_RETIREMENT_PORT ?? "3769");
const fixtureVersion = "2026-09-09-retirement-records-api-v1";

const profile = {
  owner_id: "retirement-owner-001",
  owner_label: "Jordan Lee (synthetic)",
  review_goal: "Reconcile retirement records and track a rollover receipt.",
  decision_boundary: "Do not transfer money or make an investment decision.",
  source_locator: "retirement://owners/retirement-owner-001/profile",
};

const accounts = [
  {
    account_id: "retirement-account-24960",
    provider: "Northstar Workplace Plan (synthetic)",
    account_type: "former_employer_401k",
    balance: 24960,
    currency: "USD",
    annual_fee_bps: 55,
    source_locator: "retirement://accounts/retirement-account-24960",
  },
  {
    account_id: "retirement-account-86240",
    provider: "Harbor IRA Custodian (synthetic)",
    account_type: "traditional_ira",
    balance: 86240,
    currency: "USD",
    annual_fee_bps: 25,
    source_locator: "retirement://accounts/retirement-account-86240",
  },
];

// The first two statements intentionally describe the same account and balance.
const statements = [
  {
    statement_id: "statement-24960-2026-q1",
    account_id: "retirement-account-24960",
    statement_date: "2026-03-31",
    reported_balance: 24960,
    currency: "USD",
    source_locator: "retirement://statements/statement-24960-2026-q1",
  },
  {
    statement_id: "statement-24960-2026-q2",
    account_id: "retirement-account-24960",
    statement_date: "2026-06-30",
    reported_balance: 24960,
    currency: "USD",
    source_locator: "retirement://statements/statement-24960-2026-q2",
  },
  {
    statement_id: "statement-86240-2026-q2",
    account_id: "retirement-account-86240",
    statement_date: "2026-06-30",
    reported_balance: 86240,
    currency: "USD",
    source_locator: "retirement://statements/statement-86240-2026-q2",
  },
];

const fees = [
  {
    account_id: "retirement-account-24960",
    fee_bps: 55,
    annual_fee_usd: 137.28,
    fee_basis: "0.55% of the reported balance",
    source_locator: "retirement://fees/retirement-account-24960",
  },
  {
    account_id: "retirement-account-86240",
    fee_bps: 25,
    annual_fee_usd: 215.60,
    fee_basis: "0.25% of the reported balance",
    source_locator: "retirement://fees/retirement-account-86240",
  },
];

const transfer = {
  transfer_id: "rollover-001",
  source_account_id: "retirement-account-24960",
  destination_account_id: "retirement-account-86240",
  amount: 24960,
  currency: "USD",
  status: "sent",
  receipt_status: "not_received",
  sent_date: "2026-09-04",
  next_check_after: "2026-09-11",
  source_locator: "retirement://transfers/rollover-001",
};

const receipt = {
  transfer_id: "rollover-001",
  receipt_id: "receipt-rollover-001",
  status: "received",
  received_amount: 24960,
  currency: "USD",
  received_at: "2026-09-09T00:00:00Z",
  source_locator: "retirement://transfers/rollover-001/receipt",
};

const requests: Array<{ method: string; path: string }> = [];

const docs = `# Synthetic retirement-records API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot transfer retirement money,
change an account, make an investment decision, or provide financial advice.
All people, balances, providers, and dates are test records in US dollars.

## Operations

- GET /v1/profile returns the synthetic owner's review goal and decision boundary.
- GET /v1/accounts returns each distinct retirement account and its balance and
  fee rate. Account balances are not investment recommendations.
- GET /v1/statements returns statement records. Multiple statements can refer
  to the same account and must not be added as separate accounts.
- GET /v1/fees returns the documented fee rate and annual fee estimate for each
  account.
- GET /v1/transfers returns rollover records. Rollover \`rollover-001\` is sent
  and its destination has not yet reported a receipt in this record.
- GET /v1/transfers/{transfer_id}/status returns the latest transfer status
  record and its next-check date.
- GET /v1/transfers/{transfer_id}/receipt returns the later fixture receipt
  record. Read the transfer status before reading this receipt.

There are no POST, PATCH, DELETE, investment, or money-movement routes.
The receipt route is a read-only fixture record for testing later status
verification; it does not move funds.
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
    requests.push({ method: request.method, path: `${url.pathname}${url.search}` });
    console.log(JSON.stringify(requests.at(-1)));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "retirement-records", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/accounts") return json({ accounts });
    if (request.method === "GET" && url.pathname === "/v1/statements") return json({ statements });
    if (request.method === "GET" && url.pathname === "/v1/fees") return json({ fees });
    if (request.method === "GET" && url.pathname === "/v1/transfers") return json({ transfers: [transfer] });
    if (request.method === "GET" && url.pathname === "/v1/transfers/rollover-001/status") {
      return json({
        transfer_id: transfer.transfer_id,
        status: transfer.status,
        receipt_status: transfer.receipt_status,
        sent_date: transfer.sent_date,
        next_check_after: transfer.next_check_after,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/transfers/rollover-001/receipt") return json(receipt);
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic retirement-records API listening on http://127.0.0.1:${server.port}`);
