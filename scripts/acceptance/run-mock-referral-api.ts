#!/usr/bin/env bun

// Synthetic referral-coordination service for PA-056.
// It cannot diagnose, prescribe, contact a real provider, or move money.

const port = Number(process.env.NOEMA_REFERRAL_PORT ?? "3778");
const fixtureVersion = "2026-09-09-referral-api-v1";

const profile = {
  case_id: "referral-001",
  patient_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  goal: "Arrange a feasible synthetic specialist referral without making clinical decisions.",
  decision_boundary:
    "Do not diagnose, select treatment, contact a real provider, or make a real appointment or payment.",
  referral: {
    referral_order_id: "referral-order-001",
    specialty: "Sleep medicine (synthetic)",
    order_date: "2026-09-05",
    expires_on: "2026-10-05",
    source_locator: "referral://orders/referral-order-001",
  },
  constraints: {
    plan_name: "Harbor Health PPO (synthetic)",
    max_travel_minutes: 60,
    max_transport_cost_usd: 60,
    arrival_buffer_minutes: 30,
    destination_scope: "referral-order,recent-tests,medication-list",
    source_locator: "referral://preferences/referral-001",
  },
};

const prerequisites = [
  {
    prerequisite_id: "prerequisite-001",
    name: "Referral order verification",
    required: true,
    status: "complete",
    due_on: "2026-09-12",
    booking_required: false,
    source_locator: "referral://prerequisites/prerequisite-001",
  },
  {
    prerequisite_id: "prerequisite-002",
    name: "Recent sleep diary upload",
    required: true,
    status: "missing",
    due_on: "2026-09-15",
    booking_required: true,
    source_locator: "referral://prerequisites/prerequisite-002",
  },
  {
    prerequisite_id: "prerequisite-003",
    name: "Insurance referral authorization",
    required: true,
    status: "complete",
    due_on: "2026-09-12",
    booking_required: false,
    source_locator: "referral://prerequisites/prerequisite-003",
  },
];

const records = {
  bundle_id: "record-bundle-001",
  bundle_status: "ready",
  destination_scope: "referral-order,recent-tests,medication-list",
  records: [
    {
      record_id: "record-001",
      record_kind: "referral_order",
      title: "Specialist referral order (synthetic)",
      recorded_on: "2026-09-05",
      status: "current",
      source_locator: "referral://records/record-001",
    },
    {
      record_id: "record-002",
      record_kind: "recent_test",
      title: "Recent sleep diary summary (synthetic)",
      recorded_on: "2026-09-08",
      status: "current",
      source_locator: "referral://records/record-002",
    },
    {
      record_id: "record-003",
      record_kind: "medication_list",
      title: "Current medication list (synthetic)",
      recorded_on: "2026-09-08",
      status: "current",
      source_locator: "referral://records/record-003",
    },
  ],
};

const network = [
  {
    provider_id: "provider-001",
    provider_name: "Northstar Sleep Clinic (synthetic)",
    plan_name: "Harbor Health PPO (synthetic)",
    in_network: true,
    accessibility: "ground-floor entrance",
    accepting_new_referrals: true,
    address: "100 Northstar Way (synthetic)",
    source_locator: "referral://network/provider-001",
  },
  {
    provider_id: "provider-002",
    provider_name: "Harbor Sleep Center (synthetic)",
    plan_name: "Harbor Health PPO (synthetic)",
    in_network: false,
    accessibility: "ground-floor entrance",
    accepting_new_referrals: true,
    address: "200 Harbor Road (synthetic)",
    source_locator: "referral://network/provider-002",
  },
  {
    provider_id: "provider-003",
    provider_name: "Summit Sleep Institute (synthetic)",
    plan_name: "Harbor Health PPO (synthetic)",
    in_network: true,
    accessibility: "stairs only",
    accepting_new_referrals: false,
    address: "300 Summit Avenue (synthetic)",
    source_locator: "referral://network/provider-003",
  },
];

const slots = [
  {
    slot_id: "slot-001",
    provider_id: "provider-002",
    provider_name: "Harbor Sleep Center (synthetic)",
    appointment_date: "2026-09-16",
    start_time: "09:00",
    timezone: "America/Los_Angeles",
    duration_minutes: 45,
    status: "open",
    in_network: false,
    travel_minutes: 25,
    source_locator: "referral://slots/slot-001",
  },
  {
    slot_id: "slot-002",
    provider_id: "provider-001",
    provider_name: "Northstar Sleep Clinic (synthetic)",
    appointment_date: "2026-09-17",
    start_time: "16:00",
    timezone: "America/Los_Angeles",
    duration_minutes: 45,
    status: "open",
    in_network: true,
    travel_minutes: 75,
    source_locator: "referral://slots/slot-002",
  },
  {
    slot_id: "slot-003",
    provider_id: "provider-001",
    provider_name: "Northstar Sleep Clinic (synthetic)",
    appointment_date: "2026-09-18",
    start_time: "14:00",
    timezone: "America/Los_Angeles",
    duration_minutes: 45,
    status: "open",
    in_network: true,
    travel_minutes: 45,
    source_locator: "referral://slots/slot-003",
  },
];

