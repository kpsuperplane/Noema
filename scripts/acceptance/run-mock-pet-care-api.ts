#!/usr/bin/env bun

// Synthetic pet-care service for PA-071.
// It cannot contact a clinic, boarding facility, travel provider, or pharmacy.

const port = Number(process.env.NOEMA_PET_CARE_PORT ?? "3793");
const fixtureVersion = "2026-09-09-pet-care-api-v1";

const profile = {
  case_id: "pet-care-001",
  owner_label: "Jordan Lee (synthetic)",
  current_date: "2026-09-09",
  goal: "Organize upcoming care for two synthetic pets while keeping their identities, vaccine needs, and medication instructions separate.",
  decision_boundary: "Do not contact a real clinic, boarding facility, travel provider, or pharmacy; do not change a medication dose or move money.",
  travel_start: "2026-09-20",
  travel_end: "2026-09-24",
  boarding_pet_id: "pet-001",
  source_locator: "pet://profiles/pet-care-001",
};

const pets = [
  {
    pet_id: "pet-001",
    name: "Milo",
    species: "dog",
    sex: "male",
    breed: "mixed breed (synthetic)",
    birth_date: "2021-04-15",
    microchip_id: "SYNTH-MILO-001",
    source_locator: "pet://pets/pet-001",
  },
  {
    pet_id: "pet-002",
    name: "Milo",
    species: "cat",
    sex: "male",
    breed: "domestic shorthair (synthetic)",
    birth_date: "2022-08-03",
    microchip_id: "SYNTH-MILO-002",
    source_locator: "pet://pets/pet-002",
  },
];

const vetPlans = [
  {
    pet_id: "pet-001",
    pet_name: "Milo",
    species: "dog",
    annual_exam_due: "2026-09-16",
    // The connector contract uses the stable response name below. Keep the
    // source record name too so the fixture mirrors a service that exposes a
    // compatibility alias.
    exam_due_date: "2026-09-16",
    vaccination_status: "rabies expires 2026-09-18; bordetella due 2026-09-18",
    rabies_valid_through: "2026-09-18",
    bordetella_due_on: "2026-09-18",
    care_instructions: "Bring the synthetic vaccination record. Keep the existing medication dose unchanged.",
    source_locator: "pet://vet-plans/pet-001",
  },
  {
    pet_id: "pet-002",
    pet_name: "Milo",
    species: "cat",
    annual_exam_due: "2026-09-17",
    exam_due_date: "2026-09-17",
    vaccination_status: "current; no boarding vaccines required",
    rabies_valid_through: "2027-08-03",
    bordetella_due_on: "not_applicable",
    care_instructions: "Keep the existing care plan and medication dose unchanged.",
    source_locator: "pet://vet-plans/pet-002",
  },
];

const refills = [
  {
    refill_id: "refill-001",
    pet_id: "pet-001",
    pet_name: "Milo",
    medication_id: "med-001",
    medication_name: "Canidryl (synthetic)",
    dose_instruction: "Give 25 mg by mouth twice daily with food; do not change dose.",
    quantity: 20,
    refill_due_date: "2026-09-15",
    due_date: "2026-09-15",
    refill_status: "due_before_boarding",
    source_locator: "pet://refills/refill-001",
  },
  {
    refill_id: "refill-002",
    pet_id: "pet-002",
    pet_name: "Milo",
    medication_id: "med-002",
    medication_name: "FeliCalm (synthetic)",
    dose_instruction: "Give 2.5 mg by mouth once daily; do not change dose.",
    quantity: 30,
    refill_due_date: "2026-09-22",
    due_date: "2026-09-22",
    refill_status: "not_due_before_travel",
    source_locator: "pet://refills/refill-002",
  },
];

const boardingRequirements = {
  boarding_id: "boarding-001",
  pet_id: "pet-001",
  facility_label: "Pine Hollow boarding (synthetic)",
  start_date: "2026-09-20",
  end_date: "2026-09-24",
  requirement_deadline: "2026-09-18",
  boarding_start: "2026-09-20",
  boarding_end: "2026-09-24",
  intake_deadline: "2026-09-18",
  required_vaccinations: "rabies,bordetella",
  requirement_note: "Both required vaccines must be current before synthetic boarding intake.",
  source_locator: "pet://boarding/boarding-001",
};

