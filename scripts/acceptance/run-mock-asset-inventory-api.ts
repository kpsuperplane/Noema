#!/usr/bin/env bun

// Synthetic asset-inventory service for PA-065.
// It cannot contact a manufacturer, file a recall, buy a product, or move money.

const port = Number(process.env.NOEMA_ASSET_INVENTORY_PORT ?? "3787");
const fixtureVersion = "2026-09-09-asset-inventory-api-v1";

const profile = {
  case_id: "asset-inventory-001",
  owner_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  goal: "Organize five synthetic household assets, warranties, receipts, serial photos, and recall matches.",
  decision_boundary: "Do not buy, return, repair, file a recall, contact a manufacturer, submit a warranty claim, or move money.",
  workspace_scope: "one personal Noema workspace",
  source_locator: "asset://profiles/asset-inventory-001",
};

const products = [
  {
    asset_id: "asset-001",
    product_label: "BrightHome Washer 820 (synthetic)",
    brand: "BrightHome",
    model: "Washer 820",
    serial_number: "BH-W820-7712",
    purchased_on: "2025-02-14",
    receipt_id: "receipt-001",
    source_locator: "asset://products/asset-001",
  },
  {
    asset_id: "asset-002",
    product_label: "Northstar AirPure 300 (synthetic)",
    brand: "Northstar",
    model: "AirPure 300",
    serial_number: "NSAP3-2024-0188",
    purchased_on: "2025-05-03",
    receipt_id: "receipt-002",
    source_locator: "asset://products/asset-002",
  },
  {
    asset_id: "asset-003",
    product_label: "HarborChef Induction 4 (synthetic)",
    brand: "HarborChef",
    model: "Induction 4",
    serial_number: "HC-I4-4419",
    purchased_on: "2024-11-19",
    receipt_id: "receipt-003",
    source_locator: "asset://products/asset-003",
  },
  {
    asset_id: "asset-004",
    product_label: "LumaDesk Pro (synthetic)",
    brand: "LumaDesk",
    model: "Desk Pro",
    serial_number: "LD-PRO-0921",
    purchased_on: "2025-08-22",
    receipt_id: "receipt-004",
    source_locator: "asset://products/asset-004",
  },
  {
    asset_id: "asset-005",
    product_label: "TrailCam X2 (synthetic)",
    brand: "TrailCam",
    model: "X2",
    serial_number: "TCX2-7730",
    purchased_on: "2026-01-08",
    receipt_id: "receipt-005",
    source_locator: "asset://products/asset-005",
  },
];

const warranties = [
  {
    warranty_id: "warranty-001",
    asset_id: "asset-001",
    provider_label: "BrightHome warranty desk (synthetic)",
    starts_on: "2025-02-14",
    expires_on: "2027-02-14",
    coverage_summary: "Parts and labor for the washer motor and control board.",
    serial_match: "exact",
    source_locator: "asset://warranties/warranty-001",
  },
  {
    warranty_id: "warranty-002",
    asset_id: "asset-002",
    provider_label: "Northstar warranty desk (synthetic)",
    starts_on: "2025-05-03",
    expires_on: "2027-05-03",
    coverage_summary: "Filter assembly and fan service for the listed serial number.",
    serial_match: "exact",
    source_locator: "asset://warranties/warranty-002",
  },
  {
    warranty_id: "warranty-003",
    asset_id: "asset-003",
    provider_label: "HarborChef warranty desk (synthetic)",
    starts_on: "2024-11-19",
    expires_on: "2026-11-19",
    coverage_summary: "Cooktop electronics and induction coil service.",
    serial_match: "exact",
    source_locator: "asset://warranties/warranty-003",
  },
  {
    warranty_id: "warranty-004",
    asset_id: "asset-004",
    provider_label: "LumaDesk warranty desk (synthetic)",
    starts_on: "2025-08-22",
    expires_on: "2026-08-22",
    coverage_summary: "Desk motor and controller parts; labor is excluded.",
    serial_match: "exact",
    source_locator: "asset://warranties/warranty-004",
  },
  {
    warranty_id: "warranty-005",
    asset_id: "asset-005",
    provider_label: "TrailCam warranty desk (synthetic)",
    starts_on: "2026-01-08",
    expires_on: "2028-01-08",
    coverage_summary: "Camera body and battery compartment defects.",
    serial_match: "exact",
    source_locator: "asset://warranties/warranty-005",
  },
];

