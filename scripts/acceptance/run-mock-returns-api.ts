#!/usr/bin/env bun

// Synthetic returns service for PA-072.
// It cannot contact a merchant, carrier, bank, or payment instrument.

const port = Number(process.env.NOEMA_RETURNS_PORT ?? "3794");
const fixtureVersion = "2026-09-09-returns-api-v1";

const profile = {
  case_id: "return-001",
  // Keep the descriptive source name and the connector's stable field name.
  // Both are ordinary synthetic data; no credential or private data is here.
  owner_label: "Jordan Lee (synthetic)",
  owner: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  goal: "Return the faulty keyboard and track the expected refund without mixing it with a similar purchase.",
  decision_boundary: "Synthetic only; do not contact a merchant or carrier, send a parcel, authorize a payment, or move money.",
  target_description: "KeyNest Compact Keyboard",
  source_locator: "return://profiles/return-001",
};

const purchases = [
  {
    purchase_id: "purchase-001",
    item_name: "KeyNest Compact Keyboard",
    item_variant: "compact wireless",
    item_serial: "SYNTH-KEY-001",
    serial_number: "SYNTH-KEY-001",
    purchase_date: "2026-08-25",
    return_deadline: "2026-09-15",
    item_condition: "faulty_intermittent_keys",
    condition: "faulty_intermittent_keys",
    return_eligible: true,
    eligible: true,
    final_sale: false,
    seller_label: "Northstar Goods (synthetic)",
    seller: "Northstar Goods (synthetic)",
    original_charge_id: "charge-001",
    original_amount_usd: 80,
    amount_usd: 80,
    source_locator: "return://purchases/purchase-001",
  },
  {
    purchase_id: "purchase-002",
    item_name: "KeyNest Compact Keyboard",
    item_variant: "compact wireless",
    item_serial: "SYNTH-KEY-002",
    serial_number: "SYNTH-KEY-002",
    purchase_date: "2026-07-01",
    return_deadline: "2026-07-31",
    item_condition: "working",
    condition: "working",
    return_eligible: false,
    eligible: false,
    final_sale: true,
    seller_label: "Northstar Goods (synthetic)",
    seller: "Northstar Goods (synthetic)",
    original_charge_id: "charge-002",
    original_amount_usd: 80,
    amount_usd: 80,
    source_locator: "return://purchases/purchase-002",
  },
];

const returnPolicy = {
  policy_id: "return-policy-001",
  return_window_days: 21,
  window_days: 21,
  current_date: "2026-09-09",
  allowed_reason_codes: "faulty,wrong_item,changed_mind",
  allowed_reasons: "faulty,wrong_item,changed_mind",
  shipping_required: true,
  refund_basis: "original_item_charge",
  restocking_fee_usd: 0,
  source_locator: "return://policies/return-policy-001",
};

const purchaseReceipts = [
  {
    receipt_id: "receipt-001",
    purchase_id: "purchase-001",
    original_charge_id: "charge-001",
    charged_amount_usd: 80,
    item_amount_usd: 80,
    shipping_amount_usd: 0,
    payment_status: "charged_in_synthetic_record",
    receipt_date: "2026-08-25",
    source_locator: "return://receipts/receipt-001",
  },
  {
    receipt_id: "receipt-002",
    purchase_id: "purchase-002",
    original_charge_id: "charge-002",
    charged_amount_usd: 80,
    item_amount_usd: 80,
    shipping_amount_usd: 0,
    payment_status: "charged_in_synthetic_record",
    receipt_date: "2026-07-01",
    source_locator: "return://receipts/receipt-002",
  },
];

const shippingOptions = [
  {
    return_option_id: "return-option-001",
    purchase_id: "purchase-001",
    method: "prepaid_standard_label",
    carrier_label: "Synthetic Parcel",
    carrier: "Synthetic Parcel",
    label_cost_usd: 0,
    delivery_estimate: "3-5 synthetic days",
    option_status: "eligible",
    status: "eligible",
    source_locator: "return://shipping-options/return-option-001",
  },
  {
    return_option_id: "return-option-002",
    purchase_id: "purchase-002",
    method: "prepaid_standard_label",
    carrier_label: "Synthetic Parcel",
    carrier: "Synthetic Parcel",
    label_cost_usd: 0,
    delivery_estimate: "3-5 synthetic days",
    option_status: "purchase_not_eligible",
    status: "purchase_not_eligible",
    source_locator: "return://shipping-options/return-option-002",
  },
];

const returnHistory: Array<Record<string, unknown>> = [];
type ReturnConfirmation = {
  return_id: string;
  purchase_id: string;
  original_charge_id: string;
  return_status: string;
  label_id: string;
  carrier_label: string;
  carrier: string;
  tracking_number: string;
  shipping_receipt_id: string;
  expected_refund_usd: number;
  submission_count: number;
  source_locator: string;
};

