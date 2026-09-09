# PA-070 set up the synthetic vehicle lifecycle API

Use the live Go Noema development instance to inspect the public documentation
at the exact origin below and propose one API connector. This setup is
synthetic. Do not call a `/v1` route, schedule a real service, contact a real
dealer or registry, expose a real VIN or address, authorize a charge, or move
money.

## Documentation source

- Adapter ID: `synthetic_vehicle_lifecycle_api_v1`
- Definition ID: `definition:synthetic_vehicle_lifecycle_api_v1`
- Display name: `Synthetic vehicle lifecycle API`
- Origin: `https://astronomy-interventions-romantic-cost.trycloudflare.com/`
- Source reference: `https://astronomy-interventions-romantic-cost.trycloudflare.com/docs`
- Revision: `v1`
- Authentication: none
- Scope: one personal Noema workspace
- Fixture version: `2026-09-09-vehicle-lifecycle-api-v1`

Read the documentation first. Use the returned page evidence directly. Do not
call `web.browse.switch_provider`. Then call `adapter.definition_template` once
with no arguments. After that call returns, do not call it again. Build one
proposal from the first template response and call
`adapter.propose_definition` exactly once. Do not retry or submit a second
proposal. Record the pending digest for operator review. The proposal must be
reviewed before any connector use.

## Required operations

The proposal must contain exactly these nine operations and no others:

1. `get_vehicle_profile`: GET `/v1/profile`; no arguments; read-only,
   idempotent, non-destructive, closed-world. Required fields: `case_id`
   string, `owner_label` string, `current_date` string, `vehicle_id` string,
   `vehicle_label` string, `vehicle_serial` string, `current_mileage` integer,
   `service_interval_miles` integer, `registration_deadline` string, `goal`
   string, `decision_boundary` string, and `source_locator` string.
2. `list_vehicle_service_history`: GET `/v1/service-history`; no arguments;
   read-only, idempotent, non-destructive, closed-world. Return an object-list
   from `/records` named `records`, maximum 2. Each item must contain
   `service_id`, `vehicle_id`, `service_date`, `mileage`, `service_type`,
   `vendor_label`, `notes`, and `source_locator`.
3. `list_vehicle_recalls`: GET `/v1/recalls`; no arguments; read-only,
   idempotent, non-destructive, closed-world. Return an object-list from
   `/recalls` named `recalls`, maximum 2. Each item must contain `recall_id`,
   `campaign_id`, `vehicle_id`, `serial_prefix`, `serial_start`, `serial_end`,
   `vehicle_serial`, `matches_vehicle`, `severity`, `remedy`, `status`, and
   `source_locator`.
4. `get_vehicle_registration`: GET `/v1/registration`; no arguments;
   read-only, idempotent, non-destructive, closed-world. Required fields:
   `registration_id`, `vehicle_id`, `jurisdiction_label`, `expires_on`,
   `status`, `required_documents` string array, `open_recall_blocks_renewal`
   boolean, and `source_locator`.
5. `list_vehicle_service_options`: GET `/v1/service-options`; no arguments;
   read-only, idempotent, non-destructive, closed-world. Return an object-list
   from `/options` named `options`, maximum 3. Each item must contain
   `option_id`, `vehicle_id`, `vendor_label`, `offered_date`,
   `offered_start_time`, `duration_minutes`, `included_services` string array,
   `estimated_total_usd`, `recall_campaign_id`, `registration_ready` boolean,
   `option_status`, and `source_locator`.
6. `get_vehicle_constraints`: GET `/v1/constraints`; no arguments; read-only,
   idempotent, non-destructive, closed-world. Required fields:
   `service_due_mileage`, `current_mileage`, `registration_deadline`,
   `service_window_start`, `service_window_end`, `budget_limit_usd`,
   `required_recall_campaign`, `required_document`, and `source_locator`.
