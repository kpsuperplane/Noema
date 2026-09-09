# PA-062 create the final synthetic caregiver schedule handoff

Create exactly one local Markdown artifact. Do not call any connector, read a
service endpoint, search another Task, or contact a real service. This task
supplies the complete returned facts below. Do not invent or omit values.

Use one artifact named `synthetic-caregiver-schedule-handoff-final.md`.

Preserve these exact facts:

- Profile: `case_id=caregiver-schedule-001`; `current_date=2026-09-09`;
  `person_label=Jordan Lee (synthetic)`; `schedule_window=2026-09-10 through
  2026-09-11`; `goal=Organize a safe home-care schedule from authorized
  caregiver availability.`; `workspace_scope=one personal Noema workspace`;
  `decision_boundary=Do not contact a real caregiver, provide real care,
  disclose private health information, make a payment, or create a shared
  workspace.`; source `care://profiles/caregiver-schedule-001`.
- Caregiver `caregiver-001`: Maya Chen (synthetic), authorization
  `authorized`, consent `schedule_and_basic_logistics`, skills `meal_prep,
  medication_reminder, mobility_support`, availability `2026-09-10
  08:00-12:00; 2026-09-10 16:00-20:00; 2026-09-11 08:00-12:00`, source
  `care://caregivers/caregiver-001`.
- Caregiver `caregiver-002`: Alex Rivera (synthetic), authorization
  `authorized`, consent `schedule_and_basic_logistics`, skills
  `mobility_support, transport_accompaniment`, availability `2026-09-10
  12:00-16:00; 2026-09-11 12:00-16:00`, source
  `care://caregivers/caregiver-002`.
- Caregiver `caregiver-003`: Sam Patel (synthetic), authorization `authorized`,
  consent `schedule_and_basic_logistics`, skills `mobility_support, meal_prep`,
  availability `2026-09-11 12:00-16:00; 2026-09-11 16:00-20:00`, source
  `care://caregivers/caregiver-003`.
- Coverage requirements: `shift-20260910-0800` on 2026-09-10 08:00-12:00,
  `meal_prep`, purpose `morning routine and meal preparation`;
  `shift-20260910-1200` on 2026-09-10 12:00-16:00, `mobility_support`, purpose
  `midday mobility support`; `shift-20260910-1600` on 2026-09-10 16:00-20:00,
  `meal_prep`, purpose `evening routine and meal preparation`;
  `shift-20260911-0800` on 2026-09-11 08:00-12:00, `medication_reminder`,
  purpose `morning routine and reminder support`;
  `shift-20260911-1200` on 2026-09-11 12:00-16:00, `mobility_support`, purpose
  `midday mobility support`. Source `care://constraints/caregiver-schedule-001`.
- Preferences: authorized caregivers only; maximum 8 hours per caregiver per
  day; maximum 4 hours per shift; minimum 2 hours respite between same-caregiver
  shifts; no overnight shifts; notice scope
  `schedule_and_basic_logistics_only`; source
  `care://preferences/caregiver-schedule-001`.
- Existing schedule: `schedule-000`, status `not_created`, empty assignment
  text, source `care://schedules/caregiver-schedule-001/current`.
- Exact submitted assignments:
  1. `shift_id=shift-20260910-0800;caregiver_id=caregiver-001;required_skill=meal_prep;date=2026-09-10;start=08:00;end=12:00`
  2. `shift_id=shift-20260910-1200;caregiver_id=caregiver-002;required_skill=mobility_support;date=2026-09-10;start=12:00;end=16:00`
  3. `shift_id=shift-20260910-1600;caregiver_id=caregiver-001;required_skill=meal_prep;date=2026-09-10;start=16:00;end=20:00`
  4. `shift_id=shift-20260911-0800;caregiver_id=caregiver-001;required_skill=medication_reminder;date=2026-09-11;start=08:00;end=12:00`
  5. `shift_id=shift-20260911-1200;caregiver_id=caregiver-002;required_skill=mobility_support;date=2026-09-11;start=12:00;end=16:00`.
- Schedule result: `schedule_id=schedule-001`, `receipt_id=schedule-receipt-001`,
  status `confirmed`, submission count `1`, assignment text equal to the five
  strings above joined by ` | `, source
  `care://schedules/caregiver-schedule-001/schedule-001`.
- Cancellation: `cancellation_id=cancellation-001`, caregiver Alex Rivera
  (`caregiver-002`), shift `shift-20260911-1200`, canceled on `2026-09-10`,
  effective `2026-09-11`, reason `synthetic caregiver became unavailable`,
  notice scope `schedule_and_basic_logistics_only`, source
  `care://cancellations/cancellation-001`.
- Reassignment result: `reassignment_id=reassignment-001`,
  `receipt_id=reassignment-receipt-001`, cancellation `cancellation-001`,
  replacement `caregiver-003` (Sam Patel), coverage status `confirmed`,
  submission count `1`, source `care://reassignments/reassignment-001`.
- Notice result: exact message `Schedule update: confirmed shifts and coverage
  change. Please review your assigned times in this personal workspace.` sent
  only to recipient IDs `caregiver-001`, `caregiver-002`, `caregiver-003` under
  scope `schedule_and_basic_logistics_only`; batch `notice-batch-001`, receipt
  `notice-receipt-001`, delivery `sent_to_synthetic_inbox`, submission count
  `1`, source `care://notices/notice-batch-001`.
- Final status result: `schedule_id=schedule-001`, schedule status `confirmed`,
  schedule receipt `schedule-receipt-001`, schedule submission count `1`, the
  exact assignment text above, cancellation `cancellation-001` for canceled
  shift `shift-20260911-1200` and caregiver `caregiver-002` effective
  `2026-09-11`, cancellation reason `synthetic caregiver became unavailable`,
  reassignment `reassignment-001`, reassignment receipt
  `reassignment-receipt-001`, reassignment submission count `1`, replacement
  `caregiver-003`, coverage status `confirmed`, notice batch
  `notice-batch-001`, notice receipt `notice-receipt-001`, notice recipient
  text `caregiver-001, caregiver-002, caregiver-003`, notice scope
  `schedule_and_basic_logistics_only`, notice delivery
  `sent_to_synthetic_inbox`, notice submission count `1`, source
  `care://status/caregiver-schedule-001`.

State that the assignments satisfy authorization, skills, availability, all
five required shifts, the four-hour shift maximum, eight-hour daily maximum,
two-hour respite minimum, and no-overnight rule. State that Sam's confirmed
reassignment supersedes Alex's canceled shift.

State that all records, people, schedules, reassignment, notices, and service
responses are synthetic. State that no real caregiver was contacted, no shared
workspace was created, no health information was disclosed, no care or payment
was provided, and no other real-world action occurred.

## Success conditions

- Exactly one Markdown artifact is created for this Task.
- The artifact includes the complete profile, all caregivers, requirements,
  constraints, existing schedule, exact assignments, cancellation,
  reassignment, notice, final status, receipts, counts, statuses, and every
  `care://` locator above.
- The artifact states the constraint reasoning and that Sam supersedes Alex.
- No connector operation, shared workspace, or real-world service is invoked.
