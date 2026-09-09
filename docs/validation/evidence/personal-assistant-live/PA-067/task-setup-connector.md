# PA-067 connect the synthetic home-repair API

Inspect the current documentation at
`https://analyses-vocal-declined-zoloft.trycloudflare.com/docs`. Create one
reviewed API connection. Do not invoke a `/v1` endpoint during setup.

Set `new_definition.origin` exactly to
`https://analyses-vocal-declined-zoloft.trycloudflare.com/`, including the
trailing slash. Set `source_reference` to the documentation URL. Use definition
revision `v1` and authentication `{kind:"none"}`.

Use adapter ID `synthetic_home_repair_api_v2` and definition ID
`definition:synthetic_home_repair_api_v2`. Call `adapter.definition_template`
first with an empty object. Then call `adapter.propose_definition` exactly once
with that new definition. After that proposal succeeds, do not call
`adapter.propose_definition` again, even if a reviewer requests more evidence.
Add any later evidence to `RESULT.md` and `TASK.md` with `code.run_luau` or
task file tools. Do not include a custom retry or response transform. Use
generated response recipes:
`response.kind` is `flat_object` or `object_list`; every response field has a
valid `source_pointer`; every string field has `max_bytes`; and every
object-list response has `source_pointer`, `output_name`, `max_items`, and
`fields`. Use `pagination:{kind:"none"}` on every operation. Every argument
must have a non-empty description.

The proposal must contain exactly these thirteen operations and no others.

1. `get_repair_profile`: GET `/v1/profile`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. Required fields: `case_id`
   string (64), `owner_label` string (96), `current_date` string (10),
   `time_zone` string (64), `property_label` string (128), `project_label`
   string (128), `goal` string (384), `decision_boundary` string (256),
   `budget_limit_usd` number, `required_completion_date` string (10), and
   `source_locator` string (128).
2. `list_repair_bids`: GET `/v1/bids`; no arguments; read-only, idempotent,
   non-destructive, and closed-world. Return an object-list from `/bids`
   named `bids`, maximum 3. Required item fields: `bid_id` string (64),
   `vendor_label` string (128), `scope_id` string (64), `base_amount_usd`
   number, `disposal_included` string (8), `disposal_amount_usd` number,
   `permit_included` string (8), `permit_amount_usd` number,
   `normalized_total_usd` number, `insurance_valid_through` string (10),
   `proposed_start_date` string (10), `proposed_start_time` string (5),
   `estimated_duration_days` integer, `estimated_end_date` string (10),
   `scope_summary` string (256), `bid_status` string (32), and
   `source_locator` string (128).
3. `get_repair_insurance`: GET `/v1/insurance`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. Required fields: `policy_id`
   string (64), `provider_label` string (128), `status` string (32),
   `expires_on` string (10), `required_through` string (128), `extension_id`
   string (64), `extension_effective_on` string (10), `extension_expires_on`
   string (10), `extension_premium_usd` number, `extension_status` string (32),
   `coverage_summary` string (256), and `source_locator` string (128).
4. `get_repair_scope`: GET `/v1/scope`; no arguments; read-only, idempotent,
   non-destructive, and closed-world. Required fields: `scope_id` string (64),
   `scope_title` string (128), `included_work` string (384), `excluded_work`
   string (256), `completion_standard` string (384), and `source_locator`
   string (128).
5. `get_repair_constraints`: GET `/v1/constraints`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. Required fields:
   `access_window` string (128), `quiet_hours` string (96), `no_work_dates`
   string (64), `no_work_reason` string (128), `required_completion_date`
   string (10), `budget_limit_usd` number, `budget_scope` string (256), and
   `source_locator` string (128).
6. `get_existing_repair_plan`: GET `/v1/existing-plan`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. Required fields:
   `plan_id` string (64), `plan_status` string (64), `accepted_bid_id` string
   (64), `scheduled_start` string (32), `scheduled_end` string (32), and
   `source_locator` string (128).
7. `get_repair_change_request`: GET `/v1/change-request`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. Required fields:
   `change_request_id` string (64), `project_id` string (64), `status` string
   (64), `reason` string (256), `added_scope` string (384),
   `added_amount_usd` number, `added_duration_days` integer,
   `proposed_end_date` string (10), `requires_separate_approval` string (8),
   `submission_count` integer, and `source_locator` string (128).
