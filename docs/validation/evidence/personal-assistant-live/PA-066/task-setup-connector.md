# PA-066 connect the synthetic home-maintenance API

Inspect the current documentation at
`https://ads-beast-palace-mug.trycloudflare.com/docs`. Create one reviewed API
connection. Do not invoke a `/v1` endpoint during setup.

Set `new_definition.origin` exactly to
`https://ads-beast-palace-mug.trycloudflare.com/`, including the trailing
slash. Set `source_reference` to the documentation URL. Use definition
revision `v1` and authentication `{kind:"none"}`.

Call `adapter.definition_template` first with an empty object. Then call
`adapter.propose_definition` exactly once with a new definition. Do not
include a custom retry or response transform. Use generated response recipes:
`response.kind` is `flat_object` or `object_list`; every response field has a
valid `source_pointer`; every string field has `max_bytes`; and every
object-list response has `source_pointer`, `output_name`, `max_items`, and
`fields`. Use `pagination:{kind:"none"}` on every operation. Every argument
must have a non-empty description.

The proposal must contain exactly these nine operations and no others:

1. `get_maintenance_profile`: GET `/v1/profile`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. The flat response requires
   `case_id` (string, 64 bytes), `owner_label` (string, 96), `current_date`
   (string, 10), `time_zone` (string, 64), `property_label` (string, 128),
   `goal` (string, 384), `decision_boundary` (string, 256),
   `service_budget_usd` (number), and `source_locator` (string, 128).
2. `get_hvac_manual`: GET `/v1/manual`; no arguments; read-only, idempotent,
   non-destructive, and closed-world. The flat response requires `manual_id`
   (string, 64), `asset_id` (string, 64), `manufacturer` (string, 96),
   `model` (string, 96), `service_interval_days` (integer),
   `filter_interval_days` (integer), `recommended_months` (string, 64),
   `service_duration_minutes` (integer), `safety_note` (string, 256), and
   `source_locator` (string, 128).
3. `get_hvac_warranty`: GET `/v1/warranty`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. The flat response requires
   `warranty_id` (string, 64), `asset_id` (string, 64), `provider_label`
   (string, 128), `starts_on` (string, 10), `expires_on` (string, 10),
   `coverage_summary` (string, 192), `serial_match` (string, 32), and
   `source_locator` (string, 128).
4. `get_last_hvac_service`: GET `/v1/service-history`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. The flat response
   requires `receipt_id` (string, 64), `asset_id` (string, 64), `service_date`
   (string, 10), `vendor_label` (string, 128), `service_type` (string, 64),
   `amount_usd` (number), `notes` (string, 192), and `source_locator` (string,
   128).
5. `get_maintenance_constraints`: GET `/v1/constraints`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. The flat response
   requires `safe_service_window_start` (string, 10),
   `safe_service_window_end` (string, 10), `avoid_dates` (string, 64),
   `avoid_reason` (string, 128), `quiet_hours` (string, 96), `access_window`
   (string, 128), `budget_limit_usd` (number), `budget_scope` (string, 256),
   and `source_locator` (string, 128).
6. `list_maintenance_quotes`: GET `/v1/quotes`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. Return an object-list from
   `/quotes` named `quotes`, maximum 3, with required fields `quote_id`
   (string, 64), `vendor_label` (string, 128), `asset_id` (string, 64),
   `offered_date` (string, 10), `offered_start_time` (string, 5),
   `duration_minutes` (integer), `service_type` (string, 64), `amount_usd`
   (number), `includes_filter` (string, 16), `warranty_eligible` (string, 16),
   `quote_status` (string, 32), and `source_locator` (string, 128).
7. `get_existing_maintenance_plan`: GET `/v1/existing-plan`; no arguments;
   read-only, idempotent, non-destructive, and closed-world. The flat response
   requires `plan_id` (string, 64), `recurring_task_id` (string, 96),
   `plan_status` (string, 64), `next_due_date` (string, 10), and
   `source_locator` (string, 128).
8. `record_hvac_service_receipt`: POST `/v1/service-receipts`; required
   JSON-body arguments `asset_id`, `service_date`, `vendor_label`,
   `service_type`, `amount_usd`, `warranty_id`, and `notes`. The string
   arguments need non-empty descriptions. Use a JSON body template with the
   matching `$argument` reference for every argument. Set read-only false,
   idempotent false, destructive false, and open-world true. The response
   requires `receipt_id` (string, 64 bytes), `asset_id` (string, 64),
   `service_date` (string, 10), `vendor_label` (string, 128), `service_type`
   (string, 64), `amount_usd` (number), `warranty_id` (string, 64), `notes`
   (string, 192), `submission_count` (integer), `status` (string, 64),
   `next_due_date` (string, 10), and `source_locator` (string, 128). This is
   the only unsafe operation and must remain approval-gated.
9. `get_maintenance_status`: GET `/v1/status`; no arguments; read-only,
   idempotent, non-destructive, and closed-world. The flat response requires
   `case_id` (string, 64), `asset_id` (string, 64), `derived_due_date` (string,
   10), `selected_quote_id` (string, 64), `selected_amount_usd` (number),
   `budget_limit_usd` (number), `budget_remaining_usd` (number),
   `service_receipt_id` (string, 64), `service_submission_count` (integer),
   `service_status` (string, 64), `next_due_date` (string, 10), and
   `source_locator` (string, 128).

Preserve the exact asset, manufacturer, model, serial, manual intervals,
warranty dates and coverage, last service date and amount, safe window, avoid
dates, access and quiet hours, every quote and source locator, selected quote,
service receipt, status, budget arithmetic, and derived dates. Do not dispatch a
real technician, authorize a real repair, contact a provider, charge money, or
create a shared workspace.

## Success conditions

- The proposal returns `review_required` and contains exactly these nine
  operations with generated response recipes and a JSON body template for the
  POST operation.
- No computed response schema exceeds 32,768 bytes.
- No `/v1` endpoint or state-changing operation is invoked during setup.
