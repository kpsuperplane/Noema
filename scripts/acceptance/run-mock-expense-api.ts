#!/usr/bin/env bun

// A provider-neutral expense and bank API for the live personal-assistant cases.
// It is synthetic and has no connection to a payment network or real account.

const port = Number(process.env.NOEMA_EXPENSE_PORT ?? "3740");
const fixtureVersion = "2026-09-08-expense-api-v1";

type Receipt = {
  id: string;
  date: string;
  merchant: string;
  amount: number;
  currency: string;
  purpose: string;
  project: string;
  eligible: boolean;
  duplicateOf?: string;
  exclusionReason?: string;
};

type Claim = {
  id: string;
  amount: number;
  receiptIds: string[];
  excludedReceiptIds: string[];
  status: "SUBMITTED" | "PAID";
  submissionCount: number;
};

const receipts: Receipt[] = [
  {
    id: "receipt-040",
    date: "2026-09-02",
    merchant: "Northstar Office Supply",
    amount: 40,
    currency: "USD",
    purpose: "Team supplies",
    project: "Launch support",
    eligible: true,
  },
  {
    id: "receipt-060",
    date: "2026-09-03",
    merchant: "Harbor Transit",
    amount: 60,
    currency: "USD",
    purpose: "Client visit",
    project: "Launch support",
    eligible: true,
  },
  {
    id: "receipt-060-duplicate",
    date: "2026-09-03",
    merchant: "Harbor Transit",
    amount: 60,
    currency: "USD",
    purpose: "Client visit",
    project: "Launch support",
    eligible: true,
    duplicateOf: "receipt-060",
  },
  {
    id: "receipt-025",
    date: "2026-09-04",
    merchant: "Personal Market",
    amount: 25,
    currency: "USD",
    purpose: "Personal purchase",
    project: "Launch support",
    eligible: false,
    exclusionReason: "Personal purchases are not reimbursable under the policy.",
  },
];

const bankTransactions = receipts.map((receipt) => ({
  id: `bank-${receipt.id}`,
  date: receipt.date,
  merchant: receipt.merchant,
  amount: receipt.amount,
  currency: receipt.currency,
  status: "POSTED",
}));

let claim: Claim | undefined;

const docs = `# Synthetic expense API

Fixture version: ${fixtureVersion}

This API is a deterministic test service. It cannot move money or contact an
employer, bank, merchant, or payment network.

The service uses JSON and has no authentication requirement because every record
is synthetic. The base URL is the URL that served this document.

## Operations

- \`GET /v1/expenses/receipts\`: return the bounded receipt set.
- \`GET /v1/bank/transactions\`: return posted bank charges for reconciliation.
- \`POST /v1/reimbursements\`: submit one claim. The JSON body must contain
  \`amount\`, \`receipt_ids\`, and \`excluded_receipt_ids\`. A repeated
  submission returns the original claim instead of creating another.
- \`GET /v1/reimbursements/{id}\`: read the claim status. A submitted claim
  becomes \`PAID\` on the first status read.

The response fields are stable strings, numbers, booleans, and arrays. Clients
must keep the receipt IDs so a claim can be traced to its source records.
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
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "expense", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/expenses/receipts") {
      return json({ receipts, next_page_token: null });
    }
    if (request.method === "GET" && url.pathname === "/v1/bank/transactions") {
      return json({ transactions: bankTransactions, next_page_token: null });
    }
    if (request.method === "POST" && url.pathname === "/v1/reimbursements") {
      const body = await request.json().catch(() => undefined) as Record<string, unknown> | undefined;
      const amount = body?.amount;
      const receiptIds = body?.receipt_ids;
      const excludedReceiptIds = body?.excluded_receipt_ids;
      if (typeof amount !== "number" || !Array.isArray(receiptIds) || !Array.isArray(excludedReceiptIds)) {
        return json({ error: "amount, receipt_ids, and excluded_receipt_ids are required" }, 400);
      }
      if (claim) return json(claim);
      claim = {
        id: "reimbursement-001",
        amount,
        receiptIds: receiptIds.map(String),
        excludedReceiptIds: excludedReceiptIds.map(String),
        status: "SUBMITTED",
        submissionCount: 1,
      };
      return json(claim, 201);
    }
    const id = pathId(url.pathname, "/v1/reimbursements/");
    if (request.method === "GET" && id) {
      if (!claim || claim.id !== id) return json({ error: "not_found" }, 404);
      claim.status = "PAID";
      return json(claim);
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic expense API listening on http://127.0.0.1:${server.port}`);