8. `accept_repair_bid`: POST `/v1/bid-acceptance`; required JSON-body
   arguments `project_id`, `bid_id`, `scope_id`, `scheduled_start`,
   `scheduled_end`, `normalized_total_usd`, and `approval_note`. Use a JSON
   body template with the matching `$argument` reference for every argument.
   Set read-only false, idempotent false, destructive false, and open-world
   true. Required response fields: `booking_id` string (64), `project_id`
   string (64), `accepted_bid_id` string (64), `scope_id` string (64),
   `scheduled_start` string (32), `scheduled_end` string (32),
   `normalized_total_usd` number, `status` string (64), `submission_count`
   integer, and `source_locator` string (128). This is approval-gated.
9. `approve_repair_change`: POST `/v1/change-approval`; required JSON-body
   arguments `project_id`, `change_request_id`, `approved_amount_usd`,
   `approved_end_date`, and `approval_note`. Bind each argument with
   `$argument`. Set read-only false, idempotent false, destructive false, and
   open-world true. Required response fields: `change_request_id` string (64),
   `project_id` string (64), `status` string (64), `reason` string (256),
   `added_scope` string (384), `added_amount_usd` number,
   `added_duration_days` integer, `proposed_end_date` string (10),
   `requires_separate_approval` string (8), `submission_count` integer, and
   `source_locator` string (128). This is the separately approved scope-change
   operation.
10. `extend_repair_insurance`: POST `/v1/insurance-extension`; required
    JSON-body arguments `policy_id`, `extension_id`, `effective_on`,
    `expires_on`, `premium_usd`, and `approval_note`. Bind each argument with
    `$argument`. Set read-only false, idempotent false, destructive false, and
    open-world true. Required response fields: `policy_id` string (64),
    `extension_id` string (64), `status` string (64), `effective_on` string
    (10), `expires_on` string (10), `premium_usd` number,
    `submission_count` integer, and `source_locator` string (128). This is
    approval-gated and synthetic only.
11. `record_repair_completion`: POST `/v1/completion`; required JSON-body
    arguments `project_id`, `completion_date`, `completion_evidence_id`,
    `completed_scope`, and `final_amount_usd`. Bind each argument with
    `$argument`. Set read-only false, idempotent false, destructive false, and
    open-world true. Required response fields: `completion_id` string (64),
    `project_id` string (64), `completion_date` string (10),
    `completion_evidence_id` string (96), `completed_scope` string (96),
    `final_amount_usd` number, `status` string (64), `submission_count`
    integer, and `source_locator` string (128). This is approval-gated and
    must not be described as a real repair.
12. `record_repair_payment`: POST `/v1/payment`; required JSON-body arguments
    `project_id`, `payment_date`, `invoice_id`, `amount_usd`,
    `payment_method`, and `note`. Bind each argument with `$argument`. Set
    read-only false, idempotent false, destructive false, and open-world true.
    Required response fields: `payment_receipt_id` string (64), `invoice_id`
    string (64), `project_id` string (64), `payment_date` string (10),
    `amount_usd` number, `payment_method` string (64), `status` string (64),
    `submission_count` integer, and `source_locator` string (128). This is
    approval-gated and must not move real money.
13. `get_repair_status`: GET `/v1/status`; no arguments; read-only, idempotent,
    non-destructive, and closed-world. Required fields: `case_id` string (64),
    `project_id` string (64), `project_status` string (64), `accepted_bid_id`
    string (64), `normalized_bid_total_usd` number, `change_request_id` string
    (64), `approved_change_amount_usd` number, `insurance_extension_id` string
    (64), `insurance_premium_usd` number, `contractor_total_usd` number,
    `project_total_usd` number, `budget_limit_usd` number,
    `budget_remaining_usd` number, `completion_id` string (64),
    `payment_receipt_id` string (64), `payment_amount_usd` number, and
    `source_locator` string (128).

Preserve every bid, each normalized cost component, scope inclusion and
exclusion, insurance expiry and extension, access and quiet hours, the accepted
scope, the later change request, all approvals, completion evidence, payment
receipt, budget arithmetic, project status, and every `home://` locator. Do not
create a shared workspace or perform any real-world action.

## Success conditions

- The proposal returns `review_required` and contains exactly these thirteen
  operations with generated response recipes and JSON body templates for the
  five POST operations.
- No computed response schema exceeds 32,768 bytes.
- No `/v1` endpoint or state-changing operation is invoked during setup.
