# PA-072 synthetic source evidence

The clean execution read these six operations once, in this order. The values
below are the bounded responses returned through the reviewed connector. Each
source locator was retained.

## 1. `get_return_profile`

```json
{
  "case_id": "return-001",
  "owner": "Jordan Lee (synthetic)",
  "current_date": "2026-09-09",
  "goal": "Return the faulty keyboard and track the expected refund without mixing it with a similar purchase.",
  "decision_boundary": "Synthetic only; do not contact a merchant or carrier, send a parcel, authorize a payment, or move money.",
  "target_description": "KeyNest Compact Keyboard",
  "source_locator": "return://profiles/return-001"
}
```

## 2. `list_purchases`

```json
{
  "purchases": [
    {
      "purchase_id": "purchase-001",
      "item_name": "KeyNest Compact Keyboard",
      "item_variant": "compact wireless",
      "serial_number": "SYNTH-KEY-001",
      "purchase_date": "2026-08-25",
      "return_deadline": "2026-09-15",
      "condition": "faulty_intermittent_keys",
      "eligible": true,
      "final_sale": false,
      "seller": "Northstar Goods (synthetic)",
      "original_charge_id": "charge-001",
      "amount_usd": 80,
      "source_locator": "return://purchases/purchase-001"
    },
    {
      "purchase_id": "purchase-002",
      "item_name": "KeyNest Compact Keyboard",
      "item_variant": "compact wireless",
      "serial_number": "SYNTH-KEY-002",
      "purchase_date": "2026-07-01",
      "return_deadline": "2026-07-31",
      "condition": "working",
      "eligible": false,
      "final_sale": true,
      "seller": "Northstar Goods (synthetic)",
      "original_charge_id": "charge-002",
      "amount_usd": 80,
      "source_locator": "return://purchases/purchase-002"
    }
  ]
}
```

The records share a display name and variant. Their stable purchase IDs and
serial numbers keep them separate. Only `purchase-001` is eligible.

## 3. `get_return_policy`

```json
{
  "policy_id": "return-policy-001",
  "window_days": 21,
  "current_date": "2026-09-09",
  "allowed_reasons": "faulty,wrong_item,changed_mind",
  "shipping_required": true,
  "refund_basis": "original_item_charge",
  "restocking_fee_usd": 0,
  "source_locator": "return://policies/return-policy-001"
}
```

## 4. `list_purchase_receipts`

```json
{
  "receipts": [
    {
      "receipt_id": "receipt-001",
      "purchase_id": "purchase-001",
      "original_charge_id": "charge-001",
      "charged_amount_usd": 80,
      "item_amount_usd": 80,
      "shipping_amount_usd": 0,
      "payment_status": "charged_in_synthetic_record",
      "receipt_date": "2026-08-25",
      "source_locator": "return://receipts/receipt-001"
    },
    {
      "receipt_id": "receipt-002",
      "purchase_id": "purchase-002",
      "original_charge_id": "charge-002",
      "charged_amount_usd": 80,
      "item_amount_usd": 80,
      "shipping_amount_usd": 0,
      "payment_status": "charged_in_synthetic_record",
      "receipt_date": "2026-07-01",
      "source_locator": "return://receipts/receipt-002"
    }
  ]
}
```

## 5. `list_shipping_options`

```json
{
  "options": [
    {
      "return_option_id": "return-option-001",
      "purchase_id": "purchase-001",
      "method": "prepaid_standard_label",
      "carrier": "Synthetic Parcel",
      "delivery_estimate": "3-5 synthetic days",
      "status": "eligible",
      "label_cost_usd": 0,
      "source_locator": "return://shipping-options/return-option-001"
    },
    {
      "return_option_id": "return-option-002",
      "purchase_id": "purchase-002",
      "method": "prepaid_standard_label",
      "carrier": "Synthetic Parcel",
      "delivery_estimate": "3-5 synthetic days",
      "status": "purchase_not_eligible",
      "label_cost_usd": 0,
      "source_locator": "return://shipping-options/return-option-002"
    }
  ]
}
```

## 6. `list_return_history`

```json
{
  "returns": []
}
```

The empty history was read before the synthetic submission.
