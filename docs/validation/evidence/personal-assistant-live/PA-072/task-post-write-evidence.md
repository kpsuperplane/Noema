# PA-072 post-write synthetic evidence

## Human approval

The operator approved one exact `submit_return` call after the six source
reads and the bounded Lua check.

- Task gate: `gate:6002d1e11b3a6f8255396a0534ffbd47`
- Governed action: `action:1a57c2955c6d37cb74abfa6607a604df`, revision 1
- Action state: `SUCCEEDED`
- Provider call: `call_fEONBFbMyC8ERT43ofT9Ou5f`

The action remained inside the synthetic fixture. It did not contact a
merchant or carrier, ship an item, issue a refund, authorize a payment, or
move money.

## Exact approved body

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

The fixture accepted the object by field name. It returned
`submission_count: 1`, so the synthetic state-changing operation ran once.

## POST response

```json
{
  "return_id": "return-record-001",
  "purchase_id": "purchase-001",
  "original_charge_id": "charge-001",
  "return_status": "label_created_in_synthetic_record",
  "label_id": "label-001",
  "carrier": "Synthetic Parcel",
  "tracking_number": "SYNTH-TRACK-001",
  "shipping_receipt_id": "shipping-receipt-001",
  "expected_refund_usd": 80,
  "submission_count": 1,
  "source_locator": "return://returns/return-record-001"
}
```

## Status read

The one permitted `get_return_refund_status` call used provider call ID
`call_5sO59qd2eCpeqcz50XpYASoZ`. It returned:

```json
{
  "return_id": "return-record-001",
  "purchase_id": "purchase-001",
  "original_charge_id": "charge-001",
  "return_status": "label_created_in_synthetic_record",
  "label_id": "label-001",
  "carrier": "Synthetic Parcel",
  "tracking_number": "SYNTH-TRACK-001",
  "shipping_receipt_id": "shipping-receipt-001",
  "expected_refund_usd": 80,
  "original_charge_amount_usd": 80,
  "refund_status": "expected_after_synthetic_inspection",
  "submission_count": 1,
  "source_locator": "return://refund-status/return-record-001"
}
```

The status says that an 80 USD refund is expected after synthetic inspection.
It does not say that a real refund was issued.

## Final boundary

The fixture process recorded six reads, this one POST, and this one status
read. It recorded no duplicate submission. The temporary fixture and tunnel
were stopped after the ledger was captured. No real external system was used.
