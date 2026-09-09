# PA-062 caregiver schedule

Verdict: Pass after a connector-origin recovery, three approved synthetic
writes, one successful status-read rerun, and an independently reviewed
sourced handoff artifact.

This case used the live Go Noema development instance. The profile, people,
schedules, cancellation, reassignment, notices, and service were synthetic.
No real caregiver, message service, health service, payment service, or shared
workspace was used.

## Case and fixture

- Fixture: `2026-09-09-caregiver-schedule-api-v1`
- Final fixture URL: `https://888e28d58ce564.lhr.life`
- Case: `caregiver-schedule-001`
- Person: `Jordan Lee (synthetic)`
- Window: `2026-09-10 through 2026-09-11`
- Workspace: one personal Noema workspace
- Fixture implementation:
  [`run-mock-caregiver-schedule-api.ts`](../../../../../scripts/acceptance/run-mock-caregiver-schedule-api.ts)

The fixture exposes five ordered source reads, three approval-gated writes,
one cancellation read, one final status read, and stable `care://` locators.
It rejects real-world, health, payment, and shared-workspace behavior.

The operator separately probed `/docs` and `/health` while keeping the tunnel
alive. Those probes are not task service calls. The final service request
ledger, excluding those probes, is:

1. GET `/v1/profile`
2. GET `/v1/caregivers`
3. GET `/v1/coverage-requirements`
4. GET `/v1/preferences`
5. GET `/v1/schedule`
6. POST `/v1/schedule`
7. GET `/v1/cancellation`
8. POST `/v1/reassign`
9. POST `/v1/notices`
10. GET `/v1/status`

The fixture returned one submission for each write. The final status response
returned all three receipts and all three submission counts as `1`.

## Connector setup

The connector setup required several bounded corrections. Each failed setup
task was stopped before any caregiver service route ran.

| Task | Outcome |
| --- | --- |
| `task:935d94f3a09318b62bec176f2446420b` | Canceled after the proposal used unsupported `json` response fields. |
| `task:2a6785afb374cfd5381aad7aea9336ac` | Canceled after the proposal used unsupported `string_array` response fields and exceeded browser capacity. |
| `task:4b43ae05abf53e8b96c0b5e706d8e284` | Canceled after another scalar response contract exceeded browser capacity. |
| `task:3b7e22e6961285c1a6ac016713efa9fc` | Canceled after the origin omitted its required trailing slash. |
| `task:cbd497afbfb5db54573f03e1840b625d` | Scalar v1 proposal succeeded; digest `edf254f4e83cf14e37cb4728ea9b74ef3aaa92479aff6b6c8968cef98299c737` was accepted as reviewed digest `8a9df7ba250cfdfd926cd5ca1c9717d81a3f1c7357a0f674337530140c93e91b`. |
| `task:6d070be7cbd1e0a31da3c72ed361f7ed` | Canceled after an early schedule execution attempt did not complete the required setup path. |
| `task:73087a79131357fd3d9e0056109bf12a` | Canceled because a new definition needed an explicit replacement target. |
| `task:026454d1f9fea29e0d238a1fb31a61fe` | Canceled after a compiled custom response transform failed validation. |
| `task:87f95d40df721c1ad3d2336144f10cb5` | Canceled after a response fields map failed proposal validation. |
| `task:bb2ca2c5b000088752404ca6d87e3970` | Canceled after the model copied the template's ordinary `[REDACTED]` marker into authorization. |
| `task:bfcfeae83308a33fbe2cd73ae53b2f49` | Corrected v3 proposal succeeded with literal no-auth and generated array responses; pending digest `60e1384ba21d80280e9a7cbdbfae38fce4a6acc309bd81ec990dc9e759136524` was accepted as reviewed digest `e3a1ff2f8aea5014b973faee48ee1417673148b44a143d7b2d0f08fb09be69b5`. |
| `task:b254e492a59a8dfb00c58d50f055a3b5` | Terminal setup success. It verified the v3 proposal and no service calls. |
| `task:4c2b542e63c73a3b1e4b7ef6cb584b6e` | Replacement-origin v4 setup success. It verified the exact v3 base link, ten operations, the non-empty description, eight response fields, and no service calls. |

The final v4 proposal was first left in `review_required` with digest
`5054b6c81095117c18747a2bbad0aa05d07257b647cd44204d40f5cf9cef4a0d`. The
operator accepted that exact digest after reviewing the proposal. Noema
activated reviewed digest
`1344c58df9951f3217fe345dfba4e8e397535ba717fc5d3fc60113013e9ae77a`.

