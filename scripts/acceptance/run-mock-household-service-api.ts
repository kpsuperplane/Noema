#!/usr/bin/env bun

// Synthetic household-service API for PA-073.
// It cannot contact cleaners, enter a home, disclose an access code, or take payment.

const port = Number(process.env.NOEMA_HOUSEHOLD_SERVICE_PORT ?? "3795");
const fixtureVersion = "2026-09-10-household-service-api-v1";

const profile = {
  case_id: "household-service-001",
  owner: "Jordan Lee (synthetic)",
  current_date: "2026-09-10",
  home_label: "Jordan Lee household (synthetic)",
  pet_name: "Milo",
  pet_type: "dog",
  goal: "Arrange weekly home cleaning while keeping the dog safe and home-access details private.",
  source_locator: "household://profiles/household-service-001",
};

const constraints = {
  constraint_id: "household-constraints-001",
  recurrence: "weekly",
  preferred_day: "saturday",
  preferred_start_time: "10:00",
  preferred_end_time: "12:00",
  visit_ceiling_usd: 120,
  pet_requirement: "cleaner must accept a dog on site",
  access_requirement: "resident present; never put an access code in a provider request",
  access_note: "The resident supplies one-time access at arrival. Do not disclose stored access details.",
  source_locator: "household://constraints/household-constraints-001",
};

const providers = [
  {
    provider_id: "cleaner-001",
    provider_name: "BrightNest Cleaners (synthetic)",
    pet_policy: "accepts_dogs",
    access_modes: ["resident_present"],
    recurring_supported: true,
    insured: true,
    visit_price_usd: 110,
    available_day: "saturday",
    available_start_time: "10:00",
    available_end_time: "12:00",
    cancellation_policy: "provider may cancel; backup may replace the next visit after approval",
    source_locator: "household://providers/cleaner-001",
  },
  {
    provider_id: "cleaner-002",
    provider_name: "Quiet Harbor Home Care (synthetic)",
    pet_policy: "no_pets_on_site",
    access_modes: ["resident_present"],
    recurring_supported: true,
    insured: true,
    visit_price_usd: 90,
    available_day: "saturday",
    available_start_time: "09:00",
    available_end_time: "11:00",
    cancellation_policy: "provider may cancel; no backup guarantee",
    source_locator: "household://providers/cleaner-002",
  },
  {
    provider_id: "cleaner-003",
    provider_name: "Pine & Paw Services (synthetic)",
    pet_policy: "accepts_dogs",
    access_modes: ["resident_present"],
    recurring_supported: true,
    insured: true,
    visit_price_usd: 118,
    available_day: "sunday",
    available_start_time: "11:00",
    available_end_time: "13:00",
    cancellation_policy: "backup provider; one approved replacement visit",
    source_locator: "household://providers/cleaner-003",
  },
];

const availability = [
  {
    slot_id: "slot-001",
    provider_id: "cleaner-001",
    day: "saturday",
    start_time: "10:00",
    end_time: "12:00",
    visit_price_usd: 110,
    pet_policy: "accepts_dogs",
    access_mode: "resident_present",
    slot_status: "eligible_primary",
    source_locator: "household://availability/slot-001",
  },
  {
    slot_id: "slot-002",
    provider_id: "cleaner-002",
    day: "saturday",
    start_time: "09:00",
    end_time: "11:00",
    visit_price_usd: 90,
    pet_policy: "no_pets_on_site",
    access_mode: "resident_present",
    slot_status: "pet_constraint_failed",
    source_locator: "household://availability/slot-002",
  },
  {
    slot_id: "slot-003",
    provider_id: "cleaner-003",
    day: "sunday",
    start_time: "11:00",
    end_time: "13:00",
    visit_price_usd: 118,
    pet_policy: "accepts_dogs",
    access_mode: "resident_present",
    slot_status: "eligible_backup",
    source_locator: "household://availability/slot-003",
  },
];

type Schedule = {
  schedule_id: string;
  case_id: string;
  provider_id: string;
  recurrence: string;
  day: string;
  start_time: string;
  end_time: string;
  amount_usd: number;
  access_handling: string;
  pet_handling: string;
  status: string;
  submission_count: number;
  source_locator: string;
};

type ScheduleChange = {
  change_id: string;
  schedule_id: string;
  original_provider_id: string;
  replacement_provider_id: string;
  effective_date: string;
  recurrence: string;
  day: string;
  start_time: string;
  end_time: string;
  amount_usd: number;
  access_handling: string;
  pet_handling: string;
  status: string;
  submission_count: number;
  source_locator: string;
};

let schedule: Schedule | undefined;
let scheduleChange: ScheduleChange | undefined;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const scheduleBody = {
  access_handling: "resident_present",
  amount_usd: 110,
  approval_note: "Synthetic cleaning schedule only; no cleaner contact, home access, or payment.",
  case_id: "household-service-001",
  day: "saturday",
  end_time: "12:00",
  pet_handling: "pet_on_site",
  provider_id: "cleaner-001",
  recurrence: "weekly",
  start_time: "10:00",
};

