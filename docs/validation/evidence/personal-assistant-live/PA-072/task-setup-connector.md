# PA-072 set up the synthetic returns API

Use the live Go Noema development instance to inspect the exact public
documentation below and propose one API connector. This setup is synthetic.
Do not call a service route, contact a merchant or carrier, ship an item,
issue a refund, authorize a payment, or move money.

## Documentation source

- Adapter ID: `synthetic_returns_api_v1`
- Definition ID: `definition:synthetic_returns_api_v1`
- Display name: `Synthetic returns API`
- Origin: `https://camping-bowl-contractor-residential.trycloudflare.com/`
- Source reference: `https://camping-bowl-contractor-residential.trycloudflare.com/docs`
- Revision: `v1`
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

## Required operations

The proposal must contain exactly these eight operations and no others:

1. `get_return_profile`: GET `/v1/profile`; read-only, idempotent,
   non-destructive, closed-world. Preserve case ID, owner, current date,
   goal, decision boundary, target description, and source locator.
2. `list_purchases`: GET `/v1/purchases`; read-only, idempotent,
   non-destructive, closed-world. Project `/purchases` as an object list with
   at most two records. Preserve every purchase ID, item name and variant,
   serial, purchase date, return deadline, condition, eligibility, final-sale
   flag, seller, original charge ID, amount, and locator.
3. `get_return_policy`: GET `/v1/return-policy`; read-only, idempotent,
   non-destructive, closed-world. Preserve policy ID, window, current date,
   allowed reasons, shipping requirement, refund basis, restocking fee, and
   locator.
4. `list_purchase_receipts`: GET `/v1/purchase-receipts`; read-only,
   idempotent, non-destructive, closed-world. Project `/receipts` with at
   most two records. Preserve receipt ID, purchase ID, original charge ID,
   charged, item, and shipping amounts, payment status, receipt date, and
   locator.
5. `list_shipping_options`: GET `/v1/shipping-options`; read-only,
   idempotent, non-destructive, closed-world. Project `/options` with at most
   two records. Preserve return option ID, purchase ID, method, carrier,
   label cost, delivery estimate, status, and locator.
6. `list_return_history`: GET `/v1/return-history`; read-only, idempotent,
   non-destructive, closed-world. Project `/returns` with at most two records.
   Preserve any returned return ID, purchase ID, status, and locator. An empty
   list is valid for this fixture.
7. `submit_return`: POST `/v1/returns`; non-read-only, idempotent for an exact
   repeated body, non-destructive, open-world, approval-gated. Accept only
   `case_id`, `purchase_id`, `reason_code`, `return_option_id`,
   `original_charge_id`, `expected_refund_usd`, and `approval_note` as JSON
   body arguments. Bind each body property to its same-named `$argument`.
   Preserve the returned return ID, purchase ID, charge ID, return status,
   label ID, carrier, tracking number, shipping receipt ID, expected refund,
   submission count, and locator. Do not add a retry field; the compiled write
   must use `retry: never`.
8. `get_return_refund_status`: GET `/v1/refund-status`; read-only,
   idempotent, non-destructive, closed-world. Preserve return ID, purchase ID,
   charge ID, return status, label ID, carrier, tracking number, shipping
   receipt ID, expected refund, original charge amount, refund status,
   submission count, and locator.

Use bounded generated responses. Every listed field is required. String fields
use these bounds:

| Operation | Response recipe and bounds |
| --- | --- |
| `get_return_profile` | `flat_object`; IDs, dates, owner, target, and locator max 96; goal and decision boundary max 256. |
| `list_purchases` | `object_list` from `/purchases`, output `purchases`, `max_items: 2`; IDs, names, variants, serials, dates, condition, seller, and locator max 96; booleans preserve eligibility/final-sale flags; amounts are numbers. |
| `get_return_policy` | `flat_object`; policy ID, current date, allowed reasons, refund basis, and locator max 96; integer window; boolean shipping requirement; number restocking fee. |
| `list_purchase_receipts` | `object_list` from `/receipts`, output `receipts`, `max_items: 2`; IDs, dates, status, and locator max 96; amounts are numbers. |
| `list_shipping_options` | `object_list` from `/options`, output `options`, `max_items: 2`; IDs, purchase IDs, method, carrier, estimate, status, and locator max 96; label cost is a number. |
| `list_return_history` | `object_list` from `/returns`, output `returns`, `max_items: 2`; IDs, purchase IDs, status, and locator max 96. Empty output is valid. |
| `submit_return` | `flat_object`; IDs, statuses, carrier, tracking, shipping receipt, and locator max 128; expected refund and submission count are numbers/integer. |
| `get_return_refund_status` | `flat_object`; IDs, statuses, carrier, tracking, shipping receipt, and locator max 128; expected refund, original charge amount, and submission count are numbers/integer. |

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

- The documentation page is opened before proposal work.
- One no-argument `adapter.definition_template` call is made.
- Exactly one proposal returns `review_required` with exactly the eight named
  operations and paths above.
- All seven GET operations are automatic reads. Only `submit_return` is a
  write and it is approval-gated.
- Setup makes no `/v1` request and no state-changing request.
- The operator can accept the pending digest and configure automatic reads
  with always-ask writes.