- Definition: `definition:synthetic_caregiver_schedule_api`
- Connection: `733594f1d9b4a1536e89e2f3235eb0b9`
- Connection slug: `personal-733594f1`
- Connection revision: `5`
- Policy revision: `2`
- Data sharing policy: `allow_automatically` for reads
- Unsafe action policy: `always_ask`
- Origin: `https://888e28d58ce564.lhr.life/`
- Source reference: `https://888e28d58ce564.lhr.life/docs`

The final reviewed definition has these ten bounded operations:

| Operation | Method and path | Behavior |
| --- | --- | --- |
| `get_caregiver_schedule_profile` | GET `/v1/profile` | Read-only, automatic |
| `list_authorized_caregivers` | GET `/v1/caregivers` | Read-only, automatic |
| `get_coverage_requirements` | GET `/v1/coverage-requirements` | Read-only, automatic |
| `get_schedule_preferences` | GET `/v1/preferences` | Read-only, automatic |
| `get_existing_caregiver_schedule` | GET `/v1/schedule` | Read-only, automatic |
| `get_caregiver_cancellation` | GET `/v1/cancellation` | Read-only, automatic |
| `get_caregiver_schedule_status` | GET `/v1/status` | Read-only, automatic |
| `submit_caregiver_schedule` | POST `/v1/schedule` | Unsafe, approval-gated |
| `reassign_canceled_shift` | POST `/v1/reassign` | Unsafe, approval-gated |
| `send_authorized_schedule_notices` | POST `/v1/notices` | Unsafe, approval-gated |

## Schedule execution

The main execution task was `task:262fa96f4ad9b0f971cb4bfbb06298e6`.
Its planner was `run:564dca53b5b283101a867093a45e19dc`. The first executor
`run:becd2f47698907be69169869051ae86b` completed the five required reads once
and in order. It preserved the complete returned profile, caregiver records,
requirements, preferences, existing schedule, and locators.

The first approval gate was
`gate:64fda9a543c060f1d98dbb3129e494f8`. The approved governed action was
`action:3957a04046e0ec69c7ec588e876c75f7`. It submitted exactly these five
assignments:

1. `shift_id=shift-20260910-0800;caregiver_id=caregiver-001;required_skill=meal_prep;date=2026-09-10;start=08:00;end=12:00`
2. `shift_id=shift-20260910-1200;caregiver_id=caregiver-002;required_skill=mobility_support;date=2026-09-10;start=12:00;end=16:00`
3. `shift_id=shift-20260910-1600;caregiver_id=caregiver-001;required_skill=meal_prep;date=2026-09-10;start=16:00;end=20:00`
4. `shift_id=shift-20260911-0800;caregiver_id=caregiver-001;required_skill=medication_reminder;date=2026-09-11;start=08:00;end=12:00`
5. `shift_id=shift-20260911-1200;caregiver_id=caregiver-002;required_skill=mobility_support;date=2026-09-11;start=12:00;end=16:00`

The fixture returned:

- Schedule: `schedule-001`
- Receipt: `schedule-receipt-001`
- Status: `confirmed`
- Submission count: `1`
- Locator: `care://schedules/caregiver-schedule-001/schedule-001`

The assignments use only authorized caregivers. Each caregiver has the
required skill and returned availability. Each shift is four hours. Daily
hours remain at or below eight. Maya's two September 10 shifts leave four
hours of respite. No shift crosses midnight.

The cancellation read ran after schedule submission. It returned
`cancellation-001` for Alex Rivera (`caregiver-002`) and
`shift-20260911-1200`, effective `2026-09-11`, with reason
`synthetic caregiver became unavailable` and locator
`care://cancellations/cancellation-001`.

The second approval gate was
`gate:acc62b3a6ac4b4369c620dff9f0841eb`. Governed action
`action:1ab7f265a02633c65718ea0ad909727e` submitted the single replacement
request for `caregiver-003`.

- Reassignment: `reassignment-001`
- Receipt: `reassignment-receipt-001`
- Cancellation: `cancellation-001`
- Replacement: `caregiver-003` (Sam Patel)
- Coverage: `confirmed`
- Submission count: `1`
- Locator: `care://reassignments/reassignment-001`

Sam was authorized, had `mobility_support`, and was available for the exact
canceled shift. His confirmed reassignment supersedes Alex's canceled shift.

