# PA-072 final synthetic returns API setup

Set up one reviewed API connector for this synthetic fixture. This is a fresh
proposal after an earlier invalid proposal. Do not use either earlier
definition. Do not call a service route, contact a merchant or carrier, ship an
item, issue a refund, authorize a payment, or move money.

## Source

- `adapter_id`: `synthetic_returns_api_final`
- `definition_id`: `definition:synthetic_returns_api_final`
- `display_name`: `Synthetic returns API final`
- `definition_revision`: `v3`
- `origin`: `https://materials-huntington-henderson-collective.trycloudflare.com/`
- `source_reference`: `https://materials-huntington-henderson-collective.trycloudflare.com/docs`
- `authentication`: none
- fixture: `2026-09-09-returns-api-v1`

Open the documentation URL first. Then call `adapter.definition_template` once
with no arguments. Use its schema for exactly one `adapter.propose_definition`
call. Do not retry either adapter call. Do not include `retry` or
`accepted_content_types` in the proposal. Use `pagination: {kind: "none"}` on
all operations. The server derives safe-read retries for GETs and `never` for
the POST.

## Exact operation contract

Propose exactly these eight operations, with no others. Use the exact response
field names and JSON types below. `allowed_reasons` is one comma-delimited
string, not a string array. Ignore descriptive response aliases such as
`owner_label`, `item_serial`, `carrier_label`, and `option_status`.

| Operation | Request | Response |
| --- | --- | --- |
| `get_return_profile` | GET `/v1/profile`; read-only, idempotent, non-destructive, closed-world | flat object: `case_id`, `owner`, `current_date`, `goal`, `decision_boundary`, `target_description`, `source_locator` (strings; goal and boundary max 256 bytes, others max 96) |
| `list_purchases` | GET `/v1/purchases`; read-only, idempotent, non-destructive, closed-world | object list `/purchases` to `purchases`, max 2; each item: `purchase_id`, `item_name`, `item_variant`, `serial_number`, `purchase_date`, `return_deadline`, `condition`, `seller`, `source_locator` (strings max 96), `eligible`, `final_sale` (booleans), `original_charge_id` (string max 96), `amount_usd` (number) |
| `get_return_policy` | GET `/v1/return-policy`; read-only, idempotent, non-destructive, closed-world | flat object: `policy_id`, `current_date`, `allowed_reasons`, `refund_basis`, `source_locator` (strings max 96), `window_days` (integer), `shipping_required` (boolean), `restocking_fee_usd` (number) |
| `list_purchase_receipts` | GET `/v1/purchase-receipts`; read-only, idempotent, non-destructive, closed-world | object list `/receipts` to `receipts`, max 2; each item: `receipt_id`, `purchase_id`, `original_charge_id`, `payment_status`, `receipt_date`, `source_locator` (strings max 96), `charged_amount_usd`, `item_amount_usd`, `shipping_amount_usd` (numbers) |
| `list_shipping_options` | GET `/v1/shipping-options`; read-only, idempotent, non-destructive, closed-world | object list `/options` to `options`, max 2; each item: `return_option_id`, `purchase_id`, `method`, `carrier`, `delivery_estimate`, `status`, `source_locator` (strings max 96), `label_cost_usd` (number) |
| `list_return_history` | GET `/v1/return-history`; read-only, idempotent, non-destructive, closed-world | object list `/returns` to `returns`, max 2; each item: `return_id`, `purchase_id`, `status`, `source_locator` (strings max 96). Empty list is valid. |
| `submit_return` | POST `/v1/returns`; non-read-only, idempotent for exact body, non-destructive, open-world, approval-gated | JSON body arguments only: `case_id`, `purchase_id`, `reason_code`, `return_option_id`, `original_charge_id`, `expected_refund_usd`, `approval_note`; flat response: `return_id`, `purchase_id`, `original_charge_id`, `return_status`, `label_id`, `carrier`, `tracking_number`, `shipping_receipt_id`, `source_locator` (strings max 128), `expected_refund_usd` (number), `submission_count` (integer) |
| `get_return_refund_status` | GET `/v1/refund-status`; read-only, idempotent, non-destructive, closed-world | flat object: `return_id`, `purchase_id`, `original_charge_id`, `return_status`, `label_id`, `carrier`, `tracking_number`, `shipping_receipt_id`, `refund_status`, `source_locator` (strings max 128), `expected_refund_usd`, `original_charge_amount_usd` (numbers), `submission_count` (integer) |

Every listed field is required. Bind each POST property to its same-named
`$argument`. Use no fixed headers or queries. The valid synthetic POST body is:

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

- The docs page is opened before the one template call and one proposal call.
- The proposal returns `review_required` and has exactly the eight operations.
- The generated `allowed_reasons` field has type `string`, not `string_array`.
- All seven GETs are automatic reads. Only the POST is a write and it is
  approval-gated with compiled `retry: never`.
- No `/v1` route or state-changing request occurs during setup.
