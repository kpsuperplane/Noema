# PA-063 connect the synthetic care-change scope API

The operator inspected the current documentation at
`https://live-atlanta-favor-midlands.trycloudflare.com/docs`. Use that exact
contract as the documentation record. Create one reviewed API connection. Do
not call any service endpoint during setup.

Set `new_definition.origin` exactly to
`https://live-atlanta-favor-midlands.trycloudflare.com/`, including the
trailing slash. Set `source_reference` to
`https://live-atlanta-favor-midlands.trycloudflare.com/docs`. Use definition
revision `v1` and authentication `{kind:"none"}`.

Call `adapter.definition_template` first with an empty object. Then call
`adapter.propose_definition` exactly once with a new definition. Do not
include a `retry`, `retry_policy`, or custom response transform. Noema computes
retry behavior from the method and safety flags. Use the generated response
recipe format: `response.kind` is `flat_object`; every response field has a
valid `source_pointer`, and every string field has `max_bytes`. Use
`pagination:{kind:"none"}` on every operation. Every argument must have a
non-empty description.

The proposal must contain exactly these ten operations and no others:

1. `get_care_change_profile`: GET `/v1/profile`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. The flat response requires
   `case_id` (string, 64 bytes), `person_label` (string, 96), `current_date`
   (string, 10), `observation_window` (string, 64), `goal` (string, 256),
   `workspace_scope` (string, 96), `decision_boundary` (string, 256), and
   `source_locator` (string, 128).
2. `get_first_daily_observation`: GET `/v1/observations/first`; no
   arguments; read-only, idempotent, non-destructive, and closed-world. The
   flat response requires `observation_id` (string, 64), `observed_on`
   (string, 10), `observed_at` (string, 8), `metric` (string, 64),
   `value_text` (string, 32), `unit` (string, 16), `context` (string, 64),
   `note` (string, 128), and `source_locator` (string, 128).
3. `get_latest_daily_observation`: GET `/v1/observations/latest`; no
   arguments; use the same flat response fields and limits as operation 2.
4. `get_escalation_rule`: GET `/v1/escalation-rule`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. The flat
   response requires `rule_id` (string, 64), `rule_name` (string, 96),
   `trigger` (string, 256), `action` (string, 256),
   `interpretation_boundary` (string, 256), and `source_locator` (string,
   128).
5. `get_family_recipient`: GET `/v1/recipients/family`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. The flat
   response requires `recipient_id` (string, 64), `recipient_label` (string,
   96), `role` (string, 64), `authorization_status` (string, 32),
   `disclosure_scope` (string, 96), `channel` (string, 64), and
   `source_locator` (string, 128).
6. `get_clinical_recipient`: GET `/v1/recipients/clinical`; no arguments;
   use the same flat response fields and limits as operation 5.
7. `get_existing_care_updates`: GET `/v1/updates`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. The flat
   response requires `existing_update_count` (integer),
   `existing_update_summary` (string, 256), and `source_locator` (string,
   128).
8. `send_family_care_update`: POST `/v1/family-update`; required JSON-body
   arguments `recipient_id` (string), `disclosure_scope` (string), and
   `message` (string), each with a description. Use a JSON body template with
   `$recipient_id`, `$disclosure_scope`, and `$message`. Set read-only false,
   idempotent false, destructive false, and open-world true. The flat
   response requires `update_id` (string, 64), `receipt_id` (string, 64),
   `recipient_id` (string, 64), `disclosure_scope` (string, 96),
   `delivery_status` (string, 64), `submission_count` (integer), and
   `source_locator` (string, 128). This operation is unsafe and must remain
   approval-gated.
9. `send_clinical_care_escalation`: POST `/v1/clinical-escalation`; required
   JSON-body arguments `recipient_id` (string), `disclosure_scope` (string),
   `rule_id` (string), `observation_ids_text` (string), and `message`
   (string), each with a description. Use a JSON body template with the
   matching `$argument` reference for every argument. Set read-only false,
   idempotent false, destructive false, and open-world true. The flat
   response requires `escalation_id` (string, 64), `receipt_id` (string, 64),
   `recipient_id` (string, 64), `disclosure_scope` (string, 96), `rule_id`
   (string, 64), `observation_ids_text` (string, 128), `threshold_result`
   (string, 32), `delivery_status` (string, 64), `submission_count`
   (integer), and `source_locator` (string, 128). This operation is unsafe
   and must remain approval-gated.
10. `get_care_change_status`: GET `/v1/status`; no arguments; read-only,
    idempotent, non-destructive, and closed-world. The flat response requires
    `profile_case_id` (string, 64), `latest_observation_id` (string, 64),
    `threshold_result` (string, 32), `family_update_id` (string, 64),
    `family_receipt_id` (string, 64), `family_recipient_id` (string, 64),
    `family_disclosure_scope` (string, 96), `family_delivery_status`
    (string, 64), `family_submission_count` (integer),
    `clinical_escalation_id` (string, 64), `clinical_receipt_id` (string, 64),
    `clinical_recipient_id` (string, 64), `clinical_disclosure_scope`
    (string, 96), `clinical_delivery_status` (string, 64),
    `clinical_submission_count` (integer), and `source_locator` (string,
    128).

Preserve every supplied profile fact, observation value and timestamp, rule
text, recipient identity and disclosure scope, update count, receipt, status,
and `care-change://` locator. Do not diagnose, prescribe, change treatment,
disclose health information to the family recipient, contact a real person,
create a shared workspace, accept payment, or invent a record.

## Success conditions

- The proposal returns `review_required` and contains exactly these ten
  operations with generated flat response recipes and JSON body templates for
  both POST operations.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
