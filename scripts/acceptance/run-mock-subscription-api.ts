#!/usr/bin/env bun

// Synthetic subscription service for PA-043.
// It cannot bill a card, cancel a real service, or affect a real account.

const port = Number(process.env.NOEMA_SUBSCRIPTIONS_PORT ?? "3765");
const fixtureVersion = "2026-09-08-subscription-api-v1";

type Subscription = {
  subscription_id: string;
  service_name: string;
  plan_name: string;
  billing_cadence: "monthly" | "annual";
  status: "active" | "canceled";
  amount: number;
  currency: "USD";
  next_billing_date: string | null;
  commitment_end: string | null;
  cancellation_fee: number;
  cancellation_policy: string;
  started_on: string;
  source_locator: string;
};

const profile = {
  account_id: "account-subscriptions-001",
  account_label: "Jordan Lee (synthetic)",
  currency: "USD",
  as_of: "2026-09-08",
};

const subscriptions: Subscription[] = [
  {
    subscription_id: "sub-stream-monthly",
    service_name: "Streambox",
    plan_name: "Streambox Premium Monthly",
    billing_cadence: "monthly",
    status: "active",
    amount: 18.99,
    currency: "USD",
    next_billing_date: "2026-10-01",
    commitment_end: null,
    cancellation_fee: 0,
    cancellation_policy: "Cancel any time before the next billing date. No cancellation fee.",
    started_on: "2026-04-01",
    source_locator: "subscriptions://accounts/account-subscriptions-001/sub-stream-monthly",
  },
  {
    subscription_id: "sub-stream-annual",
    service_name: "Streambox",
    plan_name: "Streambox Premium Annual",
    billing_cadence: "annual",
    status: "active",
    amount: 179,
    currency: "USD",
    next_billing_date: "2027-01-01",
    commitment_end: "2027-01-01",
    cancellation_fee: 120,
    cancellation_policy: "Annual commitment remains active through the commitment end date.",
    started_on: "2026-01-01",
    source_locator: "subscriptions://accounts/account-subscriptions-001/sub-stream-annual",
  },
];

const cancellationReceipts: Array<{
  cancellation_id: string;
  subscription_id: string;
  plan_name: string;
  status: "confirmed";
  effective_date: string;
  next_billing_date: string;
  future_charge_status: "stopped";
  cancellation_fee: number;
  currency: "USD";
  reason: string;
  source_locator: string;
}> = [];

let cycleAdvanced = false;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic subscription API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot bill a card, cancel a real
service, or expose a real account. All amounts are US dollars.

## Account

- GET /v1/profile returns account_id, account_label, currency, and as_of.

## Subscriptions

- GET /v1/subscriptions returns the account's current subscription records.
  Each record has subscription_id, service_name, plan_name, billing_cadence,
  status, amount, currency, next_billing_date, commitment_end,
  cancellation_fee, cancellation_policy, started_on, and source_locator.
- The monthly Streambox plan can be canceled before its next billing date with
  no fee. The annual Streambox plan is committed through its commitment end and
  must not be canceled as part of a monthly-plan request.
- POST /v1/subscriptions/{subscription_id}/cancel cancels one eligible
  subscription. It accepts an optional JSON reason and returns a cancellation
  receipt. The operation changes synthetic state and must be approved.
- GET /v1/cancellations/{cancellation_id} returns the saved cancellation
  receipt.

## Later billing cycle

- GET /v1/billing-events returns the next-cycle result for each subscription.
  After the test operator advances the fixture clock, a canceled monthly plan
  has status skipped_cancelled and amount zero. The annual plan remains active
  and is not due until its commitment end date.

The customer API has no payment or reactivation operation.

## Test operator control

- POST /__test__/advance-cycle is an operator-only fixture control. It is not a
  customer operation and must not be proposed as a connection operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function findSubscription(subscriptionId: string) {
  return subscriptions.find((subscription) => subscription.subscription_id === subscriptionId);
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
      return json({ ok: true, synthetic: true, service: "subscriptions", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json(profile);
    }
    if (request.method === "GET" && url.pathname === "/v1/subscriptions") {
      return json({ subscriptions });
    }
    if (request.method === "GET" && url.pathname === "/v1/billing-events") {
      const events = cycleAdvanced
        ? subscriptions.map((subscription) => ({
            subscription_id: subscription.subscription_id,
            plan_name: subscription.plan_name,
            billing_date: "2026-10-01",
            status: subscription.billing_cadence === "monthly" ? "skipped_cancelled" : "not_due",
            amount: 0,
            currency: subscription.currency,
            source_locator: `subscriptions://billing-events/2026-10-01/${subscription.subscription_id}`,
          }))
        : [];
      return json({ cycle_date: "2026-10-01", cycle_advanced: cycleAdvanced, events });
    }
    if (request.method === "GET" && url.pathname.startsWith("/v1/cancellations/")) {
      const cancellationId = url.pathname.slice("/v1/cancellations/".length);
      const receipt = cancellationReceipts.find(
        (candidate) => candidate.cancellation_id === cancellationId,
      );
      return receipt ? json(receipt) : json({ error: "not_found" }, 404);
    }
    if (request.method === "POST" && url.pathname === "/__test__/advance-cycle") {
      cycleAdvanced = true;
      return json({ ok: true, cycle_date: "2026-10-01", synthetic: true });
    }
    if (request.method === "POST" && url.pathname.startsWith("/v1/subscriptions/") && url.pathname.endsWith("/cancel")) {
      const prefix = "/v1/subscriptions/";
      const subscriptionId = url.pathname.slice(prefix.length, -"/cancel".length);
      const subscription = findSubscription(subscriptionId);
      if (!subscription) return json({ error: "not_found" }, 404);
      if (subscription.billing_cadence !== "monthly") {
        return json({
          error: "annual_commitment",
          message: "The annual plan remains committed through 2027-01-01.",
          subscription_id: subscription.subscription_id,
        }, 409);
      }
      if (subscription.status === "canceled") {
        const existing = cancellationReceipts.find(
          (receipt) => receipt.subscription_id === subscription.subscription_id,
        );
        return existing ? json(existing) : json({ error: "already_canceled" }, 409);
      }
      const reason = typeof body === "object" && body !== null && "reason" in body && typeof body.reason === "string"
        ? body.reason
        : "User no longer uses the monthly subscription.";
      subscription.status = "canceled";
      const receipt = {
        cancellation_id: `cancel-${cancellationReceipts.length + 1}`,
        subscription_id: subscription.subscription_id,
        plan_name: subscription.plan_name,
        status: "confirmed" as const,
        effective_date: "2026-09-08",
        next_billing_date: subscription.next_billing_date ?? "2026-10-01",
        future_charge_status: "stopped" as const,
        cancellation_fee: subscription.cancellation_fee,
        currency: subscription.currency,
        reason,
        source_locator: `subscriptions://cancellations/cancel-${cancellationReceipts.length + 1}`,
      };
      subscription.next_billing_date = null;
      cancellationReceipts.push(receipt);
      return json(receipt, 201);
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic subscription API listening on http://127.0.0.1:${server.port}`);
