#!/usr/bin/env bun

// Synthetic tax-records service for PA-042.
// It cannot file a return, contact a tax authority, or expose real taxpayer data.

const port = Number(process.env.NOEMA_TAX_RECORDS_PORT ?? "3764");
const fixtureVersion = "2026-09-08-tax-records-api-v1";

type TaxDocument = {
  document_id: string;
  document_type: string;
  tax_year: number;
  issuer: string;
  received_date: string;
  status: "superseded" | "current" | "received";
  amount: number;
  currency: "USD";
  source_locator: string;
  correction_of?: string;
  superseded_by?: string;
  deduction_category?: string;
  receipt_reference?: string;
  duplicate_of?: string;
};

const profile = {
  tax_year: 2025,
  taxpayer_label: "Jordan Lee (synthetic)",
  filing_status: "single",
  review_deadline: "2026-10-15",
  currency: "USD",
  packet_purpose: "Prepare an indexed packet for a professional tax review. Do not file a return.",
};

const documents: TaxDocument[] = [
  {
    document_id: "w2-2025-original",
    document_type: "W-2",
    tax_year: 2025,
    issuer: "Northstar Analytics LLC",
    received_date: "2026-01-28",
    status: "superseded",
    amount: 82000,
    currency: "USD",
    source_locator: "tax-records://documents/w2-2025-original",
    superseded_by: "w2-2025-corrected",
  },
  {
    document_id: "w2-2025-corrected",
    document_type: "W-2C",
    tax_year: 2025,
    issuer: "Northstar Analytics LLC",
    received_date: "2026-02-04",
    status: "current",
    amount: 83500,
    currency: "USD",
    source_locator: "tax-records://documents/w2-2025-corrected",
    correction_of: "w2-2025-original",
  },
  {
    document_id: "interest-2025-001",
    document_type: "1099-INT",
    tax_year: 2025,
    issuer: "Cedar Community Bank",
    received_date: "2026-01-31",
    status: "received",
    amount: 425.75,
    currency: "USD",
    source_locator: "tax-records://documents/interest-2025-001",
  },
  {
    document_id: "receipt-2025-001",
    document_type: "deductible receipt",
    tax_year: 2025,
    issuer: "Harbor Community Clinic",
    received_date: "2025-07-18",
    status: "received",
    amount: 680,
    currency: "USD",
    source_locator: "tax-records://documents/receipt-2025-001",
    deduction_category: "medical expense",
    receipt_reference: "clinic-2025-07-18",
  },
  {
    document_id: "receipt-2025-001-copy",
    document_type: "deductible receipt",
    tax_year: 2025,
    issuer: "Harbor Community Clinic",
    received_date: "2025-07-19",
    status: "received",
    amount: 680,
    currency: "USD",
    source_locator: "tax-records://documents/receipt-2025-001-copy",
    deduction_category: "medical expense",
    receipt_reference: "clinic-2025-07-18",
    duplicate_of: "receipt-2025-001",
  },
];

const missingForm = {
  document_id: "form-1098-2025",
  document_type: "1098",
  tax_year: 2025,
  issuer: "Harborview Mortgage Services",
  received_date: "2026-02-02",
  status: "received",
  amount: 3200,
  currency: "USD",
  interest_paid: 3200,
  source_locator: "tax-records://forms/1098-2025",
};

const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic tax-records API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot file a tax return, contact a
tax authority, or expose real taxpayer data. All amounts are US dollars.

## Profile

- GET /v1/profile returns tax_year, taxpayer_label, filing_status,
  review_deadline, currency, and packet_purpose.

## Records

- GET /v1/documents returns a JSON array of tax records. Each record has
  document_id, document_type, tax_year, issuer, received_date, status, amount,
  currency, and source_locator. A record may also include correction_of,
  superseded_by, deduction_category, receipt_reference, or duplicate_of.
- A superseded record must not replace a current corrected record. A
  duplicate receipt with the same receipt_reference and duplicate_of represents
  one expense and must be indexed once.

## Missing form

- GET /v1/forms/1098 returns the missing fixture form. It has document_id,
  document_type, tax_year, issuer, received_date, status, amount, currency,
  interest_paid, and source_locator.

The service has no filing, payment, or other write operation. Use the records
and the retrieved form to prepare a sourced index for professional review.
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
    if (request.method !== "GET") {
      body = await request.json().catch(() => undefined);
    }
    requests.push({
      method: request.method,
      path: `${url.pathname}${url.search}`,
      ...(body === undefined ? {} : { body }),
    });
    console.log(JSON.stringify(requests.at(-1)));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "tax-records", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json(profile);
    }
    if (request.method === "GET" && url.pathname === "/v1/documents") {
      return json(documents);
    }
    if (request.method === "GET" && url.pathname === "/v1/forms/1098") {
      return json(missingForm);
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic tax-records API listening on http://127.0.0.1:${server.port}`);
