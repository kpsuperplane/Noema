# PA-073 set up the synthetic household-service API

Use the live Go Noema development instance to inspect the public synthetic
documentation and propose one API connector. This service is synthetic. Do
not contact a cleaner, enter a home, disclose an access code, charge a card,
or move money.

## Documentation source

- adapter ID: `synthetic_household_service_api_v1`
- definition ID: `definition:synthetic_household_service_api_v1`
- display name: `Synthetic household-service API`
- definition revision: `v1`
- origin: `https://attorneys-laden-sheet-residence.trycloudflare.com/`
- source reference: `https://attorneys-laden-sheet-residence.trycloudflare.com/docs`
- authentication: none
- scope: one personal Noema workspace
- fixture version: `2026-09-10-household-service-api-v1`

Open the documentation source first. Use its returned page as the source for
the proposal. Then call `adapter.definition_template` exactly once with no
arguments. Use that response to make exactly one
`adapter.propose_definition` call. Do not retry either setup call. Keep the
proposal pending for operator review.

Do not call any `/v1` route during setup. Do not call `web.browse.switch_provider`.
Do not include `retry`, fixed headers, or fixed queries. The server derives
`transport_safe_read` for read-only GET operations and `never` for both writes.
Do not include `accepted_content_types` for generated `flat_object` or
`object_list` responses. A `custom` response must include the template's
required `accepted_content_types: ["application/json"]`. Use
`pagination: {kind: "none"}` for every operation.

## Required operations

Propose exactly these nine operations and no others. Declare the four
behavior booleans for each operation. All listed response fields are required.

1. `get_household_profile`: GET `/v1/profile`; read-only, idempotent,
   non-destructive, closed-world. Use a `flat_object` response. Preserve
   `case_id`, `owner`, `current_date`, `home_label`, `pet_name`, `pet_type`,
   `goal`, and `source_locator`.
2. `list_cleaners`: GET `/v1/providers`; read-only, idempotent,
   non-destructive, closed-world. Use an object list from `/providers` named
   `providers`, with at most three records. Preserve `provider_id`,
   `provider_name`, `pet_policy`, `access_modes`, `recurring_supported`,
   `insured`, `visit_price_usd`, `available_day`, `available_start_time`,
   `available_end_time`, `cancellation_policy`, and `source_locator`.
   `access_modes` is a bounded string array. Use a custom response transform
   only when needed to preserve that documented array; keep the output schema
   closed and bound the array to four strings of at most 64 bytes.
3. `get_household_constraints`: GET `/v1/constraints`; read-only, idempotent,
   non-destructive, closed-world. Use a `flat_object` response. Preserve
   `constraint_id`, `recurrence`, `preferred_day`, `preferred_start_time`,
   `preferred_end_time`, `visit_ceiling_usd`, `pet_requirement`,
   `access_requirement`, `access_note`, and `source_locator`.
4. `list_cleaning_availability`: GET `/v1/availability`; read-only,
   idempotent, non-destructive, closed-world. Use an object list from
   `/availability` named `availability`, with at most three records. Preserve
   `slot_id`, `provider_id`, `day`, `start_time`, `end_time`,
   `visit_price_usd`, `pet_policy`, `access_mode`, `slot_status`, and
   `source_locator`.
5. `get_cancellation_policy`: GET `/v1/cancellation-policy`; read-only,
   idempotent, non-destructive, closed-world. Use a `flat_object` response.
   Preserve `policy_id`, `cancellation_rule`, `backup_limit`,
   `preserve_access_boundary`, `preserve_pet_boundary`, and `source_locator`.
6. `create_recurring_schedule`: POST `/v1/schedules`; non-read-only,
   idempotent for an exact repeated body, non-destructive, open-world, and
   approval-gated. Accept only these JSON-body arguments: `case_id`,
   `provider_id`, `recurrence`, `day`, `start_time`, `end_time`, `amount_usd`,
   `access_handling`, `pet_handling`, and `approval_note`. Bind each body
   property to its same-named `$argument`. Use a `flat_object` response and
   preserve `schedule_id`, `case_id`, `provider_id`, `recurrence`, `day`,
   `start_time`, `end_time`, `amount_usd`, `access_handling`, `pet_handling`,
   `status`, `submission_count`, and `source_locator`.
7. `list_schedule_events`: GET `/v1/schedule-events`; read-only, idempotent,
   non-destructive, closed-world. Use an object list from `/events` named
   `events`, with at most two records. Preserve `event_id`, `schedule_id`,
   `provider_id`, `event_type`, `effective_date`, `message`, and
   `source_locator`.