const changeBody = {
  access_handling: "resident_present",
  amount_usd: 118,
  approval_note: "Synthetic backup scheduling only; no cleaner contact, home access, or payment.",
  case_id: "household-service-001",
  day: "sunday",
  effective_date: "2026-09-19",
  end_time: "13:00",
  pet_handling: "pet_on_site",
  recurrence: "weekly",
  replacement_provider_id: "cleaner-003",
  schedule_id: "cleaning-schedule-001",
  start_time: "11:00",
};

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function sameBody(value: unknown, expected: Record<string, unknown>) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(expected);
  return Object.keys(record).length === keys.length && keys.every((key) => record[key] === expected[key]);
}

const docs = `# Synthetic household service API

Fixture version: ${fixtureVersion}

This service is synthetic. It cannot contact a cleaner, enter a home, receive
an access code, charge a card, or move money. Use one personal Noema workspace.

## Source operations

- GET /v1/profile returns case household-service-001, the synthetic owner, the
  current date, one dog named Milo, the goal, and a source locator.
- GET /v1/providers returns three cleaners. cleaner-001 accepts dogs, supports
  recurring weekly visits, requires resident_present access, and costs 110 USD.
  cleaner-002 does not allow pets. cleaner-003 accepts dogs, supports
  recurring visits, requires resident_present access, and costs 118 USD.
- GET /v1/constraints returns the weekly preference, Saturday 10:00-12:00
  window, 120 USD visit ceiling, dog requirement, and the rule that access
  details must not enter a provider request.
- GET /v1/availability returns the three bounded slots. slot-001 is the
  eligible primary slot. slot-002 fails the pet constraint. slot-003 is an
  eligible Sunday backup slot.
- GET /v1/cancellation-policy returns the rule that a provider cancellation
  may be replaced once by an approved backup. It also says to preserve the
  access and pet boundaries.

All source responses are JSON. The connector contract is:

| Operation | Response fields |
| --- | --- |
| get_household_profile | case_id, owner, current_date, home_label, pet_name, pet_type, goal, source_locator (strings) |
| list_cleaners | providers[] with provider_id, provider_name, pet_policy, access_modes (string array), recurring_supported (boolean), insured (boolean), visit_price_usd (number), available_day, available_start_time, available_end_time, cancellation_policy, source_locator |
| get_household_constraints | constraint_id, recurrence, preferred_day, preferred_start_time, preferred_end_time, visit_ceiling_usd (number), pet_requirement, access_requirement, access_note, source_locator |
| list_cleaning_availability | availability[] with slot_id, provider_id, day, start_time, end_time, visit_price_usd (number), pet_policy, access_mode, slot_status, source_locator |
| get_cancellation_policy | policy_id, cancellation_rule, backup_limit (integer), preserve_access_boundary (boolean), preserve_pet_boundary (boolean), source_locator |
| create_recurring_schedule | schedule_id, case_id, provider_id, recurrence, day, start_time, end_time, amount_usd (number), access_handling, pet_handling, status, submission_count (integer), source_locator |
| list_schedule_events | events[] with event_id, schedule_id, provider_id, event_type, effective_date, message, source_locator |
| change_schedule_to_backup | change_id, schedule_id, original_provider_id, replacement_provider_id, effective_date, recurrence, day, start_time, end_time, amount_usd (number), access_handling, pet_handling, status, submission_count (integer), source_locator |
| get_schedule_status | schedule_id, original_provider_id, original_status, replacement_provider_id, replacement_status, recurring (boolean), replacement_day, replacement_start_time, replacement_end_time, replacement_amount_usd (number), access_handling, pet_handling, change_submission_count (integer), source_locator |

Use the response list limits "max_items: 3" for providers and availability,
and "max_items: 2" for schedule events. Use "pagination: {kind: none}".

## Exact synthetic write bodies

POST /v1/schedules accepts only this JSON body:

~~~json
{
  "case_id": "household-service-001",
  "provider_id": "cleaner-001",
  "recurrence": "weekly",
  "day": "saturday",
  "start_time": "10:00",
  "end_time": "12:00",
  "amount_usd": 110,
  "access_handling": "resident_present",
  "pet_handling": "pet_on_site",
  "approval_note": "Synthetic cleaning schedule only; no cleaner contact, home access, or payment."
}
~~~

POST /v1/schedule-changes accepts only this JSON body:

~~~json
{
  "case_id": "household-service-001",
  "schedule_id": "cleaning-schedule-001",
  "replacement_provider_id": "cleaner-003",
  "effective_date": "2026-09-19",
  "recurrence": "weekly",
  "day": "sunday",
  "start_time": "11:00",
  "end_time": "13:00",
  "amount_usd": 118,
  "access_handling": "resident_present",
  "pet_handling": "pet_on_site",
  "approval_note": "Synthetic backup scheduling only; no cleaner contact, home access, or payment."
}
~~~

Both writes are idempotent for an exact repeated body, non-destructive, and
approval-gated. The compiled connector must use "retry: never" for both.

## Synthetic scheduling

- POST /v1/schedules accepts the exact primary scheduling body shown in the
  documentation. It records one recurring synthetic schedule for cleaner-001.
- GET /v1/schedule-events returns one provider cancellation for the next visit
  after the primary schedule exists. It returns no event before scheduling.
- POST /v1/schedule-changes accepts the exact backup body shown in the
  documentation. It records one Sunday replacement with cleaner-003.
- GET /v1/schedule-status returns the cancelled primary and scheduled backup.
  It reports the 118 USD backup price and one change submission.

The access note is a boundary, not a credential. Never send stored access
details or invent an access code. All writes are synthetic and approval-gated.
`;

