# PA-072 corrected synthetic returns API setup

Use the live Go Noema development instance to inspect the exact public
documentation below and propose one API connector. This is a correction of the
first PA-072 setup attempt. The first accepted definition used field names that
were not present in the fixture response. This task uses the corrected fixture
response contract. Do not use the old connector or its origin.

This setup is synthetic. Do not call a service route, contact a merchant or
carrier, ship an item, issue a refund, authorize a payment, or move money.

## Documentation source

- Adapter ID: `synthetic_returns_api_v2`
- Definition ID: `definition:synthetic_returns_api_v2`
- Display name: `Synthetic returns API corrected`
- Origin: `https://materials-huntington-henderson-collective.trycloudflare.com/`
- Source reference: `https://materials-huntington-henderson-collective.trycloudflare.com/docs`
- Revision: `v2`
- Authentication: none
- Scope: one personal Noema workspace
- Fixture version: `2026-09-09-returns-api-v1`

Read the documentation first. Use the returned page evidence directly. Do not
call `web.browse.switch_provider`. Then call
`adapter.definition_template` once with no arguments. Do not call it again.
Use that response to build exactly one `adapter.propose_definition` request.
Do not retry the proposal. The proposal must be reviewed before connector use.

Do not include a `retry` field in any operation proposal. The server derives
`transport_safe_read` for read-only GET operations and `never` for the
non-read-only POST. Do not include `accepted_content_types` on generated
responses. Use `pagination: {kind: "none"}` for every operation.

## Required operations and exact response fields

The proposal must contain exactly these eight operations and no others. Use the
field names listed here. They are the connector contract. The fixture also
returns descriptive aliases such as `owner_label`, `item_serial`, and
`carrier_label`; ignore those aliases and map only the stable names below.

1. `get_return_profile`: GET `/v1/profile`; read-only, idempotent,
   non-destructive, closed-world. Required fields are `case_id`, `owner`,
   `current_date`, `goal`, `decision_boundary`, `target_description`, and
   `source_locator`.
2. `list_purchases`: GET `/v1/purchases`; read-only, idempotent,
   non-destructive, closed-world. Project `/purchases` to `purchases`, with
   at most two objects. Each object requires `purchase_id`, `item_name`,
   `item_variant`, `serial_number`, `purchase_date`, `return_deadline`,
   `condition`, `eligible`, `final_sale`, `seller`, `original_charge_id`,
   `amount_usd`, and `source_locator`.
3. `get_return_policy`: GET `/v1/return-policy`; read-only, idempotent,
   non-destructive, closed-world. Required fields are `policy_id`,
   `window_days`, `current_date`, `allowed_reasons`, `shipping_required`,
   `refund_basis`, `restocking_fee_usd`, and `source_locator`.
4. `list_purchase_receipts`: GET `/v1/purchase-receipts`; read-only,
   idempotent, non-destructive, closed-world. Project `/receipts` to
   `receipts`, with at most two objects. Each object requires `receipt_id`,
   `purchase_id`, `original_charge_id`, `charged_amount_usd`,
   `item_amount_usd`, `shipping_amount_usd`, `payment_status`, `receipt_date`,
   and `source_locator`.
5. `list_shipping_options`: GET `/v1/shipping-options`; read-only,
   idempotent, non-destructive, closed-world. Project `/options` to `options`,
   with at most two objects. Each object requires `return_option_id`,
   `purchase_id`, `method`, `carrier`, `label_cost_usd`, `delivery_estimate`,
   `status`, and `source_locator`.
6. `list_return_history`: GET `/v1/return-history`; read-only, idempotent,
   non-destructive, closed-world. Project `/returns` to `returns`, with at
   most two objects. Each object requires `return_id`, `purchase_id`, `status`,
   and `source_locator`. An empty list is valid.
7. `submit_return`: POST `/v1/returns`; non-read-only, idempotent for an exact
   repeated body, non-destructive, open-world, approval-gated. Accept only
   `case_id`, `purchase_id`, `reason_code`, `return_option_id`,
   `original_charge_id`, `expected_refund_usd`, and `approval_note` as JSON
   body arguments. Bind each property to its same-named `$argument`. Required
   response fields are `return_id`, `purchase_id`, `original_charge_id`,
   `return_status`, `label_id`, `carrier`, `tracking_number`,
   `shipping_receipt_id`, `expected_refund_usd`, `submission_count`, and
   `source_locator`. The compiled write must use `retry: never`.
8. `get_return_refund_status`: GET `/v1/refund-status`; read-only, idempotent,
   non-destructive, closed-world. Required fields are `return_id`,
   `purchase_id`, `original_charge_id`, `return_status`, `label_id`, `carrier`,
   `tracking_number`, `shipping_receipt_id`, `expected_refund_usd`,
   `original_charge_amount_usd`, `refund_status`, `submission_count`, and
   `source_locator`.

Use bounded generated responses. IDs, dates, labels, and locators use a 128
byte maximum in the two write/status responses and a 96 byte maximum in the
other responses. Goal and decision boundary use a 256 byte maximum. Numbers
and booleans retain their JSON types. Use integer types for `window_days` and
`submission_count`.

## Exact synthetic write body

The valid fixture request is:

```json
{
  "case_id": "return-001",
  "purchase_id": "purchase-001",
  "reason_code": "faulty",
  "return_option_id": "return-option-001",
  "original_charge_id": "charge-001",
  "expected_refund_usd": 80,
  "approval_note": "Synthetic return only; no merchant contact, shipment, or payment."
}
```

## Acceptance criteria

- The corrected documentation page is opened before proposal work.
- One no-argument `adapter.definition_template` call is made.
- Exactly one proposal returns `review_required` with exactly the eight named
  operations and paths above.
- Every generated transform reads the stable field names above, not the
  descriptive aliases from the first attempt.
- All seven GET operations are automatic reads. Only `submit_return` is a
  write and it is approval-gated.
- Setup makes no `/v1` request and no state-changing request.
- The operator can accept the pending digest and configure automatic reads
  with always-ask writes.
