# PA-055 connect the synthetic appointment API

Use the current documentation at
`https://optional-hundred-useful-lightweight.trycloudflare.com/docs`. Create
one reviewed API connection. Do not invoke any service endpoint during setup.

Call `adapter.definition_template` first. Then call
`adapter.propose_definition` once with a new definition. Use the generated
response recipe format exactly: `response.kind` is `flat_object` or
`object_list`; object-list responses have `source_pointer`, `output_name`,
`max_items`, and `fields`; every string field has a `max_bytes` value. Use
`pagination:{kind:"none"}` on every operation. Every argument must include a
non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these five read-only operations and no others:

1. `get_appointment_profile`: GET `/v1/profile`; no arguments; flat required
   fields `case_id` (string 64), `patient_label` (string 96), `current_date`
   (string 10), `goal` (string 160), `decision_boundary` (string 160),
   `appointment_id` (string 64), `appointment_date` (string 10), `start_time`
   (string 5), `timezone` (string 64), `duration_minutes` (integer),
   `visit_type` (string 32), `clinician_label` (string 96), and
   `appointment_source_locator` (string 96). The appointment fields are
   projected from the documented `/appointment` object, with its
   `source_locator` mapped to `appointment_source_locator`.
2. `list_appointment_symptoms`: GET `/v1/symptoms`; no arguments; object-list
   from `/symptoms` named `symptoms`, maximum 3, with required fields
   `entry_id` (string 64), `observed_on` (string 10), `symptom` (string 64),
   `severity` (string 32), `context` (string 128), `duration` (string 64),
   `uncertainty` (string 160), and `source_locator` (string 96).
3. `list_appointment_medications`: GET `/v1/medications`; no arguments;
   object-list from `/medications` named `medications`, maximum 2, with
   required fields `medication_id` (string 64), `name` (string 96),
   `dose_recorded` (string 64), `status` (string 32), `recorded_on` (string
   10), and `source_locator` (string 96).
4. `list_appointment_test_results`: GET `/v1/test-results`; no arguments;
   object-list from `/results` named `results`, maximum 3, with required
   fields `result_id` (string 64), `test_name` (string 128), `collected_on`
   (string 10), `result_value` (string 64), `unit` (string 32),
   `reference_range` (string 64), `status` (string 32), `interpretation`
   (string 192), and `source_locator` (string 96). Preserve the pending
   result and its missing value as documented.
5. `list_appointment_concerns`: GET `/v1/concerns`; no arguments;
   object-list from `/concerns` named `concerns`, maximum 3, with required
   fields `concern_id` (string 64), `priority` (integer), `user_wording`
   (string 160), and `source_locator` (string 96).

For all five operations set read-only true, idempotent true, destructive
false, and open-world false. Do not propose diagnosis, triage, treatment,
medication, booking, escalation, provider-contact, or other undocumented
operations. Preserve dates, numbers, pending status, and all source locators.

## Success conditions

- The proposal is accepted for review and contains exactly the five operations
  above with valid generated response recipes.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
