# PA-072 return the synthetic faulty item

Use the reviewed `synthetic_returns_api_fixed2` connection
(`7ae976b1253ad9b9a9419e79a094cfb8`) in the personal workspace. The service and every write are synthetic. Do not contact a real
merchant or carrier, ship an item, issue a refund, authorize a payment, or
move money.

## Source review

Read each source operation exactly once, in this order:

1. `get_return_profile`
2. `list_purchases`
3. `get_return_policy`
4. `list_purchase_receipts`
5. `list_shipping_options`
6. `list_return_history`

Keep every returned field and every `return://` source locator. Do not use
outside facts. The fixture case is `return-001`, and the target is the compact
wireless KeyNest keyboard. Use stable purchase IDs and `serial_number` for
identity. Do not mix records because the item names are equal.

## Plan and checks

Use `code.run_luau` for identity, date, eligibility, policy, charge, shipping,
amount, and exact write-body checks.

Select `purchase-001`: serial `SYNTH-KEY-001`, eligible through 2026-09-15,
faulty condition, not final sale. Match it to `charge-001` and the 80 USD
`item_amount_usd`. Select `return-option-001`: it belongs to purchase-001, has
status `eligible`, prepaid standard shipping, and zero label cost. Use reason
code `faulty`.

Reject `purchase-002`: serial `SYNTH-KEY-002`, working, final sale, and past its
2026-07-31 deadline. Reject `return-option-002`: it belongs to purchase-002 and
has status `purchase_not_eligible`.

Present the evidence and exact synthetic action body. Ask for human approval
before the write. After approval, call `submit_return` exactly once with:

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

This records one synthetic return label only. It is not a real return,
shipment, merchant contact, refund, payment, or money movement. After the POST
succeeds, call `get_return_refund_status` exactly once. Do not repeat any read
or write on continuation.

## Acceptance criteria

- The six source reads occur once each, in the listed order.
- Both same-name purchases remain distinct by stable ID and serial.
- The selected purchase, original charge, reason, and shipping option match the
  evidence above. The two invalid alternatives are rejected for the stated
  reasons.
- The POST is shown behind a human approval gate and runs once with the exact
  body above. The status read runs once after the POST.
- The fixture records exactly eight calls: six reads, one POST, and one status
  read, in that order. The POST submission count is one.
- The return has ID `return-record-001`, label `label-001`, carrier `Synthetic
  Parcel`, tracking `SYNTH-TRACK-001`, shipping receipt
  `shipping-receipt-001`, and expected refund 80 USD.
- Status reports original charge amount 80 USD, expected refund 80 USD,
  `refund_status: expected_after_synthetic_inspection`, and submission count
  one.
- No real merchant, carrier, shipment, refund, payment, or money movement is
  used. No shared workspace is created.
