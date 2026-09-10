# PA-072 fixed synthetic returns API setup

Create one fresh API connector from the synthetic returns documentation. This
proposal follows the earlier response-transform fix that preserves JSON
`false` values. Do not use an earlier returns definition. Do not call a service
route or make any real merchant, carrier, shipment, refund, payment, or money
action.

## Documentation and setup sequence

- adapter ID: `synthetic_returns_api_fixed`
- definition ID: `definition:synthetic_returns_api_fixed`
- display name: `Synthetic returns API fixed`
- revision: `v4`
- origin and source reference:
  `https://materials-huntington-henderson-collective.trycloudflare.com/docs`
- authentication: none
- fixture version: `2026-09-09-returns-api-v1`

Open the documentation URL first. Then call `adapter.definition_template` once
with no arguments. Use its response to make exactly one
`adapter.propose_definition` call. Do not retry either call. Do not include
`retry`, `accepted_content_types`, fixed headers, or fixed queries. Use
`pagination: {kind: "none"}` for every operation.

## Exact eight operations

Propose exactly these operations and no others. All listed response fields are
required. Use the exact JSON types shown. `allowed_reasons` is a comma-
delimited string, not an array. Ignore descriptive aliases such as
`owner_label`, `item_serial`, `carrier_label`, and `option_status`.

| ID | Request and response contract |
| --- | --- |
| `get_return_profile` | GET `/v1/profile`; read-only, idempotent, non-destructive, closed-world; flat fields `case_id`, `owner`, `current_date`, `goal`, `decision_boundary`, `target_description`, `source_locator` (strings; goal and boundary max 256 bytes, other strings max 96). |
| `list_purchases` | GET `/v1/purchases`; read-only, idempotent, non-destructive, closed-world; object list `/purchases` to `purchases`, max 2; each item has strings `purchase_id`, `item_name`, `item_variant`, `serial_number`, `purchase_date`, `return_deadline`, `condition`, `seller`, `original_charge_id`, `source_locator` (max 96), booleans `eligible`, `final_sale`, and number `amount_usd`. |
| `get_return_policy` | GET `/v1/return-policy`; read-only, idempotent, non-destructive, closed-world; flat fields `policy_id`, `current_date`, `allowed_reasons`, `refund_basis`, `source_locator` (strings max 96), integer `window_days`, boolean `shipping_required`, number `restocking_fee_usd`. |
| `list_purchase_receipts` | GET `/v1/purchase-receipts`; read-only, idempotent, non-destructive, closed-world; object list `/receipts` to `receipts`, max 2; each item has strings `receipt_id`, `purchase_id`, `original_charge_id`, `payment_status`, `receipt_date`, `source_locator` (max 96), and numbers `charged_amount_usd`, `item_amount_usd`, `shipping_amount_usd`. |
| `list_shipping_options` | GET `/v1/shipping-options`; read-only, idempotent, non-destructive, closed-world; object list `/options` to `options`, max 2; each item has strings `return_option_id`, `purchase_id`, `method`, `carrier`, `delivery_estimate`, `status`, `source_locator` (max 96), and number `label_cost_usd`. |
| `list_return_history` | GET `/v1/return-history`; read-only, idempotent, non-destructive, closed-world; object list `/returns` to `returns`, max 2; each item has strings `return_id`, `purchase_id`, `status`, `source_locator` (max 96). Empty list is valid. |
| `submit_return` | POST `/v1/returns`; non-read-only, idempotent for an exact repeated body, non-destructive, open-world, approval-gated; seven required JSON-body arguments `case_id`, `purchase_id`, `reason_code`, `return_option_id`, `original_charge_id`, `expected_refund_usd` (number), and `approval_note`; bind each property to its same-named `$argument`; flat response strings `return_id`, `purchase_id`, `original_charge_id`, `return_status`, `label_id`, `carrier`, `tracking_number`, `shipping_receipt_id`, `source_locator` (max 128), number `expected_refund_usd`, integer `submission_count`. |
| `get_return_refund_status` | GET `/v1/refund-status`; read-only, idempotent, non-destructive, closed-world; flat response strings `return_id`, `purchase_id`, `original_charge_id`, `return_status`, `label_id`, `carrier`, `tracking_number`, `shipping_receipt_id`, `refund_status`, `source_locator` (max 128), numbers `expected_refund_usd`, `original_charge_amount_usd`, integer `submission_count`. |

The valid synthetic write body is:

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

## Acceptance

- Documentation is opened before one template call and one proposal call.
- The proposal returns `review_required` with exactly eight operations.
- The generated `eligible` and `final_sale` fields use boolean transforms that
  preserve both `true` and `false` values.
- All seven GETs are automatic reads with compiler-derived
  `retry: transport_safe_read`. Only the POST is an approval-gated write with
  compiler-derived `retry: never`.
- Setup makes no `/v1` request or state-changing request.
