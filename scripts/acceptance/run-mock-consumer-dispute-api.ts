#!/usr/bin/env bun

// Synthetic consumer-dispute service for PA-050.
// It cannot contact a merchant, card issuer, regulator, or real complaint
// channel. Every record and state change is deterministic test data.

const port = Number(process.env.NOEMA_CONSUMER_DISPUTE_PORT ?? "3772");
const fixtureVersion = "2026-09-09-consumer-dispute-api-v1";

const profile = {
  account_id: "consumer-001",
  account_label: "Jordan Lee (synthetic)",
  goal: "Escalate an unresolved wrong-item purchase.",
  current_date: "2026-09-09",
  decision_boundary: "Do not contact a real merchant, card issuer, regulator, or complaint service.",
  source_locator: "consumer://accounts/consumer-001/profile",
};

const purchase = {
  purchase_id: "purchase-200-001",
  merchant: "Northstar Outdoor (synthetic)",
  item_ordered: "Trail jacket, blue, medium",
  item_received: "Trail jacket, red, small",
  issue: "Wrong item received",
  amount: 200,
  purchased_on: "2026-08-15",
  charged_status: "posted",
  return_deadline: "2026-09-15",
  complaint_deadline: "2026-09-30",
  source_locator: "consumer://purchases/purchase-200-001",
};

const contacts = [
  {
    contact_id: "contact-001",
    contacted_on: "2026-08-20",
    channel: "merchant chat",
    outcome: "failed",
    summary: "Merchant promised a reply within two days; no reply arrived.",
    reference: "chat-8841",
    source_locator: "consumer://contacts/contact-001",
  },
  {
    contact_id: "contact-002",
    contacted_on: "2026-08-27",
    channel: "merchant email",
    outcome: "failed",
    summary: "Merchant said the order matched its record and closed the request.",
    reference: "email-9917",
    source_locator: "consumer://contacts/contact-002",
  },
];

const returnPolicy = {
  policy_id: "return-policy-001",
  wrong_item_remedy: "refund or replacement",
  return_window_days: 30,
  return_deadline: "2026-09-15",
  merchant_response_days: 2,
  instructions: "Keep the wrong item and shipping label; do not send it until the mock channel confirms a label.",
  source_locator: "consumer://policies/return-policy-001",
};

const escalationChannels = [
  {
    channel_id: "merchant-escalation",
    label: "Merchant escalation",
    eligibility: "Use after a failed merchant contact.",
    expected_response_days: 3,
    source_locator: "consumer://channels/merchant-escalation",
  },
  {
    channel_id: "consumer-protection",
    label: "Consumer protection complaint",
    eligibility: "Use after two failed merchant contacts.",
    expected_response_days: 10,
    source_locator: "consumer://channels/consumer-protection",
  },
];

type Complaint = {
  complaint_id: string;
  receipt_id: string;
  purchase_id: string;
  channel_id: string;
  requested_remedy: string;
  status: "submitted";
  submission_count: number;
  source_locator: string;
};

let complaint: Complaint | undefined;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic consumer-dispute API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot contact a merchant, card
issuer, regulator, or real complaint channel. All records are test data. Read
routes are safe. The complaint POST records one mock complaint and requires
human approval in the client.

## Purchase and failed contacts

- GET /v1/profile returns the account goal and decision boundary.
- GET /v1/purchase returns the wrong-item purchase, its 200 amount, charge
  status, return deadline, and complaint deadline.
- GET /v1/merchant-contacts returns two failed merchant contacts.
- GET /v1/return-policy returns the wrong-item remedy and return instructions.
- GET /v1/escalation-channels returns channels and eligibility rules. The
  consumer-protection channel requires two failed merchant contacts.

## Mock complaint

- POST /v1/complaints accepts JSON purchase_id, channel_id, requested_remedy,
  chronology, and evidence_ids. It records one synthetic complaint and returns
  complaint_id, receipt_id, status, and submission_count. Repeating the same
  complaint returns the same receipt.
- GET /v1/complaints/{complaint_id} returns the submitted receipt. Its later
  status is resolved with a 200 refund approved, so a follow-up read verifies
  the mock remedy.

There is no real merchant, card, regulator, money, or complaint endpoint.
Do not propose an operator-only route.
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
    const requestEntry = {
      method: request.method,
      path: `${url.pathname}${url.search}`,
      ...(body === undefined ? {} : { body }),
    };
    requests.push(requestEntry);
    console.log(JSON.stringify(requestEntry));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "consumer-dispute", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/purchase") return json(purchase);
    if (request.method === "GET" && url.pathname === "/v1/merchant-contacts") return json({ contacts });
    if (request.method === "GET" && url.pathname === "/v1/return-policy") return json(returnPolicy);
    if (request.method === "GET" && url.pathname === "/v1/escalation-channels") return json({ channels: escalationChannels });
    if (request.method === "POST" && url.pathname === "/v1/complaints") {
      const input = (body ?? {}) as Record<string, unknown>;
      const purchaseId = String(input.purchase_id ?? "");
      const channelId = String(input.channel_id ?? "");
      const remedy = String(input.requested_remedy ?? "");
      const chronology = String(input.chronology ?? "");
      const evidenceIds = Array.isArray(input.evidence_ids) ? input.evidence_ids : [];
      if (
        purchaseId !== purchase.purchase_id ||
        channelId !== "consumer-protection" ||
        !remedy ||
        chronology.length < 24 ||
        evidenceIds.length < 2
      ) {
        return json({ error: "invalid_complaint", message: "Use the purchase, eligible channel, chronology, remedy, and both failed-contact IDs." }, 422);
      }
      if (!complaint) {
        complaint = {
          complaint_id: "complaint-consumer-001",
          receipt_id: "complaint-receipt-001",
          purchase_id: purchaseId,
          channel_id: channelId,
          requested_remedy: remedy,
          status: "submitted",
          submission_count: 1,
          source_locator: "consumer://complaints/complaint-consumer-001",
        };
      }
      return json(complaint, 201);
    }
    const complaintId = pathId(url.pathname, "/v1/complaints/");
    if (request.method === "GET" && complaintId) {
      if (!complaint || complaint.complaint_id !== complaintId) return json({ error: "not_found" }, 404);
      return json({
        ...complaint,
        status: "resolved",
        remedy: "refund_approved",
        refund_amount: 200,
        resolved_on: "2026-09-12",
        source_locator: "consumer://complaints/complaint-consumer-001/status",
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic consumer-dispute API listening on http://127.0.0.1:${server.port}`);
