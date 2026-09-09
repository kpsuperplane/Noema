# PA-070 synthetic vehicle lifecycle evidence ledger

## Call sequence completed before approval

1. `get_vehicle_profile` — call `call_wWu1H7CZh5VxfQGayPVpLu4O`
2. `list_vehicle_service_history` — call `call_xxl95fKIToHBDkgKA37VRees`
3. `list_vehicle_recalls` — call `call_JN8zila2C0RgEtL34T06ylQA`
4. `get_vehicle_registration` — call `call_L6LViskgDh1CPSCvoxDxqCuy`
5. `list_vehicle_service_options` — call `call_zGfSLlqnE1XslvRSiHB3pmsw`
6. `get_vehicle_constraints` — call `call_9tE0O7T5OiMHnvOxWHlFSzLh`

Each initial source operation succeeded and was invoked exactly once, in the required order. No booking write or post-write read has occurred.

## 1. Vehicle profile — complete returned payload

```json
{
  "case_id": "vehicle-lifecycle-001",
  "current_date": "2026-09-09",
  "current_mileage": 48620,
  "decision_boundary": "Do not contact a real dealer or registry, expose a real VIN or address, schedule a real service, authorize a charge, or move money.",
  "goal": "Keep the synthetic vehicle service interval and registration paperwork current while matching the vehicle to the correct recall.",
  "owner_label": "Jordan Lee (synthetic)",
  "registration_deadline": "2026-09-30",
  "service_interval_miles": 5000,
  "source_locator": "vehicle://profiles/vehicle-lifecycle-001",
  "vehicle_id": "vehicle-001",
  "vehicle_label": "Orion Vale hatchback (synthetic)",
  "vehicle_serial": "SYNTH-VALE-001"
}
```

## 2. Service history — complete returned payload

```json
{
  "records": [
    {
      "mileage": 39210,
      "notes": "Synthetic historical inspection; no open recall work was recorded.",
      "service_date": "2025-07-18",
      "service_id": "service-001",
      "service_type": "annual_inspection",
      "source_locator": "vehicle://service-history/service-001",
      "vehicle_id": "vehicle-001",
      "vendor_label": "Orion Vale service center (synthetic)"
    },
    {
      "mileage": 43620,
      "notes": "Synthetic interval service completed; next mileage interval is 48620.",
      "service_date": "2026-04-08",
      "service_id": "service-002",
      "service_type": "scheduled_service",
      "source_locator": "vehicle://service-history/service-002",
      "vehicle_id": "vehicle-001",
      "vendor_label": "Orion Vale service center (synthetic)"
    }
  ]
}
```

## 3. Recall records — complete returned payload

```json
{
  "recalls": [
    {
      "campaign_id": "RC-2026-04",
      "matches_vehicle": true,
      "recall_id": "recall-001",
      "remedy": "Replace the synthetic brake-control module and record completion before registration renewal.",
      "serial_end": "SYNTH-VALE-099",
      "serial_prefix": "SYNTH-VALE",
      "serial_start": "SYNTH-VALE-000",
      "severity": "safety",
      "source_locator": "vehicle://recalls/recall-001",
      "status": "open",
      "vehicle_id": "vehicle-001",
      "vehicle_serial": "SYNTH-VALE-001"
    },
    {
      "campaign_id": "RC-2025-11",
      "matches_vehicle": false,
      "recall_id": "recall-002",
      "remedy": "Synthetic infotainment update for a different serial range.",
      "serial_end": "SYNTH-VALE-299",
      "serial_prefix": "SYNTH-VALE",
      "serial_start": "SYNTH-VALE-200",
      "severity": "service",
      "source_locator": "vehicle://recalls/recall-002",
      "status": "not_applicable",
      "vehicle_id": "vehicle-other-001",
      "vehicle_serial": "SYNTH-VALE-001"
    }
  ]
}
```

## 4. Registration — complete returned payload

