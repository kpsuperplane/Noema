# PA-057 care-plan follow-up

Verdict: Pass after API setup, an explicit-false response-transform repair,
two approved synthetic actions, result verification, a quiet repeat, and one
independently reviewed sourced artifact.

This case used the live Go Noema development instance. The API, records,
measurements, bookings, alert, and result were synthetic. No real patient,
clinician, laboratory, device, or alerting service was contacted.

## Case and fixture

- Fixture: `2026-09-09-care-plan-api-v1`
- Fixture URL:
  `https://dental-printing-throughout-con.trycloudflare.com`
- Case: `care-plan-001`
- Patient label: `Jordan Lee (synthetic)`
- Current date: `2026-09-09`
- Goal: track a synthetic care plan, complete its missing follow-up, and
  preserve the supplied warning rule
- Boundary: no diagnosis, prescription, measurement interpretation, real
  clinician contact, real test, or real alert
- Fixture implementation:
  [`run-mock-care-plan-api.ts`](../../../../../scripts/acceptance/run-mock-care-plan-api.ts)

The fixture exposed a plan, three follow-ups, two measurements, two plan
revisions, one booking route, one alert route, and two verification routes.
The source follow-up snapshot listed F-811 as missing. The final status route
reported its approved booking as scheduled. The quiet repeat explicitly used
the final status as the authority and did not repeat either write.

## Connector setup

Task `task:26d8b881f5942692bce135ae81077d09` inspected the exact documentation
URL, called `adapter.definition_template` first, and proposed exactly nine
operations. It did not invoke a care-plan service endpoint. The pending
proposal digest was
`c711c290fc5b30ce02605fc9b5ccf188b4db58815fbd53700acca525ff8971ef`. The
operator accepted that exact proposal. Its reviewed v1 digest became
`92593be727432f139c8d70971bd1c078d16ceb1744fc144a3b8d96353793388c`.

The resulting connection was
`bcf541f4240c9c1923f8fb04f1029ed7`. It is active at connection revision 3,
with policy revision 2 and nine available tools. Reads use
`allow_automatically`; both POST operations use `always_ask`.

| Operation | Method and path | Behavior |
| --- | --- | --- |
| `get_care_plan_profile` | GET `/v1/profile` | Read-only, automatic |
| `get_care_plan` | GET `/v1/care-plan` | Read-only, automatic |
| `list_care_follow_ups` | GET `/v1/follow-ups` | Read-only, automatic |
| `list_care_measurements` | GET `/v1/measurements` | Read-only, automatic |
| `list_care_plan_revisions` | GET `/v1/plan-revisions` | Read-only, automatic |
| `schedule_care_follow_up_test` | POST `/v1/follow-up-test-bookings` | Unsafe, approval-gated |
| `create_care_threshold_alert` | POST `/v1/threshold-alerts` | Unsafe, approval-gated |
| `get_care_follow_up_status` | GET `/v1/follow-up-status` | Read-only, automatic |
| `get_care_test_results` | GET `/v1/test-results` | Read-only, automatic |

### Demonstrated connector defect and repair

The first monitor attempt, task `task:7e37a934d5ef6a4d42feb813327158ab`,
correctly read the profile and plan. Its generated follow-up transform then
rejected valid JSON twice because a Luau `and ... or nil` expression discarded
`false` values. No write ran. The operator canceled that attempt before any
action request.

Repair task `task:208d55453e1280c04baa74be47139be4` loaded the exact v1 digest
and replaced only `list_care_follow_ups`. Its local Luau check retained
`booking_required:false` for F-812 and `required:false` for F-813, plus all
required strings and dates. The one v2 proposal had digest
`49f01830b58bc03c5904ae37128f1e865e402c506634cf149801ee4d7e2231b6`. After
independent review, the operator accepted it. The active v2 digest is
`b72e7b51dc3dfe4c89d5c05861ff10cf0f4869cce74a5d6441dbc7ef234b4f76`.

## Main execution

The clean rerun was task `task:ff270bc6471beeb448d6feaee578c1b7`. Its first
executor run completed the five source reads exactly once and in order:

1. `get_care_plan_profile`
2. `get_care_plan`
3. `list_care_follow_ups`
4. `list_care_measurements`
5. `list_care_plan_revisions`

The first approval gate asked for one booking of F-811 on 2026-09-12 with
revision v2. The approved action was:

- Action ID: `action:e10ad46a3d60757354f800cc97ce4530`
- Action revision: `1`
- Request: F-811, preferred date `2026-09-12`, instruction revision `v2`
- Approval: `human:local`
- Response: booking `test-booking-001`, receipt
  `test-booking-receipt-001`, status `scheduled`, request count `1`

