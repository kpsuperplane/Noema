# PA-071 synthetic pet-care source evidence

The clean retry task read these seven operations once, in this order. The
payloads below are the bounded values returned through the reviewed Noema
connector. Every source locator was retained.

## 1. `get_pet_profile`

```json
{
  "case_id": "pet-care-001",
  "owner_label": "Jordan Lee (synthetic)",
  "current_date": "2026-09-09",
  "goal": "Organize upcoming care for two synthetic pets while keeping their identities, vaccine needs, and medication instructions separate.",
  "decision_boundary": "Do not contact a real clinic, boarding facility, travel provider, or pharmacy; do not change a medication dose or move money.",
  "travel_start": "2026-09-20",
  "travel_end": "2026-09-24",
  "boarding_pet_id": "pet-001",
  "source_locator": "pet://profiles/pet-care-001"
}
```

## 2. `list_pets`

```json
{
  "pets": [
    {
      "pet_id": "pet-001",
      "name": "Milo",
      "species": "dog",
      "sex": "male",
      "breed": "mixed breed (synthetic)",
      "birth_date": "2021-04-15",
      "microchip_id": "SYNTH-MILO-001",
      "source_locator": "pet://pets/pet-001"
    },
    {
      "pet_id": "pet-002",
      "name": "Milo",
      "species": "cat",
      "sex": "male",
      "breed": "domestic shorthair (synthetic)",
      "birth_date": "2022-08-03",
      "microchip_id": "SYNTH-MILO-002",
      "source_locator": "pet://pets/pet-002"
    }
  ]
}
```

## 3. `get_vet_plans`

```json
{
  "plans": [
    {
      "pet_id": "pet-001",
      "pet_name": "Milo",
      "species": "dog",
      "exam_due_date": "2026-09-16",
      "vaccination_status": "rabies expires 2026-09-18; bordetella due 2026-09-18",
      "rabies_valid_through": "2026-09-18",
      "bordetella_due_on": "2026-09-18",
      "care_instructions": "Bring the synthetic vaccination record. Keep the existing medication dose unchanged.",
      "source_locator": "pet://vet-plans/pet-001"
    },
    {
      "pet_id": "pet-002",
      "pet_name": "Milo",
      "species": "cat",
      "exam_due_date": "2026-09-17",
      "vaccination_status": "current; no boarding vaccines required",
      "rabies_valid_through": "2027-08-03",
      "bordetella_due_on": "not_applicable",
      "care_instructions": "Keep the existing care plan and medication dose unchanged.",
      "source_locator": "pet://vet-plans/pet-002"
    }
  ]
}
```

## 4. `list_pet_refills`

```json
{
  "refills": [
    {
      "refill_id": "refill-001",
      "pet_id": "pet-001",
      "pet_name": "Milo",
      "medication_id": "med-001",
      "medication_name": "Canidryl (synthetic)",
      "dose_instruction": "Give 25 mg by mouth twice daily with food; do not change dose.",
      "quantity": 20,
      "due_date": "2026-09-15",
      "refill_status": "due_before_boarding",
      "source_locator": "pet://refills/refill-001"
    },
    {
      "refill_id": "refill-002",
      "pet_id": "pet-002",
      "pet_name": "Milo",
      "medication_id": "med-002",
      "medication_name": "FeliCalm (synthetic)",
      "dose_instruction": "Give 2.5 mg by mouth once daily; do not change dose.",
      "quantity": 30,
      "due_date": "2026-09-22",
      "refill_status": "not_due_before_travel",
      "source_locator": "pet://refills/refill-002"
    }
  ]
}
```

## 5. `get_boarding_requirements`

```json
{
  "boarding_id": "boarding-001",
  "pet_id": "pet-001",
  "facility_label": "Pine Hollow boarding (synthetic)",
  "boarding_start": "2026-09-20",
  "boarding_end": "2026-09-24",
  "intake_deadline": "2026-09-18",
  "required_vaccinations": "rabies,bordetella",
  "requirement_note": "Both required vaccines must be current before synthetic boarding intake.",
  "source_locator": "pet://boarding/boarding-001"
}
```

## 6. `get_pet_travel_plan`

```json
{
  "travelers": [
    {
      "pet_id": "pet-001",
      "pet_name": "Milo",
      "arrangement": "boarding",
      "travel_start": "2026-09-20",
      "travel_end": "2026-09-24",
      "source_locator": "pet://travel/pet-001"
    },
    {
      "pet_id": "pet-002",
      "pet_name": "Milo",
      "arrangement": "stays_home",
      "travel_start": "2026-09-20",
      "travel_end": "2026-09-24",
      "source_locator": "pet://travel/pet-002"
    }
  ]
}
```

## 7. `list_pet_visit_options`

```json
{
  "options": [
    {
      "option_id": "option-001",
      "pet_id": "pet-001",
      "pet_name": "Milo",
      "service_type": "boarding_vaccination",
      "date": "2026-09-14",
      "start_time": "10:00",
      "included_services": "rabies_booster,bordetella,annual_exam",
      "status": "feasible",
      "required_by": "2026-09-18",
      "estimated_total_usd": 185,
      "source_locator": "pet://visit-options/option-001"
    },
    {
      "option_id": "option-002",
      "pet_id": "pet-001",
      "pet_name": "Milo",
      "service_type": "boarding_vaccination",
      "date": "2026-09-19",
      "start_time": "09:00",
      "included_services": "rabies_booster,bordetella",
      "status": "too_late_for_boarding",
      "required_by": "2026-09-18",
      "estimated_total_usd": 160,
      "source_locator": "pet://visit-options/option-002"
    },
    {
      "option_id": "option-003",
      "pet_id": "pet-002",
      "pet_name": "Milo",
      "service_type": "annual_wellness_exam",
      "date": "2026-09-16",
      "start_time": "11:00",
      "included_services": "annual_exam",
      "status": "feasible",
      "required_by": "2026-09-17",
      "estimated_total_usd": 95,
      "source_locator": "pet://visit-options/option-003"
    },
    {
      "option_id": "option-004",
      "pet_id": "pet-002",
      "pet_name": "Milo",
      "service_type": "boarding_vaccination",
      "date": "2026-09-14",
      "start_time": "11:00",
      "included_services": "rabies_booster,bordetella",
      "status": "not_required_for_this_pet",
      "required_by": "not_applicable",
      "estimated_total_usd": 140,
      "source_locator": "pet://visit-options/option-004"
    }
  ]
}
```

The failed corrective attempt before this retry saw the same source routes but
did not write. The fixture was restarted before the payloads above were
recorded, so this is the clean nine-call ledger used for acceptance.