```json
{
  "expires_on": "2026-09-30",
  "jurisdiction_label": "Synthetic jurisdiction",
  "open_recall_blocks_renewal": true,
  "registration_id": "registration-001",
  "required_documents": [
    "synthetic-insurance-proof",
    "synthetic-emissions-record"
  ],
  "source_locator": "vehicle://registration/registration-001",
  "status": "renewal_pending_service",
  "vehicle_id": "vehicle-001"
}
```

## 5. Service options — complete returned payload

```json
{
  "options": [
    {
      "duration_minutes": 150,
      "estimated_total_usd": 325,
      "included_services": [
        "scheduled_service",
        "RC-2026-04 brake-control remedy",
        "synthetic-emissions-record"
      ],
      "offered_date": "2026-09-16",
      "offered_start_time": "09:00",
      "option_id": "option-001",
      "option_status": "feasible",
      "recall_campaign_id": "RC-2026-04",
      "registration_ready": true,
      "source_locator": "vehicle://service-options/option-001",
      "vehicle_id": "vehicle-001",
      "vendor_label": "Orion Vale service center (synthetic)"
    },
    {
      "duration_minutes": 75,
      "estimated_total_usd": 210,
      "included_services": [
        "scheduled_service"
      ],
      "offered_date": "2026-09-12",
      "offered_start_time": "15:00",
      "option_id": "option-002",
      "option_status": "missing_recall_remedy",
      "recall_campaign_id": "none",
      "registration_ready": false,
      "source_locator": "vehicle://service-options/option-002",
      "vehicle_id": "vehicle-001",
      "vendor_label": "QuickLane service (synthetic)"
    },
    {
      "duration_minutes": 120,
      "estimated_total_usd": 290,
      "included_services": [
        "scheduled_service",
        "RC-2026-04 brake-control remedy"
      ],
      "offered_date": "2026-09-18",
      "offered_start_time": "10:00",
      "option_id": "option-003",
      "option_status": "wrong_vehicle",
      "recall_campaign_id": "RC-2026-04",
      "registration_ready": true,
      "source_locator": "vehicle://service-options/option-003",
      "vehicle_id": "vehicle-other-001",
      "vendor_label": "Orion Vale service center (synthetic)"
    }
  ]
}
```

## 6. Constraints — complete returned payload

```json
{
  "budget_limit_usd": 500,
  "current_mileage": 48620,
  "registration_deadline": "2026-09-30",
  "required_document": "synthetic-emissions-record",
  "required_recall_campaign": "RC-2026-04",
  "service_due_mileage": 48620,
  "service_window_end": "2026-09-29",
  "service_window_start": "2026-09-10",
  "source_locator": "vehicle://constraints/vehicle-lifecycle-001"
}
```

## Luau validation

`code.run_luau` call: `call_lXuYf1jvkvrG35hOrsW2JOsk`.

- Mileage: `43620 + 5000 = 48620`; current mileage and fixture due mileage are both `48620`; `due_now=true`.
- Date: selected date `2026-09-16` is within `2026-09-10` through `2026-09-29` and before registration deadline `2026-09-30`.
- Budget: selected cost `325` USD is within the `500` USD limit, leaving `175` USD.
- Recall `RC-2026-04`: serial in range, correct vehicle, open, accepted.
- Recall `RC-2025-11`: serial out of range and wrong vehicle, rejected.
- `option-001`: correct vehicle; includes scheduled service, `RC-2026-04` remedy, and `synthetic-emissions-record`; date and budget pass; accepted.
- `option-002`: rejected because it omits the required recall remedy (and emissions record; it is not registration-ready).
- `option-003`: rejected because it is for another vehicle (and omits the emissions record).

## Exact approval-gated synthetic booking body

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

## Continuation guard

- Do not repeat the six initial reads or the Luau validation.
- Await explicit human approval before the first and only `book_vehicle_service` call.
- If approved and the POST succeeds, call `get_vehicle_service_receipt` once, then `get_vehicle_status` once.
- The target service-call sequence is exactly nine service calls: the six recorded reads above, one POST, receipt read, status read. The Luau call is not a vehicle-service call.