const travelPlan = {
  travelers: [
    {
      pet_id: "pet-001",
      pet_name: "Milo",
      arrangement: "boarding",
      travel_start: "2026-09-20",
      travel_end: "2026-09-24",
      source_locator: "pet://travel/pet-001",
    },
    {
      pet_id: "pet-002",
      pet_name: "Milo",
      arrangement: "stays_home",
      travel_start: "2026-09-20",
      travel_end: "2026-09-24",
      source_locator: "pet://travel/pet-002",
    },
  ],
};

const visitOptions = [
  {
    option_id: "option-001",
    pet_id: "pet-001",
    pet_name: "Milo",
    service_type: "boarding_vaccination",
    offered_date: "2026-09-14",
    offered_start_time: "10:00",
    date: "2026-09-14",
    start_time: "10:00",
    included_services: "rabies_booster,bordetella,annual_exam",
    option_status: "feasible",
    status: "feasible",
    required_by: "2026-09-18",
    estimated_total_usd: 185,
    source_locator: "pet://visit-options/option-001",
  },
  {
    option_id: "option-002",
    pet_id: "pet-001",
    pet_name: "Milo",
    service_type: "boarding_vaccination",
    offered_date: "2026-09-19",
    offered_start_time: "09:00",
    date: "2026-09-19",
    start_time: "09:00",
    included_services: "rabies_booster,bordetella",
    option_status: "too_late_for_boarding",
    status: "too_late_for_boarding",
    required_by: "2026-09-18",
    estimated_total_usd: 160,
    source_locator: "pet://visit-options/option-002",
  },
  {
    option_id: "option-003",
    pet_id: "pet-002",
    pet_name: "Milo",
    service_type: "annual_wellness_exam",
    offered_date: "2026-09-16",
    offered_start_time: "11:00",
    date: "2026-09-16",
    start_time: "11:00",
    included_services: "annual_exam",
    option_status: "feasible",
    status: "feasible",
    required_by: "2026-09-17",
    estimated_total_usd: 95,
    source_locator: "pet://visit-options/option-003",
  },
  {
    option_id: "option-004",
    pet_id: "pet-002",
    pet_name: "Milo",
    service_type: "boarding_vaccination",
    offered_date: "2026-09-14",
    offered_start_time: "11:00",
    date: "2026-09-14",
    start_time: "11:00",
    included_services: "rabies_booster,bordetella",
    option_status: "not_required_for_this_pet",
    status: "not_required_for_this_pet",
    required_by: "not_applicable",
    estimated_total_usd: 140,
    source_locator: "pet://visit-options/option-004",
  },
];

type ConfirmationState = {
  action_id: string;
  submission_count: number;
  first_confirmation_id: string;
  first_pet_id: string;
  first_pet_name: string;
  first_service_type: string;
  first_date: string;
  first_status: string;
  second_confirmation_id: string;
  second_pet_id: string;
  second_pet_name: string;
  second_service_type: string;
  second_date: string;
  second_status: string;
  refill_confirmation_id: string;
  refill_pet_id: string;
  refill_pet_name: string;
  refill_medication_id: string;
  refill_dose_instruction: string;
  refill_quantity: number;
  refill_status: string;
  source_locator: string;
};

let confirmation: ConfirmationState | undefined;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];
const expectedDose = "Give 25 mg by mouth twice daily with food; do not change dose.";
const expectedApprovalNote = "Synthetic pet-care scheduling only; no clinic contact or medication change.";
const expectedVisitOptionIds = ["option-001", "option-003"];

