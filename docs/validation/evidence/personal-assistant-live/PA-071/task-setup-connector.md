# PA-071 set up the synthetic pet-care API

Use the live Go Noema development instance to inspect the exact public
documentation below and propose one API connector. This setup is synthetic.
Do not call a service route, contact a clinic, request a prescription, book a
boarding place, or perform any real action.

## Documentation source

- Adapter ID: `synthetic_pet_care_api_v1`
- Definition ID: `definition:synthetic_pet_care_api_v1`
- Display name: `Synthetic pet-care API`
- Origin: `https://iso-scan-authorization-ruling.trycloudflare.com/`
- Source reference: `https://iso-scan-authorization-ruling.trycloudflare.com/docs`
- Revision: `v1`
- Authentication: none
- Scope: one personal Noema workspace
- Fixture version: `2026-09-09-pet-care-api-v1`

Read the documentation first. Use the returned page evidence directly. Do not
call `web.browse.switch_provider`. Then call `adapter.definition_template`
once with no arguments. Do not call it again. Use that one template response
to build exactly one `adapter.propose_definition` request. Do not retry the
proposal. Record the returned pending digest for operator review. The
proposal must be reviewed before any connector use.

Do not include a `retry` field in any operation proposal. The server derives
`transport_safe_read` for the read-only GET operations and `never` for the
non-read-only POST. Do not include `accepted_content_types` on generated
`flat_object` or `object_list` responses; the server supplies
`application/json`. Include it only if a custom response is required.

## Required operations

The proposal must contain exactly these nine operations and no others:

1. `get_pet_profile`: GET `/v1/profile`; read-only, idempotent,
   non-destructive, closed-world. Preserve the case, owner, current date,
   goal, boundary, travel dates, boarding pet ID, and source locator.
2. `list_pets`: GET `/v1/pets`; read-only, idempotent, non-destructive,
   closed-world. Project `/pets` as an object list with at most two records.
   Preserve pet ID, display name, species, sex, breed, birth date, synthetic
   microchip ID, and source locator.
3. `get_vet_plans`: GET `/v1/vet-plan`; read-only, idempotent,
   non-destructive, closed-world. Project `/plans` as an object list with at
   most two records. Preserve each pet ID and name, species, exam due date,
   vaccination status, rabies date, bordetella date, care instructions, and
   source locator.
4. `list_pet_refills`: GET `/v1/refills`; read-only, idempotent,
   non-destructive, closed-world. Project `/refills` as an object list with at
   most two records. Preserve refill ID, pet ID and name, medication ID and
   name, exact dose instruction, quantity, due date, status, and locator.
5. `get_boarding_requirements`: GET `/v1/boarding-requirements`; read-only,
   idempotent, non-destructive, closed-world. Preserve boarding ID, pet ID,
   facility label, dates, intake deadline, required vaccine string, note, and
   locator.
6. `get_pet_travel_plan`: GET `/v1/travel`; read-only, idempotent,
   non-destructive, closed-world. Project `/travelers` as an object list with
   at most two records. Preserve each pet ID and name, arrangement, travel
   dates, and locator.
7. `list_pet_visit_options`: GET `/v1/visit-options`; read-only, idempotent,
   non-destructive, closed-world. Project `/options` as an object list with at
   most four records. Preserve option ID, pet ID and name, service type, date,
   start time, included services, status, required-by date, estimated total,
   and locator.
8. `schedule_pet_care`: POST `/v1/care-actions`; non-read-only,
   idempotent for an exact repeated request, non-destructive, open-world,
   approval-gated. It accepts only these JSON-body arguments: `case_id` string,
   `visit_option_ids` string array, `refill_pet_id` string,
   `medication_id` string, `dose_instruction` string, `refill_quantity`
   integer, `boarding_start` string, `boarding_end` string, and
   `approval_note` string. Bind every body property to its identically named
   `$argument` value. Do not add a retry field. The compiled write must still
   use `retry: never`; Noema must not replay it automatically.
9. `get_pet_care_confirmations`: GET `/v1/confirmations`; read-only,
   idempotent, non-destructive, closed-world. Preserve the action ID,
   submission count, both visit confirmation IDs, both pet IDs and names,
   both service types and dates, both statuses, refill confirmation ID and
   pet identity, medication ID, exact dose instruction, quantity, refill
   status, and locator.

