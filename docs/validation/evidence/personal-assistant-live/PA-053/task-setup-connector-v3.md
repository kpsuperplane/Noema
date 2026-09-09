# PA-053 connect the synthetic medical-record API (final bounded proposal)

Use the current documentation at
`https://referring-ciao-transportation-employ.trycloudflare.com/docs` as the
source of truth. Create exactly one reviewed API connection for this synthetic
service.

Use exactly these five operations:

1. `get_medical_profile`: GET `/v1/profile`; no arguments; return
   `case_id`, `patient_label`, `current_date`, `goal`, and `decision_boundary`.
2. `list_medical_records`: GET `/v1/records`; no arguments; return `sources`
   and `records` arrays. For each source return `provider_id`, `provider_name`,
   `access_status`, `export_date`, and `source_locator`. For each record return
   `record_id`, `provider_id`, `record_type`, `title`, `observed_on`, `status`,
   `summary`, `canonical_key`, and `source_locator`. Cap each array at 8 items
   and each string at 256 bytes.
3. `get_medical_access_status`: GET `/v1/access-status`; no arguments; return
   `source_id`, `status`, `request_count`, and `available`.
4. `request_medical_source_access`: POST `/v1/access-requests`; JSON body
   fields `source_id` and `note`; return `request_id`, `receipt_id`,
   `source_id`, `status`, and `request_count`. Use authentication `none` and
   mark this operation unsafe, destructive, idempotent, and approval-gated.
5. `list_recovered_medical_records`: GET `/v1/records/provider-d-001`; no
   arguments; return a `records` array with the same nine record fields as
   operation 2, capped at 8 items. Keep this operation read-only.

Use authentication `none` for the connection. Keep all GET operations
read-only and automatic. Preserve explicit boolean `false` values and JSON
nulls. Do not add any diagnosis, medication, treatment, billing, appointment,
provider-contact, or other undocumented operation. Do not invoke any operation
or make any external state change during setup.

## Success conditions

- One reviewed proposal has exactly the five operations above.
- Paths, methods, arguments, response fields, bounds, and behavior flags match
  the current documentation.
- Only the access-request POST is unsafe and approval-gated.
