# PA-071 organize synthetic pet care

Use the reviewed `synthetic_pet_care_api_v1` connection in the personal
workspace. The service and every write are synthetic. Do not contact a real
clinic, boarding facility, travel provider, pharmacy, or pet owner. Do not
schedule real care, request a prescription, change a medication dose, charge a
card, or move money.

## Source review

Read each initial source operation exactly once, in this order:

1. `get_pet_profile`
2. `list_pets`
3. `get_vet_plans`
4. `list_pet_refills`
5. `get_boarding_requirements`
6. `get_pet_travel_plan`
7. `list_pet_visit_options`

Keep every returned field and every `pet://` source locator. Do not use
outside facts. The fixture case is `pet-care-001`, and the two pets are
`pet-001` and `pet-002`. Both display the name Milo. Use the stable pet IDs,
species, and synthetic microchip IDs for every decision.

## Plan and checks

Use `code.run_luau` for identity, date, vaccine, option, refill, dose, and
write-body checks. Keep the two pets separate. Pet-001 is the dog that boards
from 2026-09-20 through 2026-09-24. Pet-002 is the cat that stays home.

Pet-001 needs rabies and bordetella before the 2026-09-18 boarding deadline.
Its annual exam is due 2026-09-16. Pet-002 is current for boarding vaccines;
its annual exam is due 2026-09-17. Preserve both care plans and do not infer a
new treatment.

The due refill is `refill-001` for pet-001 on 2026-09-15. Use medication
`med-001`, quantity 20, and this exact instruction:

```text
Give 25 mg by mouth twice daily with food; do not change dose.
```

Do not select pet-002's later refill or alter either dose. Select
`option-001` for pet-001 because it is feasible on 2026-09-14, includes the
required vaccines and annual exam, and is before the boarding deadline.
Select `option-003` for pet-002 because it is the feasible annual exam on
2026-09-16. Reject `option-002` because it is too late, and reject
`option-004` because boarding vaccines are not required for pet-002.

Present the findings and the exact synthetic action body. Ask for human
approval before the first write. After approval, call `schedule_pet_care`
exactly once with these exact arguments:

```json
{
  "case_id": "pet-care-001",
  "visit_option_ids": ["option-001", "option-003"],
  "refill_pet_id": "pet-001",
  "medication_id": "med-001",
  "dose_instruction": "Give 25 mg by mouth twice daily with food; do not change dose.",
  "refill_quantity": 20,
  "boarding_start": "2026-09-20",
  "boarding_end": "2026-09-24",
  "approval_note": "Synthetic pet-care scheduling only; no clinic contact or medication change."
}
```

The POST records one synthetic action only. It must never be described as a
real appointment, vaccination, boarding booking, refill, prescription, or
payment. After the POST succeeds, call `get_pet_care_confirmations` exactly
once. Do not call the POST or any source operation again on continuation.

## Acceptance criteria

- The seven initial reads occur once each, in the listed order.
- Both Milo records remain distinct by stable ID, species, and microchip.
- The boarding pet, travel dates, vaccine requirements, and deadlines are
  matched to pet-001.
- `option-001` and `option-003` are selected for the stated reasons. The two
  invalid options are rejected for the stated reasons.
- The refill uses pet-001, medication `med-001`, quantity 20, and the exact
  dose instruction. No dose is changed.
- The synthetic POST is shown behind a human approval gate and runs once with
  the exact body above.
- The confirmation read runs once after the POST.
- The fixture records exactly nine service calls: seven reads, one POST, and
  one confirmation read, in that order. The POST's submission count is one.
- The returned confirmations contain both selected visits, both stable pet
  identities, the exact refill instruction and quantity, successful statuses,
  and the synthetic source locator.
- Repeating or continuing the task does not repeat the POST or source reads.
- No real clinic, boarding facility, travel provider, pharmacy, pet owner,
  appointment, prescription, charge, payment, or money movement is used. No
  shared workspace is created.