let confirmation: ReturnConfirmation | undefined;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];
const expectedBody = {
  case_id: "return-001",
  purchase_id: "purchase-001",
  reason_code: "faulty",
  return_option_id: "return-option-001",
  original_charge_id: "charge-001",
  expected_refund_usd: 80,
  approval_note: "Synthetic return only; no merchant contact, shipment, or payment.",
};

const docs = `# Synthetic returns API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot contact a merchant,
carrier, bank, or payment instrument. It cannot ship an item, issue a real
refund, authorize a payment, or move money. Work only in one personal Noema
workspace.

## Source records

- GET /v1/profile returns case return-001, owner, the current date, the target
  item, the goal, the decision boundary, and a source locator. "owner_label"
  is retained as a descriptive alias for the same synthetic owner.
- GET /v1/purchases returns two similar KeyNest Compact Keyboard purchases.
  purchase-001 has serial SYNTH-KEY-001, faulty intermittent keys, a
  2026-09-15 return deadline, and is eligible. purchase-002 has serial
  SYNTH-KEY-002, is working and final sale, and its 2026-07-31 deadline has
  passed. Use purchase IDs and serials, not the shared item name. The response
  exposes stable connector names "serial_number", "condition", "eligible",
  "seller", and "amount_usd", alongside descriptive source aliases.
- GET /v1/return-policy returns a 21-day window, allowed reason codes,
  required shipping, original-item-charge refund basis, and zero restocking
  fee. It exposes integer "window_days" and the comma-delimited string
  "allowed_reasons" for the connector, alongside the descriptive policy field
  names.
- GET /v1/purchase-receipts returns the original charge records. The eligible
  purchase maps to charge-001 and an 80 USD item charge. The other purchase
  maps to charge-002 and must not be used.
- GET /v1/shipping-options returns a prepaid standard label for purchase-001
  and a non-eligible option for purchase-002. Use return-option-001. It
  exposes connector names "carrier" and "status" alongside source aliases.
- GET /v1/return-history returns no prior return for this case.

## Synthetic return

- POST /v1/returns accepts the exact body fields case_id, purchase_id,
  reason_code, return_option_id, original_charge_id, expected_refund_usd, and
  approval_note.
- The valid body selects purchase-001, reason faulty, return-option-001,
  charge-001, and expected refund 80 USD. Repeating the exact body returns the
  same record and keeps submission_count at one.
- GET /v1/refund-status returns the return, tracking, shipping receipt, and
  expected 80 USD refund after the synthetic return is recorded. It returns
  HTTP 409 before the return exists. "carrier" is the connector field and
  "carrier_label" is retained as a descriptive alias.

There are no other routes. Do not propose a real return, shipment, merchant
contact, carrier contact, refund, payment, or money movement.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function sameBody(value: unknown) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(expectedBody);
  if (Object.keys(record).length !== keys.length) return false;
  return keys.every((key) => record[key] === expectedBody[key as keyof typeof expectedBody]);
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
      return json({ ok: true, synthetic: true, service: "returns", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/purchases") return json({ purchases });
    if (request.method === "GET" && url.pathname === "/v1/return-policy") return json(returnPolicy);
    if (request.method === "GET" && url.pathname === "/v1/purchase-receipts") return json({ receipts: purchaseReceipts });
    if (request.method === "GET" && url.pathname === "/v1/shipping-options") return json({ options: shippingOptions });
    if (request.method === "GET" && url.pathname === "/v1/return-history") return json({ returns: returnHistory });
    if (request.method === "POST" && url.pathname === "/v1/returns") {
      if (!sameBody(body)) return json({ error: "invalid synthetic return body" }, 400);
      if (!confirmation) {
        confirmation = {
          return_id: "return-record-001",
          purchase_id: "purchase-001",
          original_charge_id: "charge-001",
          return_status: "label_created_in_synthetic_record",
          label_id: "label-001",
          carrier_label: "Synthetic Parcel",
          carrier: "Synthetic Parcel",
          tracking_number: "SYNTH-TRACK-001",
          shipping_receipt_id: "shipping-receipt-001",
          expected_refund_usd: 80,
          submission_count: 1,
          source_locator: "return://returns/return-record-001",
        };
      }
      return json(confirmation, 201);
    }
    if (request.method === "GET" && url.pathname === "/v1/refund-status") {
      if (!confirmation) return json({ error: "return not recorded" }, 409);
      return json({
        return_id: confirmation.return_id,
        purchase_id: confirmation.purchase_id,
        original_charge_id: confirmation.original_charge_id,
        return_status: confirmation.return_status,
        label_id: confirmation.label_id,
        carrier_label: confirmation.carrier_label,
        carrier: confirmation.carrier,
        tracking_number: confirmation.tracking_number,
        shipping_receipt_id: confirmation.shipping_receipt_id,
        expected_refund_usd: confirmation.expected_refund_usd,
        original_charge_amount_usd: 80,
        refund_status: "expected_after_synthetic_inspection",
        submission_count: confirmation.submission_count,
        source_locator: "return://refund-status/return-record-001",
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic returns API listening on ${server.url}`);
