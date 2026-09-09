# PA-068 connect the synthetic utility optimization API

Inspect the live documentation at
`https://rapid-started-ton-sic.trycloudflare.com/docs`. Create one reviewed API
connection. Do not invoke a `/v1` endpoint during setup.

Use adapter ID `synthetic_utility_optimization_api` and definition ID
`definition:synthetic_utility_optimization_api`. Set `new_definition.origin`
exactly to `https://rapid-started-ton-sic.trycloudflare.com/`, including the
trailing slash. Set `source_reference` to the documentation URL. Use revision
`v1` and authentication `{kind:"none"}`.

Call `adapter.definition_template` first with an empty object. Then call
`adapter.propose_definition` exactly once. After that call succeeds, never call
`adapter.propose_definition` again, even if a reviewer asks for more evidence.
Add later evidence to `RESULT.md` or `TASK.md` with file tools or Luau. Do not
include a custom retry or response transform.

Use generated response recipes. Every response field must have a valid source
pointer and every string field must have a `max_bytes` bound. Every object-list
response must specify its source pointer, output name, maximum item count, and
item fields. Use `pagination:{kind:"none"}` on every operation. Every argument
must have a non-empty description.

The proposal must contain exactly these eight operations and no others.

1. `get_utility_profile`: GET `/v1/profile`; no arguments; read-only,
   idempotent, non-destructive, closed-world. Required fields: `case_id`
   string (64), `owner_label` string (96), `current_date` string (10),
   `time_zone` string (64), `service_address` string (128), `project_label`
   string (128), `goal` string (512), `decision_boundary` string (384),
   `budget_limit_usd` number, `required_switch_date` string (10),
   `reliability_preference` string (256), and `source_locator` string (128).
2. `list_utility_usage`: GET `/v1/usage`; no arguments; read-only, idempotent,
   non-destructive, closed-world. Return an object-list from `/months` named
   `months`, maximum 12. Each item must contain `month` string (7), `kwh`
   integer, and `source_locator` string (64).
3. `list_utility_plans`: GET `/v1/plans`; no arguments; read-only, idempotent,
   non-destructive, closed-world. Return an object-list from `/plans` named
   `plans`, maximum 3. Each item must contain `plan_id` string (64),
   `plan_name` string (128), `provider_label` string (128),
   `electric_rate_usd_per_kwh` number, `electric_base_monthly_usd` number,
   `internet_standard_monthly_usd` number, `internet_intro_monthly_usd`
   number, `internet_intro_months` integer, `intro_expires_on` string (10),
   `equipment_charge_usd` number, `reliability_uptime_percent` number,
   `data_cap_label` string (64), `billing_note` string (256), `plan_status`
   string (32), and `source_locator` string (128).
4. `get_current_utility_service`: GET `/v1/current-service`; no arguments;
   read-only, idempotent, non-destructive, closed-world. Required fields:
   `current_provider_label` string (128), `current_plan_id` string (64),
   `current_monthly_total_usd` number, `early_exit_fee_usd` number,
   `projected_prorated_final_service_usd` number, `current_contract_end`
   string (10), `switch_date` string (10), and `source_locator` string (96).
5. `get_utility_constraints`: GET `/v1/constraints`; no arguments;
   read-only, idempotent, non-destructive, closed-world. Required fields:
   `minimum_uptime_percent` number, `data_cap_requirement` string (32),
   `predictable_billing_required` string (8), `budget_limit_usd` number,
   `comparison_period_months` integer, `usage_unit` string (16),
   `required_switch_date` string (10), and `source_locator` string (128).
6. `switch_utility_bundle`: POST `/v1/switch`; required JSON-body arguments
   `case_id`, `selected_plan_id`, `switch_date`,
   `annual_new_plan_total_usd`, `old_provider_final_bill_usd`,
   `annual_switch_total_usd`, and `approval_note`. Bind each argument to the
   identically named `$argument`. Set read-only false, idempotent false,
   destructive false, and open-world true. Required response fields:
   `activation_id` string (64), `case_id` string (64), `selected_plan_id`
   string (64), `activation_date` string (10),
   `annual_new_plan_total_usd` number, `old_provider_final_bill_usd` number,
   `annual_switch_total_usd` number, `status` string (64),
   `submission_count` integer, and `source_locator` string (128). This is
   approval-gated and synthetic only.
7. `get_old_provider_final_bill`: GET `/v1/final-bill`; no arguments;
   read-only, idempotent, non-destructive, closed-world. Required fields:
   `bill_id` string (64), `provider_label` string (128), `current_plan_id`
   string (64), `bill_date` string (10), `prorated_service_usd` number,
   `early_exit_fee_usd` number, `final_amount_usd` number, `status` string
   (64), `submission_count` integer, and `source_locator` string (128).
8. `get_utility_status`: GET `/v1/status`; no arguments; read-only, idempotent,
   non-destructive, closed-world. Required fields: `case_id` string (64),
   `activation_id` string (64), `selected_plan_id` string (64),
   `activation_date` string (10), `annual_new_plan_total_usd` number,
   `old_provider_bill_id` string (64), `old_provider_final_bill_usd` number,
   `annual_switch_total_usd` number, `budget_limit_usd` number,
   `budget_remaining_usd` number, `project_status` string (64),
   `switch_submission_count` integer, and `source_locator` string (128).

Preserve every usage month, all three plans and their pricing components,
introductory-rate dates, equipment charge, exit fee, uptime and data-cap
requirements, reliability preference, current service, final old-provider bill,
activation, arithmetic, status, and every `utility://` locator. Do not create a
shared workspace or perform a real service change.

## Success conditions

- The proposal returns `review_required` and contains exactly these eight
  operations, with generated response recipes and one complete JSON body
  template for the POST operation.
- No computed response schema exceeds 32,768 bytes.
- No `/v1` endpoint or state-changing operation is invoked during setup.