The third approval gate was
`gate:c50abdbaaa9a47adf6149d019b455c9c`. Governed action
`action:27aa1ebdfb77b0617b92e1bce2654208` sent exactly one factual notice to
the three authorized synthetic recipients under
`schedule_and_basic_logistics_only`:

> Schedule update: confirmed shifts and coverage change. Please review your assigned times in this personal workspace.

The fixture returned:

- Notice batch: `notice-batch-001`
- Receipt: `notice-receipt-001`
- Recipients: `caregiver-001, caregiver-002, caregiver-003`
- Delivery: `sent_to_synthetic_inbox`
- Submission count: `1`
- Locator: `care://notices/notice-batch-001`

The message contains no health, diagnosis, medicine, or treatment detail.

## Final status recovery

The original v3 tunnel rotated after the three writes. Its three attempted
status calls returned HTTP 503 before reaching the fixture. The task result
reported that limitation honestly. The fixture ledger confirms that the five
preflight reads and three writes each reached the service once; the failed
status attempts did not create a service record.

The operator then created v4 setup task
`task:4c2b542e63c73a3b1e4b7ef6cb584b6e`, accepted the reviewed replacement
origin, and kept the new tunnel alive. A separate read-only task,
`task:d68f2c6602c968ef2fd1df3765c4fad2`, called only
`get_caregiver_schedule_status` once through the active reviewed connection.
Its executor was `run:8222f100c6e4ee1dd59f7e96351477fd`. The task and reviewer
both passed.

The status response returned every aggregate field:

- Schedule `schedule-001`, receipt `schedule-receipt-001`, status `confirmed`,
  count `1`
- Cancellation `cancellation-001`, canceled shift
  `shift-20260911-1200`, caregiver `caregiver-002`, effective date
  `2026-09-11`
- Reassignment `reassignment-001`, receipt `reassignment-receipt-001`,
  replacement `caregiver-003`, coverage `confirmed`, count `1`
- Notice `notice-batch-001`, receipt `notice-receipt-001`, recipient text
  `caregiver-001, caregiver-002, caregiver-003`, scope
  `schedule_and_basic_logistics_only`, delivery `sent_to_synthetic_inbox`,
  count `1`
- Aggregate locator: `care://status/caregiver-schedule-001`

## Sourced artifact

Task `task:38c8a48acebda308a3b375a9aed50914` created exactly one local
Markdown artifact without calling a connector. Its executor was
`run:19a7436a79d54f69d2f882ac8aa5e1f2`; reviewer was
`run:0afc65f1b5ce9288ef5c56ce5a5e6a86`.

- Artifact: `artifact:63506aa950cbe4879a3a83bb346de65a`
- Version: `artifact_version:5977e8b1a5bb8c1313cdf278f365a7ee`
- Filename: `synthetic-caregiver-schedule-handoff-final.md`
- Size: 8,990 bytes
- Preview: Markdown

The reviewed artifact contains the complete profile, all caregivers,
requirements, constraints, existing schedule, exact assignments, cancellation,
reassignment, notice, final status, receipts, counts, statuses, and every
`care://` locator. It records the constraint reasoning, states that Sam
supersedes Alex, and states the synthetic-only boundary.

## Acceptance

| Criterion | Result |
| --- | --- |
| Propose and review a connector with the documented operations | Pass after v1 through v4 contract corrections; the active reviewed definition has ten operations. |
| Keep connector setup read-only | Pass; setup tasks called only adapter tools and no fixture service route. |
| Read profile, caregivers, requirements, preferences, and existing schedule in order | Pass; the fixture ledger records the five reads once and in order. |
| Preserve authorization, skills, availability, requirements, and constraints | Pass; the executor and artifact preserve all returned records and locators. |
| Submit one feasible schedule | Pass; one approved POST returned `schedule-001`, `schedule-receipt-001`, `confirmed`, and count `1`. |
| Detect and recover the cancellation | Pass; one cancellation read identified Alex, and one approved Sam reassignment returned confirmed coverage. |
| Send only the authorized notice | Pass; one approved POST reached the three synthetic inboxes with the exact consent-bounded scope and count `1`. |
| Verify final status | Pass after the tunnel-origin correction; one status-only task returned the complete aggregate response. |
| Save one complete sourced handoff | Pass; one independently reviewed 8,990-byte Markdown artifact contains every supplied fact and locator. |
| Keep real-world actions and shared workspaces out of scope | Pass; all records and actions were synthetic and the task used one personal workspace. |

The temporary caregiver fixture, keepalive, and tunnels must be stopped after
inspection.
