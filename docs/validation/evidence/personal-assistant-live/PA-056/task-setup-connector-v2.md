# PA-056 corrected connector proposal

The first setup task submitted one proposal and received
`invalid_proposal`. Its record-bundle response mixed flat fields and an object
list. This corrected task separates those shapes, which is required by the
adapter response recipe.

Use the current documentation at
`https://crops-titled-gear-claims.trycloudflare.com/docs`. Create one reviewed
API connection. Do not invoke any service endpoint during setup.

Call `adapter.definition_template` first. Then call
`adapter.propose_definition` once with a new definition. Use the generated
response recipe format exactly: `response.kind` is `flat_object` or
`object_list`; object-list responses have `source_pointer`, `output_name`,
`max_items`, and `fields`; every string field has a `max_bytes` value. Use
`pagination:{kind:"none"}` on every operation. Every argument must include a
non-empty description.

Use authentication `{kind:"none"}` for every operation. The proposal must
contain exactly these twelve operations and no others:

1. `get_referral_profile`: GET `/v1/profile`; no arguments; flat required
   fields `case_id` (string 64), `patient_label` (string 96), `current_date`
   (string 10), `goal` (string 192), `decision_boundary` (string 192),
   `referral_order_id` (string 64), `specialty` (string 96), `order_date`
   (string 10), `expires_on` (string 10), `referral_source_locator` (string
   96), `plan_name` (string 96), `max_travel_minutes` (integer),
   `max_transport_cost_usd` (number), `arrival_buffer_minutes` (integer),
   `destination_scope` (string 160), and `constraints_source_locator`
   (string 96). Project the referral and constraints fields from their
   documented nested objects.
2. `list_referral_prerequisites`: GET `/v1/prerequisites`; no arguments;
   object-list from `/prerequisites` named `prerequisites`, max 3, with
   required fields `prerequisite_id` (string 64), `name` (string 128),
   `required` (boolean), `status` (string 32), `due_on` (string 10),
   `booking_required` (boolean), and `source_locator` (string 96).
3. `get_referral_record_bundle`: GET `/v1/records`; no arguments; flat
   required fields `bundle_id` (string 64), `bundle_status` (string 32), and
   `destination_scope` (string 160). Do not include the nested records array
   in this operation.
4. `list_referral_records`: GET `/v1/records`; no arguments; object-list from
   `/records` named `records`, max 3, with required fields `record_id` (string
   64), `record_kind` (string 32), `title` (string 128), `recorded_on`
   (string 10), `status` (string 32), and `source_locator` (string 96).
5. `list_referral_network`: GET `/v1/network`; no arguments; object-list from
   `/network` named `network`, max 3, with required fields `provider_id`
   (string 64), `provider_name` (string 128), `plan_name` (string 96),
   `in_network` (boolean), `accessibility` (string 96),
   `accepting_new_referrals` (boolean), `address` (string 128), and
   `source_locator` (string 96).
6. `list_referral_slots`: GET `/v1/slots`; no arguments; object-list from
   `/slots` named `slots`, max 3, with required fields `slot_id` (string 64),
   `provider_id` (string 64), `provider_name` (string 128),
   `appointment_date` (string 10), `start_time` (string 5), `timezone` (string
   64), `duration_minutes` (integer), `status` (string 32), `in_network`
   (boolean), `travel_minutes` (integer), and `source_locator` (string 96).
7. `list_referral_transport_options`: GET `/v1/transport-options`; no
   arguments; object-list from `/options` named `options`, max 3, with required
   fields `option_id` (string 64), `kind` (string 32), `provider_label`
   (string 128), `travel_minutes` (integer), `cost_usd` (number),
   `availability` (string 32), `arrival_buffer_minutes` (integer), and
   `source_locator` (string 96).
8. `schedule_referral_prerequisite`: POST `/v1/prerequisite-bookings`; JSON
   body arguments `prerequisite_id`, `preferred_date`, and `note` (required
   strings with non-empty descriptions); body template maps each argument by
   name. Flat required response fields are `booking_id` (string 64),
   `receipt_id` (string 64), `prerequisite_id` (string 64), `status` (string
   32), `scheduled_on` (string 10), and `request_count` (integer). Set
   read-only false, idempotent true, destructive true, and open-world false.
9. `transfer_referral_records`: POST `/v1/record-transfers`; JSON body
   arguments `bundle_id`, `destination_provider_id`, and `authorization_note`
   (required strings with non-empty descriptions); flat required response
   fields `transfer_id` (string 64), `receipt_id` (string 64), `bundle_id`
   (string 64), `destination_provider_id` (string 64), `status` (string 32),
   and `request_count` (integer). Set read-only false, idempotent true,
   destructive true, and open-world false.
10. `book_referral_appointment`: POST `/v1/appointments`; JSON body
    arguments `referral_order_id`, `slot_id`, and `note` (required strings with
    non-empty descriptions); flat required response fields `booking_id`
    (string 64), `receipt_id` (string 64), `referral_order_id` (string 64),
    `slot_id` (string 64), `status` (string 32), and `request_count` (integer).
    Set read-only false, idempotent true, destructive true, and open-world
    false.
11. `book_referral_transport`: POST `/v1/transport-bookings`; JSON body
    arguments `appointment_id`, `option_id`, and `note` (required strings with
    non-empty descriptions); flat required response fields
    `transport_booking_id` (string 64), `receipt_id` (string 64),
    `appointment_id` (string 64), `option_id` (string 64), `status` (string
    32), and `request_count` (integer). Set read-only false, idempotent true,
    destructive true, and open-world false.
12. `get_referral_follow_up_status`: GET `/v1/follow-up-status`; no
    arguments; flat required fields `prerequisite_status` (string 32),
    `prerequisite_receipt_id` (string 64), `transfer_status` (string 32),
    `transfer_receipt_id` (string 64), `appointment_status` (string 32),
    `appointment_receipt_id` (string 64), `booked_slot_id` (string 64),
    `transport_status` (string 32), `transport_receipt_id` (string 64),
    `booked_transport_option_id` (string 64), `next_follow_up_date` (string
    16), and `source_locator` (string 96).

For operations 1–7 and 12 set read-only true, idempotent true, destructive
false, and open-world false. Only operations 8–11 are unsafe and
approval-gated. Preserve explicit booleans, numbers, dates, and all source
locators. Do not propose diagnosis, treatment, prescription, real booking,
provider contact, payment, or any undocumented operation.

## Success conditions

- The corrected proposal is accepted for review and contains exactly the
  twelve operations above with valid generated response recipes.
- No computed response schema exceeds 32,768 bytes.
- No service endpoint or state-changing operation is invoked during setup.