const transportOptions = [
  {
    option_id: "transport-001",
    kind: "public_transit",
    provider_label: "Metro route 8 (synthetic)",
    travel_minutes: 90,
    cost_usd: 4.5,
    availability: "available",
    arrival_buffer_minutes: 30,
    source_locator: "referral://transport/transport-001",
  },
  {
    option_id: "transport-002",
    kind: "rideshare",
    provider_label: "Harbor Ride (synthetic)",
    travel_minutes: 50,
    cost_usd: 42,
    availability: "available",
    arrival_buffer_minutes: 30,
    source_locator: "referral://transport/transport-002",
  },
  {
    option_id: "transport-003",
    kind: "shuttle",
    provider_label: "Summit Shuttle (synthetic)",
    travel_minutes: 40,
    cost_usd: 18,
    availability: "unavailable",
    arrival_buffer_minutes: 30,
    source_locator: "referral://transport/transport-003",
  },
];

let prerequisiteBooking:
  | {
      booking_id: string;
      receipt_id: string;
      prerequisite_id: string;
      status: string;
      scheduled_on: string;
      request_count: number;
    }
  | undefined;
let recordTransfer:
  | {
      transfer_id: string;
      receipt_id: string;
      bundle_id: string;
      destination_provider_id: string;
      status: string;
      request_count: number;
    }
  | undefined;
let appointmentBooking:
  | {
      booking_id: string;
      receipt_id: string;
      referral_order_id: string;
      slot_id: string;
      status: string;
      request_count: number;
    }
  | undefined;
let transportBooking:
  | {
      transport_booking_id: string;
      receipt_id: string;
      appointment_id: string;
      option_id: string;
      status: string;
      request_count: number;
    }
  | undefined;

const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic referral-coordination API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot diagnose, prescribe, make
clinical decisions, contact a real provider, make a real appointment, or move
money. All names, dates, records, slots, and costs are test data. Read routes
are safe. The four request routes record mock actions and require human
approval in the client.

## Read the referral and constraints

- GET /v1/profile returns case_id, patient_label, current_date, goal,
  decision_boundary, a referral order, and transport or destination constraints.
- GET /v1/prerequisites returns a JSON object with a prerequisites array. Each
  item has prerequisite_id, name, required, status, due_on, booking_required,
  and source_locator. prerequisite-002 is initially missing and must be
  scheduled before the referral appointment is booked.
- GET /v1/records returns bundle_id, bundle_status, destination_scope, and a
  records array. The three records are authorized for the documented scope only.
- GET /v1/network returns a JSON object with a network array. Only provider-001
  is in-network, accepting new referrals, and accessible.
- GET /v1/slots returns a JSON object with a slots array. slot-003 is the
  feasible in-network open slot: its travel time is 45 minutes. slot-001 is
  out-of-network and slot-002 exceeds the 60-minute travel limit.
- GET /v1/transport-options returns a JSON object with an options array.
  transport-002 is available, costs 42 USD, and takes 50 minutes. The stated
  constraints cap travel at 60 minutes and cost at 60 USD.

## Approved synthetic actions

- POST /v1/prerequisite-bookings accepts JSON prerequisite_id,
  preferred_date, and note. Use prerequisite-002 and a complete note. It
  returns booking_id, receipt_id, prerequisite_id, status, scheduled_on, and
  request_count. Repeating the same request returns the same receipt.
- POST /v1/record-transfers accepts JSON bundle_id, destination_provider_id,
  and authorization_note. Use record-bundle-001 and provider-001. It returns
  transfer_id, receipt_id, bundle_id, destination_provider_id, status, and
  request_count. It transfers only the documented bundle scope.
- POST /v1/appointments accepts JSON referral_order_id, slot_id, and note.
  It requires the prerequisite booking and slot-003. It returns booking_id,
  receipt_id, referral_order_id, slot_id, status, and request_count.
- POST /v1/transport-bookings accepts JSON appointment_id, option_id, and
  note. Use the returned appointment booking ID and transport-002. It returns
  transport_booking_id, receipt_id, appointment_id, option_id, status, and
  request_count.
- GET /v1/follow-up-status returns the four action statuses and receipt IDs,
  the booked slot and transport option, the next_follow_up_date, and a source
  locator. It is safe to call after the approved actions.

