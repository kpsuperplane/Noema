# PA-061 connect the synthetic discharge-transition API

Use the current documentation at
`https://emphasis-actually-tan-vermont.trycloudflare.com/docs`. Create one
reviewed API connection. Do not invoke any service endpoint during setup.

Call `adapter.definition_template` first. Then call
`adapter.propose_definition` once with a new definition. Do not include a
`retry` or `retry_policy` property in an operation proposal. Noema computes
the retry policy from the method and safety flags. Use the generated response recipe format
exactly: `response.kind` is `flat_object` or `object_list`; object-list
responses have `source_pointer`, `output_name`, `max_items`, and `fields`;
every string field has a `max_bytes` value. Use `pagination:{kind:"none"}` on
every operation. Every argument must include a non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these ten operations and no others:

1. `get_discharge_transition_profile`: GET `/v1/profile`; no arguments; flat
   required fields `case_id` (string 64), `person_label` (string 96),
   `current_date` (string 10), `discharge_date` (string 10), `goal` (string
   256), `next_day_appointment` (string 10), `decision_boundary` (string 256),
   and `source_locator` (string 128).
2. `list_discharge_orders`: GET `/v1/discharge-orders`; no arguments;
   object-list from `/orders` named `orders`, maximum 2, with required fields
   `order_id` (string 64), `medication_id` (string 64),
   `medication_label` (string 128), `instruction` (string 128), `status`
   (string 64), `ordered_on` (string 10), `conflict_group` (string 64), and
   `source_locator` (string 128).
3. `list_pre_discharge_medications`: GET `/v1/old-medications`; no arguments;
   object-list from `/medications` named `medications`, maximum 2, with the
   required fields `medication_id` (string 64), `medication_label` (string
   128), `instruction` (string 128), `status` (string 64), `recorded_on`
   (string 10), `conflict_group` (string 64), and `source_locator` (string
   128).
4. `list_discharge_equipment`: GET `/v1/equipment`; no arguments;
   object-list from `/equipment` named `equipment`, maximum 2, with required
   fields `equipment_id` (string 64), `label` (string 128),
   `delivery_status` (string 64), `delivery_window` (string 64),
   `setup_required` (string 16), and `source_locator` (string 128).
5. `get_discharge_transport`: GET `/v1/transport`; no arguments; flat required
   fields `transport_id` (string 64), `provider_label` (string 128),
   `pickup_on` (string 10), `pickup_time` (string 8), `accessibility` (string
   64), `status` (string 32), `destination` (string 128), and `source_locator`
   (string 128).
6. `get_discharge_follow_up_appointment`: GET `/v1/appointment`; no arguments;
   flat required fields `appointment_id` (string 64), `appointment_on` (string
   10), `start_time` (string 8), `timezone` (string 64), `visit_type` (string
   128), `clinician_label` (string 96), `status` (string 32), and
   `source_locator` (string 128).
7. `get_discharge_caregiver`: GET `/v1/caregiver`; no arguments; flat required
   fields `caregiver_id` (string 64), `caregiver_label` (string 96),
   `authorization_status` (string 32), `availability_window` (string 64),
   `handoff_status` (string 32), and `source_locator` (string 128).
8. `list_discharge_warnings`: GET `/v1/warnings`; no arguments; object-list
   from `/warnings` named `warnings`, maximum 2, with required fields
   `warning_id` (string 64), `instruction_text` (string 256), `supplied_by`
   (string 96), `priority` (string 64), and `source_locator` (string 128).
9. `coordinate_discharge_transition`: POST `/v1/transition-coordination`;
   required JSON-body arguments `conflict_group` (string), `equipment_ids`
   (string_array), `transport_id` (string), `appointment_id` (string),
   `caregiver_id` (string), and `factual_note` (string). Include every
   argument in `json_body_template` using `$argument` references. Set
   read-only false, idempotent false, destructive false, open-world true. Its
   flat response must require `coordination_id` (string
   64), `receipt_id` (string 64), `conflict_group` (string 64),
   `equipment_status` (string 32), `transport_status` (string 32),
   `appointment_status` (string 32), `caregiver_handoff_status` (string 32),
   `clinician_clarification_status` (string 32), `status` (string 32),
   `request_count` (integer), `submitted_on` (string 10), and
   `source_locator` (string 128). This is the only unsafe operation and must
   remain approval-gated.
10. `get_discharge_transition_status`: GET `/v1/transition-status`; no
    arguments; flat required fields `coordination_id` (string 64),
    `receipt_id` (string 64), `conflict_group` (string 64),
    `equipment_status` (string 32), `transport_status` (string 32),
    `appointment_status` (string 32), `caregiver_handoff_status` (string 32),
    `clinician_clarification_status` (string 32), `status` (string 32),
    `request_count` (integer), and `source_locator` (string 128).

Set read-only true, idempotent true, destructive false, and open-world false
for operations 1–8 and 10. Preserve every medicine instruction, conflict
label, logistics status, warning text, receipt, request count, and
`transition://` locator. Do not propose a dose change, diagnosis, treatment,
real clinician contact, real transport, payment, or undocumented operation.

## Success conditions

- The proposal is accepted for review and contains exactly the ten operations
  above with valid generated response recipes and a JSON body template for the
  POST.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