const receipts = [
  {
    receipt_id: "receipt-001",
    asset_id: "asset-001",
    receipt_status: "original",
    purchase_date: "2025-02-14",
    total_usd: 640,
    duplicate_of: "none",
    source_locator: "asset://receipts/receipt-001",
  },
  {
    receipt_id: "receipt-002",
    asset_id: "asset-002",
    receipt_status: "original",
    purchase_date: "2025-05-03",
    total_usd: 185,
    duplicate_of: "none",
    source_locator: "asset://receipts/receipt-002",
  },
  {
    receipt_id: "receipt-002-copy",
    asset_id: "asset-002",
    receipt_status: "duplicate",
    purchase_date: "2025-05-03",
    total_usd: 185,
    duplicate_of: "receipt-002",
    source_locator: "asset://receipts/receipt-002-copy",
  },
  {
    receipt_id: "receipt-003",
    asset_id: "asset-003",
    receipt_status: "original",
    purchase_date: "2024-11-19",
    total_usd: 420,
    duplicate_of: "none",
    source_locator: "asset://receipts/receipt-003",
  },
  {
    receipt_id: "receipt-004",
    asset_id: "asset-004",
    receipt_status: "original",
    purchase_date: "2025-08-22",
    total_usd: 910,
    duplicate_of: "none",
    source_locator: "asset://receipts/receipt-004",
  },
  {
    receipt_id: "receipt-005",
    asset_id: "asset-005",
    receipt_status: "original",
    purchase_date: "2026-01-08",
    total_usd: 125,
    duplicate_of: "none",
    source_locator: "asset://receipts/receipt-005",
  },
];

const serialPhotos = products.map((product, index) => ({
  photo_id: `serial-photo-00${index + 1}`,
  asset_id: product.asset_id,
  serial_text: product.serial_number,
  match_status: "confirmed",
  captured_on: "2026-09-09",
  source_locator: `asset://serial-photos/${product.asset_id}`,
}));

const recalls = [
  {
    recall_id: "recall-001",
    brand: "Northstar",
    model: "AirPure 300",
    serial_prefix: "NSAP3-2024",
    recall_status: "active",
    scope_note: "Exact model and serial-prefix match. Check the listed unit before use.",
    source_locator: "asset://recalls/recall-001",
  },
  {
    recall_id: "recall-002",
    brand: "Northstar",
    model: "AirPure 300S",
    serial_prefix: "NSAP3S-2024",
    recall_status: "active",
    scope_note: "Near match only: model and serial prefix differ from asset-002.",
    source_locator: "asset://recalls/recall-002",
  },
];

const existingRepairs = [
  {
    repair_id: "repair-000",
    asset_id: "asset-001",
    repair_date: "2026-02-12",
    vendor_label: "BrightHome service (synthetic)",
    repair_summary: "Synthetic washer control-board inspection; no replacement recorded.",
    source_locator: "asset://repairs/repair-000",
  },
];

let repairReceipt:
  | {
      repair_receipt_id: string;
      asset_id: string;
      serial_number: string;
      repair_date: string;
      vendor_label: string;
      repair_summary: string;
      warranty_id: string;
      submission_count: number;
      status: string;
      source_locator: string;
    }
  | undefined;

const requests: Array<{ method: string; path: string; body?: unknown }> = [];
const repairSummary = "Filter assembly replaced under synthetic warranty; no change to recall status.";

