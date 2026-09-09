#!/usr/bin/env bun

// Synthetic vehicle lifecycle service for PA-070.
// It cannot contact a real vehicle owner, dealer, registry, or payment service.

const port = Number(process.env.NOEMA_VEHICLE_LIFECYCLE_PORT ?? "3792");
const fixtureVersion = "2026-09-09-vehicle-lifecycle-api-v1";

const profile = {
  case_id: "vehicle-lifecycle-001",
  owner_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  vehicle_id: "vehicle-001",
  vehicle_label: "Orion Vale hatchback (synthetic)",
  vehicle_serial: "SYNTH-VALE-001",
  current_mileage: 48620,
  service_interval_miles: 5000,
  registration_deadline: "2026-09-30",
  goal: "Keep the synthetic vehicle service interval and registration paperwork current while matching the vehicle to the correct recall.",
  decision_boundary: "Do not contact a real dealer or registry, expose a real VIN or address, schedule a real service, authorize a charge, or move money.",
  source_locator: "vehicle://profiles/vehicle-lifecycle-001",
};

const serviceHistory = [
  {
    service_id: "service-001",
    vehicle_id: "vehicle-001",
    service_date: "2025-07-18",
    mileage: 39210,
    service_type: "annual_inspection",
    vendor_label: "Orion Vale service center (synthetic)",
    notes: "Synthetic historical inspection; no open recall work was recorded.",
    source_locator: "vehicle://service-history/service-001",
  },
  {
    service_id: "service-002",
    vehicle_id: "vehicle-001",
    service_date: "2026-04-08",
    mileage: 43620,
    service_type: "scheduled_service",
    vendor_label: "Orion Vale service center (synthetic)",
    notes: "Synthetic interval service completed; next mileage interval is 48620.",
    source_locator: "vehicle://service-history/service-002",
  },
];

const recalls = [
  {
    recall_id: "recall-001",
    campaign_id: "RC-2026-04",
    vehicle_id: "vehicle-001",
    serial_prefix: "SYNTH-VALE",
    serial_start: "SYNTH-VALE-000",
    serial_end: "SYNTH-VALE-099",
    vehicle_serial: profile.vehicle_serial,
    matches_vehicle: true,
    severity: "safety",
    remedy: "Replace the synthetic brake-control module and record completion before registration renewal.",
    status: "open",
    source_locator: "vehicle://recalls/recall-001",
  },
  {
    recall_id: "recall-002",
    campaign_id: "RC-2025-11",
    vehicle_id: "vehicle-other-001",
    serial_prefix: "SYNTH-VALE",
    serial_start: "SYNTH-VALE-200",
    serial_end: "SYNTH-VALE-299",
    vehicle_serial: profile.vehicle_serial,
    matches_vehicle: false,
    severity: "service",
    remedy: "Synthetic infotainment update for a different serial range.",
    status: "not_applicable",
    source_locator: "vehicle://recalls/recall-002",
  },
];

const registration = {
  registration_id: "registration-001",
  vehicle_id: "vehicle-001",
  jurisdiction_label: "Synthetic jurisdiction",
  expires_on: "2026-09-30",
  status: "renewal_pending_service",
  required_documents: ["synthetic-insurance-proof", "synthetic-emissions-record"],
  open_recall_blocks_renewal: true,
  source_locator: "vehicle://registration/registration-001",
};

const serviceOptions = [
  {
    option_id: "option-001",
    vehicle_id: "vehicle-001",
    vendor_label: "Orion Vale service center (synthetic)",
    offered_date: "2026-09-16",
    offered_start_time: "09:00",
    duration_minutes: 150,
    included_services: ["scheduled_service", "RC-2026-04 brake-control remedy", "synthetic-emissions-record"],
    estimated_total_usd: 325,
    recall_campaign_id: "RC-2026-04",
    registration_ready: true,
    option_status: "feasible",
    source_locator: "vehicle://service-options/option-001",
  },
  {
    option_id: "option-002",
    vehicle_id: "vehicle-001",
    vendor_label: "QuickLane service (synthetic)",
    offered_date: "2026-09-12",
    offered_start_time: "15:00",
    duration_minutes: 75,
    included_services: ["scheduled_service"],
    estimated_total_usd: 210,
    recall_campaign_id: "none",
    registration_ready: false,
    option_status: "missing_recall_remedy",
    source_locator: "vehicle://service-options/option-002",
  },
  {
    option_id: "option-003",
    vehicle_id: "vehicle-other-001",
    vendor_label: "Orion Vale service center (synthetic)",
    offered_date: "2026-09-18",
    offered_start_time: "10:00",
    duration_minutes: 120,
    included_services: ["scheduled_service", "RC-2026-04 brake-control remedy"],
    estimated_total_usd: 290,
    recall_campaign_id: "RC-2026-04",
    registration_ready: true,
    option_status: "wrong_vehicle",
    source_locator: "vehicle://service-options/option-003",
  },
];

const constraints = {
  service_due_mileage: 48620,
  current_mileage: profile.current_mileage,
  registration_deadline: registration.expires_on,
  service_window_start: "2026-09-10",
  service_window_end: "2026-09-29",
  budget_limit_usd: 500,
  required_recall_campaign: "RC-2026-04",
  required_document: "synthetic-emissions-record",
  source_locator: "vehicle://constraints/vehicle-lifecycle-001",
};

let serviceReceipt:
  | {
      service_receipt_id: string;
      vehicle_id: string;
      option_id: string;
      service_date: string;
      odometer_miles: number;
      recall_campaign_id: string;
      estimated_total_usd: number;
      status: string;
      submission_count: number;
      next_service_mileage: number;
      next_service_due_date: string;
      registration_status: string;
      registration_deadline: string;
      source_locator: string;
    }
  | undefined;
