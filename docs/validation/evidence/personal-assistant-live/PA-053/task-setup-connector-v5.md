# PA-053 connect the synthetic medical-record API (small valid proposal)

Use the current documentation at
`https://referring-ciao-transportation-employ.trycloudflare.com/docs`. Create
one reviewed API connection. Do not invoke any service endpoint.

Call `adapter.definition_template` first. Then call `adapter.propose_definition`
once with a new definition. Use the generated response recipe format exactly:
`response.kind` is `flat_object` or `object_list`; object-list responses have
`source_pointer`, `output_name`, `max_items`, and `fields`; every string field
has a `max_bytes` value. Use `pagination:{kind:"none"}` on every operation.
Every argument must include a non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these six operations and no others:

1. `get_medical_profile`: GET `/v1/profile`; no arguments; flat fields
   `case_id` (string 64), `patient_label` (string 128), `current_date` (string
   10), `goal` (string 128), and `decision_boundary` (string 128), all required.
2. `list_medical_sources`: GET `/v1/providers`; no arguments; object-list from
   `/providers` named `providers`, max 4, fields `provider_id` (string 64),
   `provider_name` (string 96), `access_status` (string 32), `export_date`
   (string 10), and `source_locator` (string 96), all required.
3. `list_medical_records`: GET `/v1/records`; no arguments; object-list from
   `/records` named `records`, max 8, fields `record_id` (string 64),
   `provider_id` (string 64), `record_type` (string 32), `title` (string 64),
   `observed_on` (string 10), `status` (string 24), `summary` (string 128),
   `canonical_key` (string 64), and `source_locator` (string 96), all required.
4. `get_medical_access_status`: GET `/v1/access-status`; no arguments; flat
   fields `source_id` (string 64), `status` (string 32), `request_count`
   (integer), and `available` (boolean), all required. Preserve false.
5. `request_medical_source_access`: POST `/v1/access-requests`; required
   JSON-body arguments `source_id` and `note`, each with a non-empty
   description; body template
   `{source_id:{$argument:"source_id"},note:{$argument:"note"}}`; flat
   response fields `request_id` (string 64), `receipt_id` (string 64),
   `source_id` (string 64), `status` (string 32), and `request_count`
   (integer), all required. Set read-only false, idempotent true, destructive
   true, open-world false.
6. `list_recovered_medical_records`: GET `/v1/records/provider-d-001`; no
   arguments; object-list from `/records` named `records`, max 8, with the same
   nine fields and bounds as operation 3. Keep it read-only.

For operations 1–4 and 6 set read-only true, idempotent true, destructive
false, and open-world false. GET operations are automatic. Only operation 5 is
unsafe and approval-gated. Preserve JSON nulls and explicit boolean false
values. Do not invent diagnosis, medication, treatment, billing, appointment,
provider-contact, or other undocumented operations.

## Success conditions

- The proposal is accepted for review and contains exactly the six operations
  above with valid generated response recipes.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
