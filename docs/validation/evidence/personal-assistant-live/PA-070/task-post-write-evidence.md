# PA-070 post-write synthetic vehicle evidence

## Human approval

The human explicitly approved exactly one `book_vehicle_service` call with the authoritative body, while reiterating that the action must remain synthetic only with no dealer contact, registry action, real appointment, repair, inspection, renewal, charge, payment, or money movement.

## Final service-call sequence

The vehicle-service calls are exactly nine, in the required order:

1. `get_vehicle_profile` — `call_wWu1H7CZh5VxfQGayPVpLu4O`
2. `list_vehicle_service_history` — `call_xxl95fKIToHBDkgKA37VRees`
3. `list_vehicle_recalls` — `call_JN8zila2C0RgEtL34T06ylQA`
4. `get_vehicle_registration` — `call_L6LViskgDh1CPSCvoxDxqCuy`
5. `list_vehicle_service_options` — `call_zGfSLlqnE1XslvRSiHB3pmsw`
6. `get_vehicle_constraints` — `call_9tE0O7T5OiMHnvOxWHlFSzLh`
7. `book_vehicle_service` — `call_BK8UyBkMZaD69JUXodG3SZG4`
8. `get_vehicle_service_receipt` — `call_5we2yr5aIx8qDnhQSVnVHmoV`
9. `get_vehicle_status` — `call_eSh6VtaqStkRL3BxZKTGMUnj`

The POST was not repeated. Each initial read and each post-write read occurred once.

## Exact approved POST body

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

## POST response — complete returned payload

```json
{
  "estimated_total_usd": 325,
  "next_service_due_date": "2027-09-16",
  "next_service_mileage": 53620,
  "odometer_miles": 48620,
  "option_id": "option-001",
  "recall_campaign_id": "RC-2026-04",
  "registration_deadline": "2026-09-30",
  "registration_status": "ready_for_renewal",
  "service_date": "2026-09-16",
  "service_receipt_id": "service-receipt-001",
  "source_locator": "vehicle://service-receipts/service-receipt-001",
  "status": "booked_in_synthetic_record",
  "submission_count": 1,
  "vehicle_id": "vehicle-001"
}
```

## Receipt read — complete returned payload

```json
{
  "estimated_total_usd": 325,
  "next_service_due_date": "2027-09-16",
  "next_service_mileage": 53620,
  "odometer_miles": 48620,
  "option_id": "option-001",
  "recall_campaign_id": "RC-2026-04",
  "registration_deadline": "2026-09-30",
  "registration_status": "ready_for_renewal",
  "service_date": "2026-09-16",
  "service_receipt_id": "service-receipt-001",
  "source_locator": "vehicle://service-receipts/service-receipt-001",
  "status": "booked_in_synthetic_record",
  "submission_count": 1,
  "vehicle_id": "vehicle-001"
}
```

## Status read — complete returned payload

```json
{
  "current_mileage": 48620,
  "last_service_date": "2026-09-16",
  "last_service_mileage": 48620,
  "next_service_due_date": "2027-09-16",
  "next_service_mileage": 53620,
  "recall_campaign_id": "RC-2026-04",
  "recall_status": "completed_in_synthetic_record",
  "registration_deadline": "2026-09-30",
  "registration_status": "ready_for_renewal",
  "service_submission_count": 1,
  "source_locator": "vehicle://status/vehicle-lifecycle-001",
  "vehicle_id": "vehicle-001"
}
```

## Validation

Post-write Luau validation call `call_y2DRAQ3sfhhn04COOsnOQZBU` returned:

```json
{
  "all_pass": true,
  "booking_matches": true,
  "booking_receipt_equal": true,
  "interval_miles": 5000,
  "receipt_matches": true,
  "status_matches": true
}
```

The synthetic record therefore shows receipt `service-receipt-001`, status `booked_in_synthetic_record`, one submission, matching campaign `RC-2026-04` completed in the synthetic record, next service at 53,620 miles on September 16, 2027, registration status `ready_for_renewal`, amount 325 USD, receipt locator `vehicle://service-receipts/service-receipt-001`, and status locator `vehicle://status/vehicle-lifecycle-001`.

No real-world action occurred. This is not a real appointment, service, repair, inspection, registration renewal, charge, payment, or money movement.
