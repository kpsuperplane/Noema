# PA-062 organize the synthetic caregiver schedule

Use the reviewed API connection
`synthetic_caregiver_schedule_api_personal-733594f1`. Its definition is
`definition:synthetic_caregiver_schedule_api` with reviewed semantic digest
`e3a1ff2f8aea5014b973faee48ee1417673148b44a143d7b2d0f08fb09be69b5`.
This is one personal Noema workspace. Do not create or use a shared workspace.
All people, schedules, notices, and service responses are synthetic.

Do not contact a real caregiver, send a real message, disclose health
information, provide care, accept payment, or take another real-world action.
Use only the connector operations named below. Do not browse the web.

## Read and reason

Call each read operation exactly once, in this order, before any write:

1. `get_caregiver_schedule_profile`
2. `list_authorized_caregivers`
3. `get_coverage_requirements`
4. `get_schedule_preferences`
5. `get_existing_caregiver_schedule`

Preserve every returned profile field, caregiver identity, authorization
status, consent scope, skill, availability window, required shift, purpose,
constraint, existing schedule field, and `care://` source locator.

Build a schedule for all five required shifts. Use these exact assignment
strings so the synthetic service can validate each shift:

- `shift_id=shift-20260910-0800;caregiver_id=caregiver-001;required_skill=meal_prep;date=2026-09-10;start=08:00;end=12:00`
- `shift_id=shift-20260910-1200;caregiver_id=caregiver-002;required_skill=mobility_support;date=2026-09-10;start=12:00;end=16:00`
- `shift_id=shift-20260910-1600;caregiver_id=caregiver-001;required_skill=meal_prep;date=2026-09-10;start=16:00;end=20:00`
- `shift_id=shift-20260911-0800;caregiver_id=caregiver-001;required_skill=medication_reminder;date=2026-09-11;start=08:00;end=12:00`
- `shift_id=shift-20260911-1200;caregiver_id=caregiver-002;required_skill=mobility_support;date=2026-09-11;start=12:00;end=16:00`

Before submission, explain in TASK.md or the result how the assignments
respect the returned constraints: authorized caregivers only, each shift no
longer than four hours, at most eight hours per caregiver per day, at least
two hours of respite between same-caregiver shifts, and no overnight shifts.
Do not invent a caregiver, skill, availability window, or shift.

## Submit and recover

Request exactly one approval-gated call to
`submit_caregiver_schedule` with the five assignment strings above. Wait for
the human approval gate before the call. Preserve the returned schedule ID,
receipt ID, status, submission count, assignment text, and `care://` locator.
The expected synthetic receipt is `schedule-001` with
`receipt_id=schedule-receipt-001`, `status=confirmed`, and
`submission_count=1`.

Then call `get_caregiver_cancellation` exactly once. Preserve every returned
cancellation field and locator. The expected cancellation is
`cancellation-001` for `shift-20260911-1200`, where authorized caregiver
`caregiver-002` (Alex Rivera) is unavailable on `2026-09-11`.

Request exactly one approval-gated call to `reassign_canceled_shift` with:

- `cancellation_id=cancellation-001`
- `shift_id=shift-20260911-1200`
- `replacement_caregiver_id=caregiver-003`

Wait for approval. Preserve the returned reassignment ID, receipt ID,
cancellation ID, replacement ID, coverage status, submission count, and
locator. The expected synthetic receipt is `reassignment-001` with
`receipt_id=reassignment-receipt-001`, `coverage_status=confirmed`, and
`submission_count=1`.

Request exactly one approval-gated call to `send_authorized_schedule_notices`
with recipient IDs `[caregiver-001, caregiver-002, caregiver-003]`, exact
notice scope `schedule_and_basic_logistics_only`, and this factual message:
`Schedule update: confirmed shifts and coverage change. Please review your assigned times in this personal workspace.`
The message must contain no health, diagnosis, medicine, or treatment detail.
Wait for approval. Preserve the returned notice batch ID, receipt ID, recipient
text, notice scope, delivery status, submission count, and locator. The
expected synthetic receipt is `notice-batch-001` with
`receipt_id=notice-receipt-001`, `delivery_status=sent_to_synthetic_inbox`,
and `submission_count=1`.

Finally call `get_caregiver_schedule_status` exactly once. Preserve every
returned status field, receipt, count, replacement, notice result, and
`care://` locator. State that the reassignment supersedes Alex's canceled
shift and that all actions stayed within the one personal workspace.

## Success conditions

- The five reads run once each and in the stated order.
- The five required shifts are covered by the documented authorized
  caregivers, skills, availability, and schedule constraints.
- Exactly one approved schedule submission returns `schedule-001`,
  `schedule-receipt-001`, and count `1`.
- Exactly one cancellation read identifies Alex's canceled shift.
- Exactly one approved reassignment uses Sam (`caregiver-003`) and returns
  confirmed coverage with count `1`.
- Exactly one approved notice batch goes only to the three documented
  authorized recipients, uses the exact consent-bounded scope, and returns
  the synthetic inbox receipt with count `1`.
- Exactly one final status read preserves all receipts, statuses, counts, IDs,
  and locators.
- No shared workspace, real message, health disclosure, care, payment, or
  other real-world action occurs.