8. `change_schedule_to_backup`: POST `/v1/schedule-changes`; non-read-only,
   idempotent for an exact repeated body, non-destructive, open-world, and
   approval-gated. Accept only these JSON-body arguments: `case_id`,
   `schedule_id`, `replacement_provider_id`, `effective_date`, `recurrence`,
   `day`, `start_time`, `end_time`, `amount_usd`, `access_handling`,
   `pet_handling`, and `approval_note`. Bind each body property to its
   same-named `$argument`. Use a `flat_object` response and preserve
   `change_id`, `schedule_id`, `original_provider_id`,
   `replacement_provider_id`, `effective_date`, `recurrence`, `day`,
   `start_time`, `end_time`, `amount_usd`, `access_handling`, `pet_handling`,
   `status`, `submission_count`, and `source_locator`.
9. `get_schedule_status`: GET `/v1/schedule-status`; read-only, idempotent,
   non-destructive, closed-world. Use a `flat_object` response. Preserve
   `schedule_id`, `original_provider_id`, `original_status`,
   `replacement_provider_id`, `replacement_status`, `recurring`,
   `replacement_day`, `replacement_start_time`, `replacement_end_time`,
   `replacement_amount_usd`, `access_handling`, `pet_handling`,
   `change_submission_count`, and `source_locator`.

Use no operation arguments on the GET operations. Use no undocumented route,
operation, credential, or access detail.

## Response bounds

Use the response recipe bounds below. Every string field must specify the
listed `max_bytes`; every object list must specify `max_items`; every number,
integer, and boolean must keep its native JSON type.

| Operation | Response recipe and bounds |
| --- | --- |
| `get_household_profile` | `flat_object`; `case_id`, `owner`, `current_date`, `home_label`, `pet_name`, `pet_type`, and `source_locator` max 96; `goal` max 256. |
| `list_cleaners` | `object_list` from `/providers`, output `providers`, `max_items: 3`; IDs, provider name, pet policy, day, times, and locator max 96; `access_modes` is a string array with max four items and max 64 bytes per item; `cancellation_policy` max 256; `recurring_supported` and `insured` booleans; `visit_price_usd` number. |
| `get_household_constraints` | `flat_object`; IDs, recurrence, day, times, and locator max 96; `pet_requirement` and `access_requirement` max 256; `access_note` max 256; `visit_ceiling_usd` number. |
| `list_cleaning_availability` | `object_list` from `/availability`, output `availability`, `max_items: 3`; IDs, day, times, pet policy, access mode, status, and locator max 96; `visit_price_usd` number. |
| `get_cancellation_policy` | `flat_object`; `policy_id` and `source_locator` max 96; `cancellation_rule` max 256; `backup_limit` integer; both boundary fields booleans. |
| `create_recurring_schedule` | `flat_object`; IDs, case, provider, recurrence, day, times, access and pet handling, status, and locator max 128; `amount_usd` number; `submission_count` integer. |
| `list_schedule_events` | `object_list` from `/events`, output `events`, `max_items: 2`; IDs and dates max 96; `event_type` max 96; `message` max 256; `source_locator` max 96. |
| `change_schedule_to_backup` | `flat_object`; IDs, case, providers, dates, recurrence, day, times, access and pet handling, status, and locator max 128; `amount_usd` number; `submission_count` integer. |
| `get_schedule_status` | `flat_object`; IDs, statuses, day, times, access and pet handling, and locator max 128; `recurring` boolean; `replacement_amount_usd` number; `change_submission_count` integer. |

## Synthetic write bodies

The primary schedule must use this exact body after human approval:

```json
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
```

The backup change must use this exact body after a separate human approval:

```json
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
```

Never send the access note or any access code to the provider. Both writes are
synthetic and must remain behind normal Noema approval.

## Setup acceptance

- The documentation page is opened before the template call.
- Exactly one no-argument `adapter.definition_template` call is made.
- Exactly one `adapter.propose_definition` call returns `review_required`.
- The proposal has exactly the nine operation IDs and paths above.
- The seven GET operations are automatic reads. Both POST operations are
  approval-gated writes with compiled `retry: never`.
- The proposal preserves the documented boolean values and cleaner access
  modes without widening the output schema.
- Setup makes no `/v1` request and no state-changing request.
- The operator can accept the inspected pending digest and configure all
  reads automatically with both writes set to always ask.
