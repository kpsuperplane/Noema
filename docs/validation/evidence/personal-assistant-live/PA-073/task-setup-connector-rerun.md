# PA-073 rerun the household-service connector setup

Run a fresh connector setup for the synthetic household-service API. The
previous setup task (`task:c8f4f607a74adbbdd369c8231364a1f5`) opened the docs
and template correctly, but its custom `list_cleaners` response omitted the
required `accepted_content_types` field. The validator rejected that proposal
before any service data route was called. This new task authorizes one fresh
template call and one fresh proposal call.

No real service is involved. Do not contact a cleaner, enter a home, disclose
an access code, charge a card, or move money. Use one personal Noema workspace.

## Source and order

- adapter ID: `synthetic_household_service_api_v1_final`
- definition ID: `definition:synthetic_household_service_api_v1_final`
- display name: `Synthetic household-service API rerun`
- definition revision: `v2`
- origin: `https://attorneys-laden-sheet-residence.trycloudflare.com/`
- source reference: `https://attorneys-laden-sheet-residence.trycloudflare.com/docs`
- authentication: `{kind: "none"}`
- fixture version: `2026-09-10-household-service-api-v1`

This is a fresh attempt after two validator findings. The prior attempts
omitted a custom response content type and then omitted argument descriptions.
Every argument object must include a concise non-empty `description` as well
as its name, type, and required flag. This applies to every argument on both
POST operations, even though the argument list is otherwise exactly as shown.

Open the exact documentation URL first. Then call
`adapter.definition_template` once with no arguments. Then call
`adapter.propose_definition` once for the new definition. Do not retry either
call. Keep the successful proposal pending for operator review.

Do not call any `/v1` route during setup. Do not switch web providers. Do not
include retry, fixed headers, or fixed queries. The server derives
`transport_safe_read` for GETs and `never` for writes. Omit
`accepted_content_types` on generated `flat_object` and `object_list`
responses. For the custom `list_cleaners` response, include exactly
`accepted_content_types: ["application/json"]`, because the template requires
it. Use `pagination: {kind: "none"}` everywhere.

## Exact operations

Propose exactly these nine operations and no others. All GETs have no
arguments, are read-only, idempotent, non-destructive, and closed-world. Both
POSTs are non-read-only, exact-body-idempotent, non-destructive, open-world,
and approval-gated.

| Operation | Request and required response |
| --- | --- |
| `get_household_profile` | GET `/v1/profile`; `flat_object` fields `case_id`, `owner`, `current_date`, `home_label`, `pet_name`, `pet_type`, `goal`, `source_locator`. |
| `list_cleaners` | GET `/v1/providers`; bounded object list from `/providers` named `providers`, max 3. Fields `provider_id`, `provider_name`, `pet_policy`, `access_modes` (array), `recurring_supported`, `insured`, `visit_price_usd`, `available_day`, `available_start_time`, `available_end_time`, `cancellation_policy`, `source_locator`. Use a closed custom response to preserve the array, with `accepted_content_types: ["application/json"]`. |
| `get_household_constraints` | GET `/v1/constraints`; `flat_object` fields `constraint_id`, `recurrence`, `preferred_day`, `preferred_start_time`, `preferred_end_time`, `visit_ceiling_usd`, `pet_requirement`, `access_requirement`, `access_note`, `source_locator`. |
| `list_cleaning_availability` | GET `/v1/availability`; object list from `/availability` named `availability`, max 3. Fields `slot_id`, `provider_id`, `day`, `start_time`, `end_time`, `visit_price_usd`, `pet_policy`, `access_mode`, `slot_status`, `source_locator`. |
| `get_cancellation_policy` | GET `/v1/cancellation-policy`; `flat_object` fields `policy_id`, `cancellation_rule`, `backup_limit`, `preserve_access_boundary`, `preserve_pet_boundary`, `source_locator`. |
| `create_recurring_schedule` | POST `/v1/schedules`; JSON body arguments only `case_id`, `provider_id`, `recurrence`, `day`, `start_time`, `end_time`, `amount_usd`, `access_handling`, `pet_handling`, `approval_note`, each bound to its same-named `$argument`; `flat_object` fields `schedule_id`, `case_id`, `provider_id`, `recurrence`, `day`, `start_time`, `end_time`, `amount_usd`, `access_handling`, `pet_handling`, `status`, `submission_count`, `source_locator`. |
| `list_schedule_events` | GET `/v1/schedule-events`; object list from `/events` named `events`, max 2. Fields `event_id`, `schedule_id`, `provider_id`, `event_type`, `effective_date`, `message`, `source_locator`. |
| `change_schedule_to_backup` | POST `/v1/schedule-changes`; JSON body arguments only `case_id`, `schedule_id`, `replacement_provider_id`, `effective_date`, `recurrence`, `day`, `start_time`, `end_time`, `amount_usd`, `access_handling`, `pet_handling`, `approval_note`, each bound to its same-named `$argument`; `flat_object` fields `change_id`, `schedule_id`, `original_provider_id`, `replacement_provider_id`, `effective_date`, `recurrence`, `day`, `start_time`, `end_time`, `amount_usd`, `access_handling`, `pet_handling`, `status`, `submission_count`, `source_locator`. |
| `get_schedule_status` | GET `/v1/schedule-status`; `flat_object` fields `schedule_id`, `original_provider_id`, `original_status`, `replacement_provider_id`, `replacement_status`, `recurring`, `replacement_day`, `replacement_start_time`, `replacement_end_time`, `replacement_amount_usd`, `access_handling`, `pet_handling`, `change_submission_count`, `source_locator`. |

Use the documented response types. Every string needs a `max_bytes` bound.
Use 96 bytes for source and record fields, 128 bytes for schedule fields, and
256 bytes for descriptive text. Use native number, integer, and boolean
types. Bound `access_modes` to four strings of at most 64 bytes. Use
`max_items: 3` for providers and availability, and `max_items: 2` for events.
Mark every listed response field as required. Keep all output schemas closed.

## Exact synthetic write bodies for later execution

Do not send either body during setup. Preserve these bodies for a later task
only after separate normal human approvals.

Primary schedule:

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

Backup change:

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

Never include the access note or an access code in either request.

## Acceptance

- The docs page is opened before the one template call.
- The template is called once with no arguments.
- One proposal is called once and returns `review_required`.
- The proposal has exactly the nine operation IDs and paths above.
- The custom cleaner response includes `accepted_content_types` and preserves
  the bounded `access_modes` array. Generated responses omit that field.
- All seven GETs compile as automatic reads. Both POSTs compile with
  `retry: never` and remain approval-gated.
- Setup makes no `/v1` request, no state-changing request, and no provider
  switch.
- Record the pending semantic digest for operator acceptance. Do not accept
  or invoke the connector in this setup task.