The second approval gate asked for one alert for `measurement-001` under v2.
The approved action was:

- Action ID: `action:a64dc552f91b1c1cf33b4c0afa2ae383`
- Action revision: `1`
- Request: measurement `measurement-001`, plan revision `v2`
- Approval: `human:local`
- Response: alert `alert-001`, receipt `alert-receipt-001`, status
  `submitted`, request count `1`
- The request note preserved the supplied comparison and warning rule.

After both approvals, Noema called `get_care_follow_up_status` once and
`get_care_test_results` once. The status read returned follow-up `scheduled`,
alert `submitted`, applied revision `v2`, result `received`, receipts
`test-booking-receipt-001`, `alert-receipt-001`, and
`test-result-receipt-001`, next review date `2026-09-19`, and source
`care://status/care-plan-001`. The result read returned result `result-811`,
follow-up F-811, status `received`, receipt `test-result-receipt-001`, date
`2026-09-12`, and source `care://results/result-811`.

The fixture request ledger for the clean run was:

| Sequence | Route | Count | Result |
| ---: | --- | ---: | --- |
| 1 | GET `/v1/profile` | 1 | Source read |
| 2 | GET `/v1/care-plan` | 1 | Source read |
| 3 | GET `/v1/follow-ups` | 1 | Source read |
| 4 | GET `/v1/measurements` | 1 | Source read |
| 5 | GET `/v1/plan-revisions` | 1 | Source read |
| 6 | POST `/v1/follow-up-test-bookings` | 1 | `test-booking-receipt-001` |
| 7 | POST `/v1/threshold-alerts` | 1 | `alert-receipt-001` |
| 8 | GET `/v1/follow-up-status` | 1 | Final status |
| 9 | GET `/v1/test-results` | 1 | `test-result-receipt-001` |

The action event ledger records proposed, approved, execution-started, and
succeeded for both action IDs. The action decision rows are consumed by
`human:local`. No write was duplicated.

## Quiet repeat

Task `task:e6718a4687b8391f8dc745650ac956a2` read all seven sources once:
the five source routes, then status, then results. It made no POST request and
created no action. It preserved the scheduled booking, submitted alert,
received result, all three receipts, v2, next review date, and every returned
`care://` locator. It explicitly honored the v2 instruction not to create a
second alert, even though the source follow-up snapshot still said F-811 was
missing.

## Sourced artifact

Task `task:7b2252867775a905c6ea7ecfaaad59e1` created exactly one local
Markdown artifact without calling the connector:

- Artifact: `artifact:eb4cf89d5de9e12271c07514573abd30`
- Version: `artifact_version:2a7af94776cf663c871201cc0756a6f8`
- Title: `Synthetic Care-Plan Follow-Up Record — Jordan Lee — 2026-09-09`
- Filename: `care-plan-follow-up-record.md`
- Size: 6,311 bytes
- Content digest: unavailable from the artifact service

Independent inspection confirmed that the saved file contains the fixture
facts, plan and revision text, follow-up snapshot, measurements, action IDs
and revisions, approval outcomes, all receipts, final status and result, next
review date, request accounting, every supplied `care://` locator, and the
synthetic-only boundary. It distinguishes the stale source snapshot from the
authoritative final status. No connector call or external state change was
made during artifact creation.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover the service from documentation and propose a connector | Pass; one exact nine-operation proposal was reviewed and accepted. |
| Keep setup read-only | Pass; setup inspected documentation and invoked no care-plan route. |
| Preserve explicit boolean values | Pass after replacing only the affected follow-up transform; local Luau checks retained both false values. |
| Read the source facts in order | Pass; five reads ran once each in the required order. |
| Schedule the missing follow-up | Pass; F-811 was approved and scheduled for 2026-09-12 with v2. |
| Apply the supplied rule and revised instruction | Pass; measurement-001 and the exact warning rule were carried into one v2 alert request. |
| Require human approval for unsafe actions | Pass; both actions were separately approved and consumed by `human:local`. |
| Verify receipts and result status | Pass; status and result reads preserved all three receipts and `received`. |
| Avoid duplicate work on an unchanged repeat | Pass; seven reads ran once and no POST or action was created. |
| Save one complete sourced artifact | Pass; one independently reviewed 6,311-byte Markdown artifact was created. |
| Keep clinical and real-world actions out of scope | Pass; no diagnosis, treatment, clinician contact, real test, real alert, or other clinical decision occurred. |

The temporary fixture and tunnel were stopped after inspection.
