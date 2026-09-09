# PA-062 connect the synthetic caregiver-schedule API

The operator inspected the current documentation at
`https://ca156b7dfbf630.lhr.life/docs` before this task. Use the exact contract
below as the documentation record. Do not call `web.browse` or any other web
tool. Create one reviewed API connection. Do not invoke any service endpoint
during setup.

Set `new_definition.origin` exactly to
`https://ca156b7dfbf630.lhr.life/`, including the trailing slash. Keep
`source_reference` at `https://ca156b7dfbf630.lhr.life/docs`.
Use definition revision `v2`; the earlier `v1` origin is no longer live.

Call `adapter.definition_template` first. Then call
`adapter.propose_definition` once with a new definition. Do not include a
`retry` or `retry_policy` property in an operation proposal. Noema computes
retry behavior from the method and safety flags. Use the generated response
recipe format exactly: `response.kind` is `flat_object` or `object_list`;
object-list responses have `source_pointer`, `output_name`, `max_items`, and
`fields`; every string field has a `max_bytes` value. Use
`pagination:{kind:"none"}` on every operation. Every argument must include a
non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these ten operations and no others:

1. `get_caregiver_schedule_profile`: GET `/v1/profile`; no arguments; flat
   required fields `case_id` (string 64), `person_label` (string 96),
   `current_date` (string 10), `schedule_window` (string 64), `goal` (string
   256), `workspace_scope` (string 96), `decision_boundary` (string 256),
   and `source_locator` (string 128).
2. `list_authorized_caregivers`: GET `/v1/caregivers`; no arguments;
   object-list from `/caregivers` named `caregivers`, maximum 3, with required
   fields `caregiver_id` (string 64), `caregiver_label` (string 96),
   `authorization_status` (string 32), `consent_scope` (string 96),
   `skills` (string 128), `availability` (string 256), and `source_locator`
   (string 128).
3. `get_coverage_requirements`: GET `/v1/coverage-requirements`; no
   arguments; object-list from `/required_shifts` named `required_shifts`,
   maximum 5, with required fields `shift_id` (string 64), `date` (string 10),
   `start` (string 8), `end` (string 8), `required_skill` (string 64),
   `purpose` (string 128).
4. `get_schedule_preferences`: GET `/v1/preferences`; no arguments; flat
   required fields `authorized_caregivers_only` (boolean),
   `maximum_shift_hours` (integer),
   `minimum_respite_hours_between_same_caregiver_shifts` (integer),
   `maximum_hours_per_caregiver_per_day` (integer), `no_overnight_shifts`
   (boolean), `notice_scope` (string 96), and `source_locator` (string 128).
5. `get_existing_caregiver_schedule`: GET `/v1/schedule`; no arguments; flat
   required fields `schedule_id` (string 64), `status` (string 32),
   `shift_assignments_text` (string 2048), and `source_locator` (string 128).
6. `submit_caregiver_schedule`: POST `/v1/schedule`; required JSON-body
   argument `shift_assignments` (string_array). Include it in
   `json_body_template` using `$shift_assignments`. Each item must preserve
   the documented shift_id, caregiver_id, required_skill, date, start, and
   end values. Set read-only false, idempotent false, destructive false,
   open-world true. Its flat response must require `schedule_id` (string 64),
   `receipt_id` (string 64), `status` (string 32), `submission_count`
   (integer), `shift_assignments_text` (string 2048), and `source_locator`
   (string 128). This is an unsafe operation and must remain approval-gated.
7. `get_caregiver_cancellation`: GET `/v1/cancellation`; no arguments; flat
   required fields `cancellation_id` (string 64), `shift_id` (string 64),
   `caregiver_id` (string 64), `caregiver_label` (string 96), `canceled_on`
   (string 10), `effective_date` (string 10), `reason` (string 128),
   `notice_scope` (string 96), and `source_locator` (string 128).
8. `reassign_canceled_shift`: POST `/v1/reassign`; required JSON-body
   arguments `cancellation_id` (string), `shift_id` (string), and
   `replacement_caregiver_id` (string). Include all arguments in
   `json_body_template` using `$argument` references. Set read-only false,
   idempotent false, destructive false, open-world true. Its flat response
   must require `reassignment_id` (string 64), `receipt_id` (string 64),
   `cancellation_id` (string 64), `replacement_caregiver_id` (string 64),
   `coverage_status` (string 32), `submission_count` (integer), and
   `source_locator` (string 128). This is unsafe and must remain
   approval-gated.
9. `send_authorized_schedule_notices`: POST `/v1/notices`; required JSON-body
   arguments `recipient_ids` (string_array), `notice_scope` (string), and
   `message` (string). Include all arguments in `json_body_template` using
   `$argument` references. Set read-only false, idempotent false, destructive
   false, open-world true. Its flat response must require `notice_batch_id`
   (string 64), `receipt_id` (string 64), `recipient_ids_text` (string 256),
   `notice_scope` (string 96), `delivery_status` (string 64),
   `submission_count` (integer), and `source_locator` (string 128). This is
   unsafe and must remain approval-gated.
10. `get_caregiver_schedule_status`: GET `/v1/status`; no arguments; flat
    required fields `schedule_id` (string 64), `schedule_status` (string 32),
    `schedule_receipt_id` (string 64), `schedule_submission_count` (integer),
    `shift_assignments_text` (string 2048), `cancellation_id` (string 64),
    `canceled_shift_id` (string 64), `canceled_caregiver_id` (string 64),
    `cancellation_effective_date` (string 10), `cancellation_reason` (string
    128), `reassignment_id` (string 64), `reassignment_receipt_id` (string
    64), `replacement_caregiver_id` (string 64), `coverage_status` (string
    32), `reassignment_submission_count` (integer), `notice_batch_id` (string
    64), `notice_receipt_id` (string 64), `notice_recipient_ids_text` (string 256), `notice_scope` (string 96), `delivery_status` (string 64),
    `notice_submission_count` (integer), and `source_locator` (string 128).

Set read-only true, idempotent true, destructive false, and open-world false
for operations 1–5, 7, and 10. Preserve every caregiver identity,
authorization and consent scope, skill, availability window, required shift,
respite limit, cancellation, replacement, notice recipient, receipt, status,
and `care://` locator. Do not create a shared workspace, send a real message,
disclose health information, provide care, accept payment, or invent a
caregiver or availability record.

## Success conditions

- The proposal is accepted for review and contains exactly the ten operations
  above with valid generated response recipes and JSON body templates for the
  three POST operations.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