There are no other routes. Do not propose diagnosis, treatment, prescription,
provider contact, real scheduling, real transport, payment, or undocumented
operations.
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
      return json({ ok: true, synthetic: true, service: "referral", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/prerequisites") return json({ prerequisites });
    if (request.method === "GET" && url.pathname === "/v1/records") return json(records);
    if (request.method === "GET" && url.pathname === "/v1/network") return json({ network });
    if (request.method === "GET" && url.pathname === "/v1/slots") return json({ slots });
    if (request.method === "GET" && url.pathname === "/v1/transport-options") return json({ options: transportOptions });

    if (request.method === "POST" && url.pathname === "/v1/prerequisite-bookings") {
      const input = (body ?? {}) as Record<string, unknown>;
      const prerequisiteId = typeof input.prerequisite_id === "string" ? input.prerequisite_id : "";
      const preferredDate = typeof input.preferred_date === "string" ? input.preferred_date : "";
      const note = typeof input.note === "string" ? input.note : "";
      if (prerequisiteId !== "prerequisite-002" || preferredDate !== "2026-09-15" || note.length < 12) {
        return json({ error: "invalid_prerequisite_booking", message: "Use prerequisite-002, 2026-09-15, and a complete note." }, 422);
      }
      if (!prerequisiteBooking) {
        prerequisiteBooking = {
          booking_id: "prerequisite-booking-001",
          receipt_id: "prerequisite-receipt-001",
          prerequisite_id: prerequisiteId,
          status: "scheduled",
          scheduled_on: preferredDate,
          request_count: 1,
        };
      }
      return json(prerequisiteBooking, 201);
    }

    if (request.method === "POST" && url.pathname === "/v1/record-transfers") {
      const input = (body ?? {}) as Record<string, unknown>;
      const bundleId = typeof input.bundle_id === "string" ? input.bundle_id : "";
      const destination = typeof input.destination_provider_id === "string" ? input.destination_provider_id : "";
      const note = typeof input.authorization_note === "string" ? input.authorization_note : "";
      if (bundleId !== "record-bundle-001" || destination !== "provider-001" || note.length < 12) {
        return json({ error: "invalid_record_transfer", message: "Use record-bundle-001, provider-001, and a complete authorization note." }, 422);
      }
      if (!recordTransfer) {
        recordTransfer = {
          transfer_id: "record-transfer-001",
          receipt_id: "transfer-receipt-001",
          bundle_id: bundleId,
          destination_provider_id: destination,
          status: "submitted",
          request_count: 1,
        };
      }
      return json(recordTransfer, 201);
    }

    if (request.method === "POST" && url.pathname === "/v1/appointments") {
      const input = (body ?? {}) as Record<string, unknown>;
      const orderId = typeof input.referral_order_id === "string" ? input.referral_order_id : "";
      const slotId = typeof input.slot_id === "string" ? input.slot_id : "";
      const note = typeof input.note === "string" ? input.note : "";
      if (orderId !== "referral-order-001" || slotId !== "slot-003" || note.length < 12) {
        return json({ error: "invalid_appointment_booking", message: "Use referral-order-001, slot-003, and a complete note." }, 422);
      }
      if (!prerequisiteBooking) return json({ error: "prerequisite_incomplete", message: "Schedule prerequisite-002 first." }, 409);
      if (!appointmentBooking) {
        appointmentBooking = {
          booking_id: "appointment-booking-001",
          receipt_id: "appointment-receipt-001",
          referral_order_id: orderId,
          slot_id: slotId,
          status: "booked",
          request_count: 1,
        };
      }
      return json(appointmentBooking, 201);
    }

    if (request.method === "POST" && url.pathname === "/v1/transport-bookings") {
      const input = (body ?? {}) as Record<string, unknown>;
      const appointmentId = typeof input.appointment_id === "string" ? input.appointment_id : "";
      const optionId = typeof input.option_id === "string" ? input.option_id : "";
      const note = typeof input.note === "string" ? input.note : "";
      if (appointmentId !== "appointment-booking-001" || optionId !== "transport-002" || note.length < 12) {
        return json({ error: "invalid_transport_booking", message: "Use appointment-booking-001, transport-002, and a complete note." }, 422);
      }
      if (!appointmentBooking) return json({ error: "appointment_missing", message: "Book the specialist appointment first." }, 409);
      if (!transportBooking) {
        transportBooking = {
          transport_booking_id: "transport-booking-001",
          receipt_id: "transport-receipt-001",
          appointment_id: appointmentId,
          option_id: optionId,
          status: "booked",
          request_count: 1,
        };
      }
      return json(transportBooking, 201);
    }

    if (request.method === "GET" && url.pathname === "/v1/follow-up-status") {
      return json({
        prerequisite_status: prerequisiteBooking?.status ?? "not_started",
        prerequisite_receipt_id: prerequisiteBooking?.receipt_id ?? "not_available",
        transfer_status: recordTransfer?.status ?? "not_started",
        transfer_receipt_id: recordTransfer?.receipt_id ?? "not_available",
        appointment_status: appointmentBooking?.status ?? "not_started",
        appointment_receipt_id: appointmentBooking?.receipt_id ?? "not_available",
        booked_slot_id: appointmentBooking?.slot_id ?? "not_available",
        transport_status: transportBooking?.status ?? "not_started",
        transport_receipt_id: transportBooking?.receipt_id ?? "not_available",
        booked_transport_option_id: transportBooking?.option_id ?? "not_available",
        next_follow_up_date: appointmentBooking ? "2026-09-26" : "not_scheduled",
        source_locator: "referral://status/referral-001",
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic referral API listening on http://127.0.0.1:${server.port}`);