const cancellationPolicy = {
  policy_id: "household-cancellation-policy-001",
  cancellation_rule: "If the selected provider cancels, replace the next visit with one approved eligible backup.",
  backup_limit: 1,
  preserve_access_boundary: true,
  preserve_pet_boundary: true,
  source_locator: "household://policies/household-cancellation-policy-001",
};

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    let body: unknown;
    if (request.method !== "GET") body = await request.json().catch(() => undefined);
    const entry = { method: request.method, path: `${url.pathname}${url.search}`, ...(body === undefined ? {} : { body }) };
    requests.push(entry);
    console.log(JSON.stringify(entry));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "household-service", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, { headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" } });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/providers") return json({ providers });
    if (request.method === "GET" && url.pathname === "/v1/constraints") return json(constraints);
    if (request.method === "GET" && url.pathname === "/v1/availability") return json({ availability });
    if (request.method === "GET" && url.pathname === "/v1/cancellation-policy") return json(cancellationPolicy);
    if (request.method === "POST" && url.pathname === "/v1/schedules") {
      if (!sameBody(body, scheduleBody)) return json({ error: "invalid primary schedule body" }, 400);
      if (!schedule) {
        schedule = {
          schedule_id: "cleaning-schedule-001",
          case_id: "household-service-001",
          provider_id: "cleaner-001",
          recurrence: "weekly",
          day: "saturday",
          start_time: "10:00",
          end_time: "12:00",
          amount_usd: 110,
          access_handling: "resident_present",
          pet_handling: "pet_on_site",
          status: "scheduled_in_synthetic_record",
          submission_count: 1,
          source_locator: "household://schedules/cleaning-schedule-001",
        };
      }
      return json(schedule, schedule.submission_count === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/schedule-events") {
      if (!schedule) return json({ error: "schedule not recorded" }, 409);
      return json({ events: [{
        event_id: "cancellation-001",
        schedule_id: schedule.schedule_id,
        provider_id: schedule.provider_id,
        event_type: "provider_cancelled",
        effective_date: "2026-09-19",
        message: "Synthetic provider cancellation for the next visit; choose one approved backup.",
        source_locator: "household://schedule-events/cancellation-001",
      }] });
    }
    if (request.method === "POST" && url.pathname === "/v1/schedule-changes") {
      if (!schedule || !sameBody(body, changeBody)) return json({ error: "invalid synthetic backup body" }, 400);
      if (!scheduleChange) {
        scheduleChange = {
          change_id: "schedule-change-001",
          schedule_id: schedule.schedule_id,
          original_provider_id: schedule.provider_id,
          replacement_provider_id: "cleaner-003",
          effective_date: "2026-09-19",
          recurrence: "weekly",
          day: "sunday",
          start_time: "11:00",
          end_time: "13:00",
          amount_usd: 118,
          access_handling: "resident_present",
          pet_handling: "pet_on_site",
          status: "backup_scheduled_in_synthetic_record",
          submission_count: 1,
          source_locator: "household://schedule-changes/schedule-change-001",
        };
      }
      return json(scheduleChange, scheduleChange.submission_count === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/schedule-status") {
      if (!schedule) return json({ error: "schedule not recorded" }, 409);
      return json({
        schedule_id: schedule.schedule_id,
        original_provider_id: schedule.provider_id,
        original_status: scheduleChange ? "cancelled_in_synthetic_record" : schedule.status,
        replacement_provider_id: scheduleChange?.replacement_provider_id ?? "none",
        replacement_status: scheduleChange?.status ?? "not_recorded",
        recurring: true,
        replacement_day: scheduleChange?.day ?? "none",
        replacement_start_time: scheduleChange?.start_time ?? "none",
        replacement_end_time: scheduleChange?.end_time ?? "none",
        replacement_amount_usd: scheduleChange?.amount_usd ?? 0,
        access_handling: scheduleChange?.access_handling ?? schedule.access_handling,
        pet_handling: scheduleChange?.pet_handling ?? schedule.pet_handling,
        change_submission_count: scheduleChange?.submission_count ?? 0,
        source_locator: "household://schedule-status/cleaning-schedule-001",
      });
    }
    if (request.method === "GET" && url.pathname === "/fixture/requests") return json({ fixture: fixtureVersion, requests });
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic household service listening on ${server.url}`);