let serviceSubmissionCount = 0;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];
const expectedApprovalNote = "Synthetic service booking only; no dealer contact or payment.";

const docs = `# Synthetic vehicle lifecycle API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot contact a dealer or registry,
expose a real VIN or address, schedule a real service, authorize a charge, or
move money. Work only in one personal Noema workspace.

## Source records

- GET /v1/profile returns one synthetic vehicle, current mileage 48620, a 5000
  mile interval, and a 2026-09-30 registration deadline.
- GET /v1/service-history returns two records. The latest service was at
  43620 miles on 2026-04-08, so the next interval is due at 48620 miles.
- GET /v1/recalls returns one open recall whose serial range matches
  SYNTH-VALE-001 and one record for a different serial range.
- GET /v1/registration returns a renewal-pending status. The open matching
  recall blocks renewal until the remedy and synthetic emissions record are
  recorded.
- GET /v1/service-options returns one feasible option that includes scheduled
  service, recall RC-2026-04, and the emissions record; a cheaper option omits
  the recall; and one option belongs to another vehicle.
- GET /v1/constraints returns the due mileage, service window, 500 USD limit,
  required recall, required document, and registration deadline.

## Synthetic service booking

- POST /v1/service-bookings accepts only vehicle-001, option-001, 2026-09-16,
  odometer 48620, recall RC-2026-04, registration deadline 2026-09-30,
  estimated total 325, and this exact note: ${expectedApprovalNote}
- The first accepted request returns service-receipt-001, submission_count 1,
  next_service_mileage 53620, and registration_status ready_for_renewal.
- Repeating the exact request returns the same receipt and keeps submission_count
  at one. It does not contact a real service provider.

## Verification

- GET /v1/service-receipt returns the synthetic receipt after the booking.
- GET /v1/status returns the completed recall, next service interval, and
  registration status after the booking.

There are no other routes. Do not propose a real repair, dealer contact,
registry update, charge, payment, or undocumented operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function receiptResult() {
  return (
    serviceReceipt ?? {
      service_receipt_id: "none",
      vehicle_id: profile.vehicle_id,
      option_id: "none",
      service_date: "none",
      odometer_miles: profile.current_mileage,
      recall_campaign_id: "none",
      estimated_total_usd: 0,
      status: "not_booked",
      submission_count: serviceSubmissionCount,
      next_service_mileage: constraints.service_due_mileage,
      next_service_due_date: "none",
      registration_status: registration.status,
      registration_deadline: registration.expires_on,
      source_locator: "vehicle://service-receipts/service-receipt-001",
    }
  );
}

function statusResult() {
  const booked = serviceReceipt !== undefined;
  return {
    vehicle_id: profile.vehicle_id,
    current_mileage: profile.current_mileage,
    last_service_date: booked ? serviceReceipt!.service_date : serviceHistory[1].service_date,
    last_service_mileage: booked ? serviceReceipt!.odometer_miles : serviceHistory[1].mileage,
    next_service_mileage: booked ? serviceReceipt!.next_service_mileage : constraints.service_due_mileage,
    next_service_due_date: booked ? serviceReceipt!.next_service_due_date : "due_now",
    recall_campaign_id: booked ? "RC-2026-04" : "RC-2026-04",
    recall_status: booked ? "completed_in_synthetic_record" : "open",
    registration_status: booked ? "ready_for_renewal" : registration.status,
    registration_deadline: registration.expires_on,
    service_submission_count: serviceSubmissionCount,
    source_locator: "vehicle://status/vehicle-lifecycle-001",
  };
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
      return json({ ok: true, synthetic: true, service: "vehicle-lifecycle", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/service-history") return json({ records: serviceHistory });
    if (request.method === "GET" && url.pathname === "/v1/recalls") return json({ recalls });
    if (request.method === "GET" && url.pathname === "/v1/registration") return json(registration);
    if (request.method === "GET" && url.pathname === "/v1/service-options") return json({ options: serviceOptions });
    if (request.method === "GET" && url.pathname === "/v1/constraints") return json(constraints);
    if (request.method === "GET" && url.pathname === "/v1/service-receipt") {
      return serviceReceipt ? json(receiptResult()) : json({ error: "not_booked" }, 409);
    }
    if (request.method === "GET" && url.pathname === "/v1/status") return json(statusResult());
    if (request.method === "POST" && url.pathname === "/v1/service-bookings") {
      const input = (body ?? {}) as Record<string, unknown>;
      const valid =
        input.vehicle_id === "vehicle-001" &&
        input.option_id === "option-001" &&
        input.service_date === "2026-09-16" &&
        input.odometer_miles === 48620 &&
        input.recall_campaign_id === "RC-2026-04" &&
        input.registration_deadline === "2026-09-30" &&
        input.estimated_total_usd === 325 &&
        input.approval_note === expectedApprovalNote;
      if (!valid) return json({ error: "invalid_synthetic_booking" }, 422);
      if (!serviceReceipt) {
        serviceSubmissionCount = 1;
        serviceReceipt = {
          service_receipt_id: "service-receipt-001",
          vehicle_id: "vehicle-001",
          option_id: "option-001",
          service_date: "2026-09-16",
          odometer_miles: 48620,
          recall_campaign_id: "RC-2026-04",
          estimated_total_usd: 325,
          status: "booked_in_synthetic_record",
          submission_count: serviceSubmissionCount,
          next_service_mileage: 53620,
          next_service_due_date: "2027-09-16",
          registration_status: "ready_for_renewal",
          registration_deadline: "2026-09-30",
          source_locator: "vehicle://service-receipts/service-receipt-001",
        };
      }
      return json(receiptResult());
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic vehicle lifecycle API listening on http://127.0.0.1:${server.port}`);
