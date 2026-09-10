# PA-072 final fixed connector setup

Create a fresh connector from the synthetic returns API documentation. The
origin is the URL root with a trailing slash. The source reference is the
`/docs` URL. Do not use earlier returns definitions or call any `/v1` route.
No real merchant, carrier, shipment, refund, payment, or money action is
allowed.

## Source and sequence

- adapter ID: `synthetic_returns_api_fixed2`
- definition ID: `definition:synthetic_returns_api_fixed2`
- display name: `Synthetic returns API fixed 2`
- definition revision: `v5`
- origin: `https://materials-huntington-henderson-collective.trycloudflare.com/`
- source reference: `https://materials-huntington-henderson-collective.trycloudflare.com/docs`
- authentication: none
- fixture version: `2026-09-09-returns-api-v1`

Open the source reference first. Call `adapter.definition_template` once with
no arguments. Then call `adapter.propose_definition` exactly once. Do not
retry either call. Omit `retry`, `accepted_content_types`, fixed headers, and
fixed queries. Set `pagination: {kind: "none"}` on every operation.

## Operations

Propose exactly these eight operations. All response fields are required. Use
the exact names and types. `allowed_reasons` is one comma-delimited string,
not a string array. The fixture includes descriptive aliases such as
`owner_label`, `item_serial`, `carrier_label`, and `option_status`; ignore
those aliases.

| ID | Contract |
| --- | --- |
| `get_return_profile` | GET `/v1/profile`; read-only, idempotent, non-destructive, closed-world; flat strings `case_id`, `owner`, `current_date`, `target_description`, `source_locator` (max 96), `goal`, `decision_boundary` (max 256). |
| `list_purchases` | GET `/v1/purchases`; read-only, idempotent, non-destructive, closed-world; object list `/purchases` to `purchases`, max 2; each object strings `purchase_id`, `item_name`, `item_variant`, `serial_number`, `purchase_date`, `return_deadline`, `condition`, `seller`, `original_charge_id`, `source_locator` (max 96), booleans `eligible`, `final_sale`, number `amount_usd`. |
| `get_return_policy` | GET `/v1/return-policy`; read-only, idempotent, non-destructive, closed-world; flat strings `policy_id`, `current_date`, `allowed_reasons`, `refund_basis`, `source_locator` (max 96), integer `window_days`, boolean `shipping_required`, number `restocking_fee_usd`. |
| `list_purchase_receipts` | GET `/v1/purchase-receipts`; read-only, idempotent, non-destructive, closed-world; object list `/receipts` to `receipts`, max 2; each object strings `receipt_id`, `purchase_id`, `original_charge_id`, `payment_status`, `receipt_date`, `source_locator` (max 96), numbers `charged_amount_usd`, `item_amount_usd`, `shipping_amount_usd`. |
| `list_shipping_options` | GET `/v1/shipping-options`; read-only, idempotent, non-destructive, closed-world; object list `/options` to `options`, max 2; each object strings `return_option_id`, `purchase_id`, `method`, `carrier`, `delivery_estimate`, `status`, `source_locator` (max 96), number `label_cost_usd`. |
| `list_return_history` | GET `/v1/return-history`; read-only, idempotent, non-destructive, closed-world; object list `/returns` to `returns`, max 2; each object strings `return_id`, `purchase_id`, `status`, `source_locator` (max 96). Empty list is valid. |
| `submit_return` | POST `/v1/returns`; non-read-only, idempotent for an exact repeated body, non-destructive, open-world, approval-gated; JSON-body arguments `case_id`, `purchase_id`, `reason_code`, `return_option_id`, `original_charge_id`, `expected_refund_usd` (number), `approval_note`, each bound to its same-named `$argument`; flat response strings `return_id`, `purchase_id`, `original_charge_id`, `return_status`, `label_id`, `carrier`, `tracking_number`, `shipping_receipt_id`, `source_locator` (max 128), number `expected_refund_usd`, integer `submission_count`. |
| `get_return_refund_status` | GET `/v1/refund-status`; read-only, idempotent, non-destructive, closed-world; flat strings `return_id`, `purchase_id`, `original_charge_id`, `return_status`, `label_id`, `carrier`, `tracking_number`, `shipping_receipt_id`, `refund_status`, `source_locator` (max 128), numbers `expected_refund_usd`, `original_charge_amount_usd`, integer `submission_count`. |

The valid synthetic POST body is:

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

- The source page is opened before the one template and one proposal call.
- The proposal returns `review_required` with exactly eight operations.
- `eligible` and `final_sale` use generated boolean transforms that preserve
  both true and false values.
- All seven GETs compile as automatic reads with `retry: transport_safe_read`.
  The POST is approval-gated with `retry: never`.
- No `/v1` request or state-changing request occurs during setup.
