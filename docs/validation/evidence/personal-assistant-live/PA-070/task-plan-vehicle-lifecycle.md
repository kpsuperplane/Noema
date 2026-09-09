# PA-070 plan the synthetic vehicle lifecycle

Use the reviewed `synthetic_vehicle_lifecycle_api_v1` connection in the
personal workspace. The service and every write are synthetic. Do not contact
a real dealer, registry, insurer, lender, repair shop, or vehicle owner. Do
not schedule real service, renew registration, authorize a charge, make a
payment, or move money.

## Source review

Read each initial source operation exactly once, in this order:

1. `get_vehicle_profile`
2. `list_vehicle_service_history`
3. `list_vehicle_recalls`
4. `get_vehicle_registration`
5. `list_vehicle_service_options`
6. `get_vehicle_constraints`

Keep every returned field and every `vehicle://` source locator. Do not use
outside facts. The fixture case is `vehicle-lifecycle-001`, the vehicle is
`vehicle-001`, and its synthetic serial is `SYNTH-VALE-001`.

## Plan and calculation

Use `code.run_luau` for the mileage, date, budget, recall, and option checks.
The latest service was at 43,620 miles with a 5,000-mile interval. The
current mileage is 48,620, so service is due now. Match the vehicle serial to
the recall range and select campaign `RC-2026-04`; reject the nonmatching
recall. Keep the registration deadline of 2026-09-30 and the open-recall
requirement.

Select `option-001` only. It is for `vehicle-001`, is on 2026-09-16, includes
the scheduled service, recall work, and emissions check, costs 325 USD, and
meets the 500 USD limit before the registration deadline. Reject `option-002`
because it omits the recall, and reject `option-003` because it is for another
vehicle.

Present the findings and the exact synthetic booking body. Ask for human
approval before the first write. After approval, call
`book_vehicle_service` exactly once with these exact arguments:

```json
{
  "vehicle_id": "vehicle-001",
  "option_id": "option-001",
  "service_date": "2026-09-16",
  "odometer_miles": 48620,
  "recall_campaign_id": "RC-2026-04",
  "registration_deadline": "2026-09-30",
  "estimated_total_usd": 325,
  "approval_note": "Synthetic service booking only; no dealer contact or payment."
}
```

The POST records a synthetic booking only. It must never be described as a
real appointment, repair, inspection, registration, or payment. After the POST
succeeds, call `get_vehicle_service_receipt` exactly once and then
`get_vehicle_status` exactly once. Confirm receipt
`service-receipt-001`, status `booked_in_synthetic_record`, submission count
one, the completed recall, next service at 53,620 miles, next service date
2027-09-16, registration status `ready_for_renewal`, and
`vehicle://service-receipts/service-receipt-001`.

## Acceptance criteria

- The six initial reads occur once each, in the listed order.
- All profile, history, recall, registration, option, constraint, and source
  locator fields remain available for the decision.
- The due-mileage calculation is 43,620 + 5,000 = 48,620 miles.
- The matching recall is `RC-2026-04`; the nonmatching recall is rejected.
- `option-001` is selected for its vehicle, included work, date, and 325 USD
  price. The other two options are rejected for the stated reasons.
- The booking POST is shown behind a human approval gate and runs once with
  the exact body above.
- The receipt and status reads each run once after the POST.
- The fixture records exactly nine service calls: six reads, one POST, and
  two post-write reads, in that order.
- The returned receipt has one submission and the expected next interval,
  recall, registration status, amount, and locator.
- Repeating the task does not repeat the POST or any source read. Existing
  receipt and status locators remain usable on continuation.
- No real vehicle, dealer, registry, insurer, appointment, repair, charge,
  payment, registration, or money movement is used. No shared workspace is
  created.
