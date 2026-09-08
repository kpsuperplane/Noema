#!/usr/bin/env bun

// Synthetic spending ledger for PA-037.
// It has no connection to a bank, card, merchant, or real account.

const port = Number(process.env.NOEMA_SPENDING_PORT ?? "3759");
const fixtureVersion = "2026-09-08-spending-api-v1";

type SpendingRow = {
  row_id: string;
  transaction_id: string;
  date: string;
  period: "A" | "B";
  merchant: string;
  category: string | null;
  amount: number;
  currency: "USD";
  kind: "charge" | "refund";
  source_locator: string;
};

const profile = {
  ledger_name: "Ninety-day household spending",
  period_start: "2026-06-01",
  period_end: "2026-08-29",
  comparison_periods: [
    { id: "A", start: "2026-06-01", end: "2026-07-15" },
    { id: "B", start: "2026-07-16", end: "2026-08-29" },
  ],
  currency: "USD",
  deduplication_rule:
    "Rows with the same transaction_id, date, merchant, amount, currency, and kind are one transaction. Keep one row.",
  refund_rule: "Refund amounts are negative and reduce net spending.",
  missing_category_rule:
    "Keep rows with a missing category in the total and flag them for review. Do not guess a category.",
  expected_net_total: 2400,
};

const rows: SpendingRow[] = [
  {
    row_id: "row-001",
    transaction_id: "txn-001",
    date: "2026-06-01",
    period: "A",
    merchant: "Harborview Apartments",
    category: "Housing",
    amount: 700,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-001",
  },
  {
    row_id: "row-002",
    transaction_id: "txn-002",
    date: "2026-06-05",
    period: "A",
    merchant: "Green Basket Market",
    category: "Groceries",
    amount: 160,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-002",
  },
  {
    row_id: "row-003",
    transaction_id: "txn-003",
    date: "2026-06-10",
    period: "A",
    merchant: "Metro Transit",
    category: "Transport",
    amount: 60,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-003",
  },
  {
    row_id: "row-004",
    transaction_id: "txn-004",
    date: "2026-06-15",
    period: "A",
    merchant: "Juniper Cafe",
    category: "Dining",
    amount: 80,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-004",
  },
  {
    row_id: "row-005",
    transaction_id: "txn-004",
    date: "2026-06-15",
    period: "A",
    merchant: "Juniper Cafe",
    category: "Dining",
    amount: 80,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-005",
  },
  {
    row_id: "row-006",
    transaction_id: "txn-005",
    date: "2026-06-25",
    period: "A",
    merchant: "Riverlight Utilities",
    category: "Utilities",
    amount: 60,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-006",
  },
  {
    row_id: "row-007",
    transaction_id: "txn-006",
    date: "2026-07-04",
    period: "A",
    merchant: "North Clinic",
    category: "Medical",
    amount: 40,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-007",
  },
  {
    row_id: "row-008",
    transaction_id: "txn-007",
    date: "2026-07-16",
    period: "B",
    merchant: "Harborview Apartments",
    category: "Housing",
    amount: 700,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-008",
  },
  {
    row_id: "row-009",
    transaction_id: "txn-008",
    date: "2026-07-20",
    period: "B",
    merchant: "Green Basket Market",
    category: "Groceries",
    amount: 200,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-009",
  },
  {
    row_id: "row-010",
    transaction_id: "txn-009",
    date: "2026-07-23",
    period: "B",
    merchant: "Metro Transit",
    category: "Transport",
    amount: 70,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-010",
  },
  {
    row_id: "row-011",
    transaction_id: "txn-010",
    date: "2026-07-28",
    period: "B",
    merchant: "Juniper Cafe",
    category: "Dining",
    amount: 140,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-011",
  },
  {
    row_id: "row-012",
    transaction_id: "txn-011",
    date: "2026-08-02",
    period: "B",
    merchant: "Juniper Cafe",
    category: "Dining",
    amount: -100,
    currency: "USD",
    kind: "refund",
    source_locator: "https://spending.example.test/ledger.csv#row-012",
  },
  {
    row_id: "row-013",
    transaction_id: "txn-012",
    date: "2026-08-08",
    period: "B",
    merchant: "Cloudline Services",
    category: null,
    amount: 120,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-013",
  },
  {
    row_id: "row-014",
    transaction_id: "txn-013",
    date: "2026-08-15",
    period: "B",
    merchant: "Riverlight Utilities",
    category: "Utilities",
    amount: 80,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-014",
  },
  {
    row_id: "row-015",
    transaction_id: "txn-014",
    date: "2026-08-22",
    period: "B",
    merchant: "Lantern Cinema",
    category: "Entertainment",
    amount: 90,
    currency: "USD",
    kind: "charge",
    source_locator: "https://spending.example.test/ledger.csv#row-015",
  },
];

const docs = `# Synthetic spending API

Fixture version: ${fixtureVersion}

This deterministic, read-only service contains a ninety-day household
spending ledger from 2026-06-01 through 2026-08-29. It has no connection to a
bank, card, merchant, or real account. Every row is synthetic test data.

## Operations

- \`GET /v1/profile\`: return the date range, two equal-length comparison
  periods, currency, deduplication rule, refund rule, missing-category rule,
  and expected net total.
- \`GET /v1/transactions\`: return every ledger row in a \`transactions\`
  array. Each row contains row_id, transaction_id, date, period, merchant,
  category (null means missing), amount, currency, kind, and source_locator.

The first comparison period is A (2026-06-01 through 2026-07-15). The second
is B (2026-07-16 through 2026-08-29). There is one exact duplicate row:
row-005 duplicates row-004. There is one refund: txn-011 for -100 USD. The
Cloudline Services row has a missing category. After removing the duplicate,
the net total is 2400 USD. Period A totals 1100 USD and period B totals 1300
USD. The service has no write operation.
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
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "spending", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/transactions") {
      return json({ transactions: rows, next_page_token: null });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic spending API listening on http://127.0.0.1:${server.port}`);