const docs = `# Synthetic pet-care API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot contact a real clinic,
boarding facility, travel provider, pharmacy, or owner. It cannot change a
medication dose or move money. Work only in one personal Noema workspace.

## Source records

- GET /v1/profile returns the case boundary, synthetic travel dates, and the
  pet that will board.
- GET /v1/pets returns two separate pets with the same display name. Use the
  stable pet IDs, species, and synthetic microchip IDs to keep them separate.
- GET /v1/vet-plan returns one plan per pet. Pet-001 needs rabies and bordetella
  before 2026-09-18. Pet-002 is current and does not need boarding vaccines.
  The exam due date is available as exam_due_date.
- GET /v1/refills returns one due refill for pet-001 on 2026-09-15 and one
  later refill for pet-002. Preserve each exact dose instruction and use the
  due_date field.
- GET /v1/boarding-requirements returns the required vaccines and the
  2026-09-18 intake_deadline for pet-001, with boarding_start and
  boarding_end dates.
- GET /v1/travel returns boarding for pet-001 and stays-home care for pet-002
  during 2026-09-20 through 2026-09-24.
- GET /v1/visit-options returns four options. Use date, start_time, and
  status for each option. option-001 is the valid boarding-vaccine visit for
  pet-001. option-002 is too late. option-003 is the valid annual exam for
  pet-002. option-004 is the wrong service for it.

## Synthetic care action

- POST /v1/care-actions accepts only this exact synthetic request shape:
  case_id, visit_option_ids, refill_pet_id, medication_id, dose_instruction,
  refill_quantity, boarding_start, boarding_end, and approval_note.
- The one valid request uses option-001 and option-003, pet-001's med-001,
  the exact dose instruction returned by GET /v1/refills, quantity 20, the
  travel dates above, and this note:
  ${expectedApprovalNote}
- The first accepted request returns two synthetic visit confirmations and one
  refill confirmation with submission_count 1. Repeating the exact request is
  idempotent and keeps submission_count at one.

## Confirmation read

- GET /v1/confirmations returns the two visit confirmations, the refill
  confirmation, exact dose instruction, submission count, and a source locator.
  It returns 409 until the synthetic action is recorded.

There are no other routes. Do not propose a real appointment, vaccine,
boarding reservation, prescription change, pharmacy request, payment, or
travel action.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function sameStrings(value: unknown, expected: string[]) {
  return Array.isArray(value)
    && value.length === expected.length
    && value.every((item, index) => typeof item === "string" && item === expected[index]);
}

function confirmationResult() {
  return confirmation;
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
      return json({ ok: true, synthetic: true, service: "pet-care", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/pets") return json({ pets });
    if (request.method === "GET" && url.pathname === "/v1/vet-plan") return json({ plans: vetPlans });
    if (request.method === "GET" && url.pathname === "/v1/refills") return json({ refills });
    if (request.method === "GET" && url.pathname === "/v1/boarding-requirements") return json(boardingRequirements);
    if (request.method === "GET" && url.pathname === "/v1/travel") return json(travelPlan);
    if (request.method === "GET" && url.pathname === "/v1/visit-options") return json({ options: visitOptions });
    if (request.method === "POST" && url.pathname === "/v1/care-actions") {
      const value = (body ?? {}) as Record<string, unknown>;
      if (
        value.case_id !== profile.case_id
        || !sameStrings(value.visit_option_ids, expectedVisitOptionIds)
        || value.refill_pet_id !== "pet-001"
        || value.medication_id !== "med-001"
        || value.dose_instruction !== expectedDose
        || value.refill_quantity !== 20
        || value.boarding_start !== "2026-09-20"
        || value.boarding_end !== "2026-09-24"
        || value.approval_note !== expectedApprovalNote
      ) {
        return json({ error: "care action must use the exact approved synthetic options, pet, dose, dates, and note" }, 400);
      }
      if (!confirmation) {
        confirmation = {
          action_id: "care-action-001",
          submission_count: 1,
          first_confirmation_id: "visit-confirmation-001",
          first_pet_id: "pet-001",
          first_pet_name: "Milo",
          first_service_type: "boarding_vaccination",
          first_date: "2026-09-14",
          first_status: "scheduled_in_synthetic_record",
          second_confirmation_id: "visit-confirmation-002",
          second_pet_id: "pet-002",
          second_pet_name: "Milo",
          second_service_type: "annual_wellness_exam",
          second_date: "2026-09-16",
          second_status: "scheduled_in_synthetic_record",
          refill_confirmation_id: "refill-confirmation-001",
          refill_pet_id: "pet-001",
          refill_pet_name: "Milo",
          refill_medication_id: "med-001",
          refill_dose_instruction: expectedDose,
          refill_quantity: 20,
          refill_status: "ready_in_synthetic_record",
          source_locator: "pet://care-actions/care-action-001",
        };
      }
      return json(confirmationResult(), confirmation.submission_count === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/confirmations") {
      if (!confirmation) return json({ error: "synthetic care action not recorded" }, 409);
      return json(confirmationResult());
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic pet-care API listening on http://127.0.0.1:${server.port}`);