The exact confirmation field names for operations 8 and 9 are:
`action_id`, `submission_count`, `first_confirmation_id`, `first_pet_id`,
`first_pet_name`, `first_service_type`, `first_date`, `first_status`,
`second_confirmation_id`, `second_pet_id`, `second_pet_name`,
`second_service_type`, `second_date`, `second_status`,
`refill_confirmation_id`, `refill_pet_id`, `refill_pet_name`,
`refill_medication_id`, `refill_dose_instruction`, `refill_quantity`,
`refill_status`, and `source_locator`.

Use `pagination: {kind: "none"}` for every operation. Use bounded generated
responses. The generated response fields below use `type` values `string`,
`integer`, `number`, or `boolean`; every string field needs the listed
`max_bytes` value and every object list needs the listed `max_items` value.
Generated responses must omit `accepted_content_types`.

| Operation | Response recipe and bounds |
| --- | --- |
| `get_pet_profile` | `flat_object`; root fields `case_id`, `owner_label`, `current_date`, `travel_start`, `travel_end`, `boarding_pet_id` max 64; `goal`, `decision_boundary` max 256; `source_locator` max 96. |
| `list_pets` | `object_list` from `/pets`, output `pets`, `max_items: 2`; `pet_id`, `name`, `species`, `sex`, `breed`, `birth_date`, `microchip_id`, `source_locator` max 96. |
| `get_vet_plans` | `object_list` from `/plans`, output `plans`, `max_items: 2`; IDs and dates max 64; `pet_name`, `species` max 64; `vaccination_status`, `care_instructions` max 256; `rabies_valid_through`, `bordetella_due_on`, `source_locator` max 96. |
| `list_pet_refills` | `object_list` from `/refills`, output `refills`, `max_items: 2`; IDs and dates max 64; names max 96; `dose_instruction` max 160; `quantity` integer; `refill_status`, `source_locator` max 96. |
| `get_boarding_requirements` | `flat_object`; IDs and dates max 64; `facility_label` max 96; `required_vaccinations` max 96; `requirement_note` max 256; `source_locator` max 96. |
| `get_pet_travel_plan` | `object_list` from `/travelers`, output `travelers`, `max_items: 2`; IDs, names, arrangement, dates, and locators max 96. |
| `list_pet_visit_options` | `object_list` from `/options`, output `options`, `max_items: 4`; IDs, names, service type, dates, times, status, and required-by max 64; `included_services` max 96; `source_locator` max 96; `estimated_total_usd` number. These smaller bounds keep the computed response below the 32 KiB limit. |
| `schedule_pet_care` | `flat_object`; preserve all returned confirmation fields with the same bounds used by `get_pet_care_confirmations`, plus `action_id` max 64 and `submission_count` integer. |
| `get_pet_care_confirmations` | `flat_object`; `action_id` max 64; `submission_count` integer; all confirmation IDs, pet IDs/names, services, dates, statuses, medication ID, dose, quantity, and locator use max 160. |

For each generated response, mark all listed fields as required. Keep object
shapes bounded and do not propose an undocumented route or a real pet-care
action.

## Exact synthetic write body

The valid fixture request is:

```json
{
  "case_id": "pet-care-001",
  "visit_option_ids": ["option-001", "option-003"],
  "refill_pet_id": "pet-001",
  "medication_id": "med-001",
  "dose_instruction": "Give 25 mg by mouth twice daily with food; do not change dose.",
  "refill_quantity": 20,
  "boarding_start": "2026-09-20",
  "boarding_end": "2026-09-24",
  "approval_note": "Synthetic pet-care scheduling only; no clinic contact or medication change."
}
```

## Acceptance criteria

- The documentation page is opened before the proposal.
- One no-argument `adapter.definition_template` call is made.
- Exactly one `adapter.propose_definition` call returns `review_required`.
- The proposal contains exactly the nine named operations and paths above.
- All seven GET source operations are automatic reads. Only
  `schedule_pet_care` is a write and it is approval-gated.
- Setup makes no `/v1` request and no state-changing request.
- The operator can accept the inspected pending digest and configure the
  resulting connection for automatic reads and always-ask writes.

## Accepted setup receipt

The reviewed pending digest was
`ea0f94cf5f0b7ef8ed2c3d1b6c65d5d9357567917eb841d0eb1e11b3f92d7bd1`.
The operator accepted it, producing canonical digest
`23681a58fb0ecba53e6dc60f6097809c07cb63c02f9c3e8464bb9709dad9dc0b`.
The active connection is `be6439434be1224531bd5150201025a5` with revision 2
and policy revision 2. Its policy allows all nine reads automatically and
requires approval for the one write.