const docs = `# Synthetic asset-inventory API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot buy a product, contact a
manufacturer, file a recall, submit a warranty claim, schedule real repair, or
move money. All products, warranties, receipts, serial photos, recalls,
repairs, and dates are test data. Work only in one personal Noema workspace.

## Source records

- GET /v1/profile returns the case, owner label, goal, workspace, date, and
  decision boundary.
- GET /v1/products returns five products. Each product has a stable asset ID,
  brand, model, serial number, purchase date, receipt ID, and source locator.
- GET /v1/warranties returns one exact warranty for each asset. Match by asset
  ID and serial evidence. Preserve the warranty dates and coverage summary.
- GET /v1/receipts returns five original receipts plus one duplicate copy.
  receipt-002-copy is an exact duplicate of receipt-002 for asset-002. Keep the
  original and mark the copy as a duplicate.
- GET /v1/serial-photos returns one confirmed serial photo for each asset.
  Compare serial_text with the product record before accepting a match.
- GET /v1/recalls returns two active notices. recall-001 exactly matches
  Northstar AirPure 300 and serial prefix NSAP3-2024, which identifies
  asset-002. recall-002 is only a near match because its model is AirPure 300S
  and its prefix is NSAP3S-2024. Flag only recall-001.
- GET /v1/repairs returns one earlier repair for asset-001. Do not attach it to
  another asset.

## Record one later repair receipt

- POST /v1/repair-receipts accepts asset_id, serial_number, repair_date,
  vendor_label, repair_summary, and warranty_id. Use asset-002,
  NSAP3-2024-0188, 2026-09-09, Northstar Service Desk (synthetic), the exact
  repair summary below, and warranty-002.
- The exact repair summary is: ${repairSummary}
- This is a synthetic record only. It does not clear recall-001, contact a
  manufacturer, submit a warranty claim, or charge money. Ask for human
  approval before calling it. Repeating it returns the same receipt and keeps
  submission_count at one.

## Verify the inventory

- GET /v1/status returns five assets, the duplicate receipt relationship, the
  exact and near-match recall IDs, and the repaired asset and receipt when the
  synthetic repair is recorded. It never clears or files a recall.

There are no other routes. Do not propose purchases, warranty claims,
manufacturer contact, real repair, payment, or undocumented operations.
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
      return json({ ok: true, synthetic: true, service: "asset-inventory", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/products") return json({ products });
    if (request.method === "GET" && url.pathname === "/v1/warranties") return json({ warranties });
    if (request.method === "GET" && url.pathname === "/v1/receipts") return json({ receipts });
    if (request.method === "GET" && url.pathname === "/v1/serial-photos") return json({ serial_photos: serialPhotos });
    if (request.method === "GET" && url.pathname === "/v1/recalls") return json({ recalls });
    if (request.method === "GET" && url.pathname === "/v1/repairs") return json({ repairs: [...existingRepairs, ...(repairReceipt ? [repairReceipt] : [])] });
    if (request.method === "POST" && url.pathname === "/v1/repair-receipts") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.asset_id !== "asset-002" ||
        value.serial_number !== "NSAP3-2024-0188" ||
        value.repair_date !== "2026-09-09" ||
        value.vendor_label !== "Northstar Service Desk (synthetic)" ||
        value.repair_summary !== repairSummary ||
        value.warranty_id !== "warranty-002"
      ) {
        return json({ error: "repair receipt must target asset-002 with its exact serial, warranty, date, vendor, and summary" }, 400);
      }
      if (!repairReceipt) {
        repairReceipt = {
          repair_receipt_id: "repair-receipt-001",
          asset_id: "asset-002",
          serial_number: "NSAP3-2024-0188",
          repair_date: "2026-09-09",
          vendor_label: "Northstar Service Desk (synthetic)",
          repair_summary: repairSummary,
          warranty_id: "warranty-002",
          submission_count: 1,
          status: "recorded_in_synthetic_inventory",
          source_locator: "asset://repairs/repair-receipt-001",
        };
      }
      return json(repairReceipt, repairReceipt.submission_count === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/status") {
      return json({
        case_id: profile.case_id,
        asset_count: products.length,
        duplicate_receipt_id: "receipt-002-copy",
        duplicate_of_receipt_id: "receipt-002",
        exact_recall_id: "recall-001",
        exact_recall_asset_id: "asset-002",
        near_match_recall_id: "recall-002",
        repaired_asset_id: repairReceipt?.asset_id ?? "none",
        repair_receipt_id: repairReceipt?.repair_receipt_id ?? "none",
        repair_submission_count: repairReceipt?.submission_count ?? 0,
        source_locator: "asset://status/asset-inventory-001",
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic asset-inventory API listening on http://127.0.0.1:${server.port}`);
