# PA-072 return the synthetic faulty item

Use the reviewed `synthetic_returns_api_v1` connection in the personal
workspace. The service and every write are synthetic. Do not contact a real
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
wireless KeyNest keyboard. Use stable purchase IDs and serials for identity.

## Plan and checks

Use `code.run_luau` for identity, date, eligibility, policy, charge, shipping,
amount, and write-body checks. Keep the two purchases separate even though
they have the same display name and variant.

Select `purchase-001` because it has serial `SYNTH-KEY-001`, is eligible until
2026-09-15, is faulty, and is not final sale. Match it to `charge-001` and the
80 USD original item charge. Select `return-option-001` because it belongs to
that purchase, is eligible, is prepaid standard shipping, and costs zero.
Use reason code `faulty`.

Reject `purchase-002`: it has a different serial, is past its 2026-07-31
deadline, is final sale, and is working. Reject `return-option-002` because it
belongs to the rejected purchase and is not eligible.

Present the findings and the exact synthetic action body. Ask for human
approval before the write. After approval, call `submit_return` exactly once
with these exact arguments:

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

The POST records one synthetic return label only. It must never be described
as a real return, shipment, merchant contact, refund, payment, or money
movement. After the POST succeeds, call `get_return_refund_status` exactly
once. Do not call the POST or any source operation again on continuation.

## Acceptance criteria

- The six source reads occur once each, in the listed order.
- Both same-name purchases remain distinct by stable ID and serial.
- `purchase-001`, `charge-001`, reason `faulty`, and
  `return-option-001` are selected for the stated evidence.
- `purchase-002` and `return-option-002` are rejected for the stated reasons.
- The POST is shown behind a human approval gate and runs once with the exact
  body above.
- The status read runs once after the POST.
- The fixture records exactly eight service calls: six reads, one POST, and
  one status read, in that order. The POST submission count is one.
- The return record contains `return-record-001`, label `label-001`, carrier
  `Synthetic Parcel`, tracking `SYNTH-TRACK-001`, shipping receipt
  `shipping-receipt-001`, and expected refund 80 USD.
- The status record reports original charge amount 80 USD, expected refund
  80 USD, `refund_status: expected_after_synthetic_inspection`, and submission
  count one.
- No real merchant, carrier, shipment, refund, payment, or money movement is
  used. No shared workspace is created.