7. `book_vehicle_service`: POST `/v1/service-bookings`; JSON-body arguments
   `vehicle_id`, `option_id`, `service_date`, `odometer_miles`,
   `recall_campaign_id`, `registration_deadline`, `estimated_total_usd`, and
   `approval_note`, each bound to the identically named `$argument`. Set
   read-only false, idempotent false, destructive false, open-world true. Do
   not include a `retry` field in proposal input; the server derives
   `retry: never` for this POST. This is an approval-gated synthetic write.
   Required response
   fields: `service_receipt_id`, `vehicle_id`, `option_id`, `service_date`,
   `odometer_miles`, `recall_campaign_id`, `estimated_total_usd`, `status`,
   `submission_count`, `next_service_mileage`, `next_service_due_date`,
   `registration_status`, `registration_deadline`, and `source_locator`.
8. `get_vehicle_service_receipt`: GET `/v1/service-receipt`; no arguments;
   read-only, idempotent, non-destructive, closed-world. Preserve the same
   receipt fields as `book_vehicle_service`.
9. `get_vehicle_status`: GET `/v1/status`; no arguments; read-only, idempotent,
   non-destructive, closed-world. Required fields: `vehicle_id`,
   `current_mileage`, `last_service_date`, `last_service_mileage`,
   `next_service_mileage`, `next_service_due_date`, `recall_campaign_id`,
   `recall_status`, `registration_status`, `registration_deadline`,
   `service_submission_count`, and `source_locator`.

Every operation must accept and return only `application/json`, use bounded
response projections, include explicit string and array limits, and use
`pagination: {kind: "none"}`. For generated `flat_object` and `object_list`
responses, omit `accepted_content_types`; the server supplies
`application/json`. Include `accepted_content_types: ["application/json"]`
only when using a `custom` response. The definition must remain below the
platform schema-size limit. Do not include an undocumented route or a real
vehicle operation.

### Compact response bounds

Use these small bounds. They cover every fixture value and keep each operation
below the 32 KiB platform limit. Do not use broad defaults for every string.

| Operation | Required response bounds |
| --- | --- |
| `get_vehicle_profile` | `case_id`, `current_date`, `vehicle_id`, `vehicle_serial`, `registration_deadline` 64 bytes; `owner_label`, `vehicle_label` 96; `goal`, `decision_boundary` 256; `source_locator` 96. |
| `list_vehicle_service_history` | `records.maxItems: 2`; IDs 32; dates 32; `service_type` 64; `vendor_label` 96; `notes` 160; locators 96. |
| `list_vehicle_recalls` | `recalls.maxItems: 2`; IDs and serial fields 64; `severity`, `status` 32; `remedy` 256; locator 96. |
| `get_vehicle_registration` | IDs and dates 64; `jurisdiction_label` 96; `status` 64; `required_documents.maxItems: 4`, item strings 64; locator 96. |
| `list_vehicle_service_options` | `options.maxItems: 3`; IDs and dates 64; `vendor_label` 96; `included_services.maxItems: 4`, item strings 96; `option_status` 64; locator 96. |
| `get_vehicle_constraints` | Dates 32; `required_recall_campaign`, `required_document` 64; locator 96. |
| `book_vehicle_service` and `get_vehicle_service_receipt` | IDs and dates 64; `status`, `registration_status` 64; locator 96. |
| `get_vehicle_status` | IDs and dates 64; `recall_status`, `registration_status` 64; locator 96. |

Use the manifest's exact camel-case keys when a custom schema is required:
`additionalProperties`, `maxBytes`, `maxItems`, and `items`. Do not use
snake-case variants. Every object must declare required fields and set
`additionalProperties` to `false`. Keep response limits in response recipes or
custom output schemas, not argument objects.

## Acceptance criteria

- The documentation page is opened before the proposal is built.
- One no-argument `adapter.definition_template` call is made before the
  proposal. No second template call is made.
- Exactly one `adapter.propose_definition` call returns `review_required`.
- The proposal contains exactly the nine named operations and paths above.
- All GET operations are automatic reads. Only `book_vehicle_service` is a
  write, and it is approval-gated. The server derives `retry: never`; the
  proposal input must omit a `retry` field.
- Setup makes no `/v1` request and no state-changing request.
- The operator can accept the inspected pending digest and configure the
  resulting personal connection for automatic reads and always-ask writes.
