# PA-058 connect the synthetic provider directory

Use the current documentation at
`https://web-continues-webster-cardiovascular.trycloudflare.com/docs`. Create one
reviewed API connection. Do not invoke any service endpoint during setup.

Call `adapter.definition_template` first. Then call
`adapter.propose_definition` once with a new definition. Use the generated
response recipe format exactly: `response.kind` is `flat_object` or
`object_list`; object-list responses have `source_pointer`, `output_name`,
`max_items`, and `fields`; every string field has a `max_bytes` value. Use
`pagination:{kind:"none"}` on every operation. Every argument must include a
non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these three read-only operations and no others:

1. `get_provider_search_profile`: GET `/v1/profile`; no arguments; flat
   required fields `case_id` (string 64), `current_date` (string 10),
   `goal` (string 256), `specialty` (string 96), `plan_name` (string 96),
   `required_language` (string 32), `wheelchair_access_required` (string
   32), `max_fee_usd` (number), `max_travel_minutes` (integer),
   `appointment_window` (string 64), `decision_boundary` (string 192), and
   `source_locator` (string 128).
2. `list_provider_options`: GET `/v1/providers`; no arguments; object-list
   from `/providers` named `providers`, maximum 4, with required fields
   `provider_id` (string 64), `provider_name` (string 128), `specialty`
   (string 96), `network_status` (string 32), `language_support` (string
   96), `wheelchair_access` (string 32), `fee_usd` (number),
   `travel_minutes` (integer), `availability_status` (string 32),
   `next_available_date` (string 16), `quality_note` (string 192), and
   `source_locator` (string 128).
3. `check_provider_availability`: GET
   `/v1/providers/{provider_id}/availability`; one required path argument
   `provider_id` (string, with a non-empty description); flat required fields
   `provider_id` (string 64), `availability_status` (string 32),
   `earliest_available_date` (string 16), `appointment_window` (string 64),
   `verification_note` (string 192), and `source_locator` (string 128).

Set read-only true, idempotent true, destructive false, and open-world false
for all three operations. Preserve the source status labels, numbers, dates,
fees, limits, and every `provider://` locator. Do not propose a booking,
diagnosis, treatment, clinician contact, insurance change, payment, or any
undocumented operation.

## Success conditions

- The proposal is accepted for review and contains exactly the three
  operations above with valid generated response recipes.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
