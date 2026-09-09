# PA-059 connect the synthetic health-plan comparison API

Use the current documentation at
`https://policies-porter-defining-guided.trycloudflare.com/docs`. Create one reviewed API connection. Do not invoke any
service endpoint during setup.

Call `adapter.definition_template` first. Then call
`adapter.propose_definition` once with a new definition. Use the generated
response recipe format exactly: `response.kind` is `flat_object` or
`object_list`; object-list responses have `source_pointer`,
`output_name`, `max_items`, and `fields`; every string field has a
`max_bytes` value. Use `pagination:{kind:"none"}` on every operation.
Every argument must include a non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these three read-only operations and no others:

1. `get_health_plan_comparison_profile`: GET `/v1/profile`; no arguments;
   flat required fields `case_id` (string 64), `patient_label` (string
   96), `current_date` (string 10), `goal` (string 256),
   `household_size` (integer), `network_preference` (string 128),
   `scenario_period` (string 64), `expected_cost_rule` (string 512),
   `worst_case_rule` (string 512), `decision_boundary` (string 256), and
   `source_locator` (string 128).
2. `list_health_plan_options`: GET `/v1/plans`; no arguments; object-list
   from `/plans` named `plans`, maximum 2, with required fields
   `plan_id` (string 64), `plan_name` (string 128),
   `premium_monthly_usd` (number), `medical_deductible_usd` (number),
   `prescription_deductible_usd` (number),
   `out_of_pocket_max_usd` (number), `network_scope` (string 96),
   `out_of_network_cost_share` (string 64),
   `primary_care_copay_usd` (number), `specialist_copay_usd` (number),
   `urgent_care_copay_usd` (number), `lab_coinsurance_percent` (number),
   `generic_copay_usd` (number), `preferred_brand_copay_usd` (number),
   `specialty_copay_usd` (number), `excluded_drug_id` (string 64),
   `excluded_drug_label` (string 128), `excluded_drug_cash_price_usd`
   (number), `network_note` (string 256), and `source_locator` (string
   128).
3. `list_health_plan_usage`: GET `/v1/usage`; no arguments; object-list
   from `/usage` named `usage`, maximum 7, with required fields
   `usage_id` (string 64), `kind` (string 64), `label` (string 128),
   `units` (integer), `allowed_unit_cost_usd` (number),
   `cost_share_rule` (string 96), `drug_id` (string 64),
   `drug_tier` (string 64), and `source_locator` (string 128).

Set read-only true, idempotent true, destructive false, and open-world false
for all three operations. Preserve every premium, deductible, maximum, copay,
coinsurance percentage, network label, excluded-drug field, usage unit,
allowed-cost number, cost-share rule, and `plan://` locator. Do not propose
enrollment, cancellation, claim submission, insurer contact, payment,
diagnosis, treatment, or any undocumented operation.

## Success conditions

- The proposal is accepted for review and contains exactly the three
  operations above with valid generated response recipes.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
