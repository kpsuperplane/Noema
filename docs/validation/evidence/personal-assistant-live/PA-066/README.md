# PA-066 home maintenance

Verdict: Pass after reviewed API setup, seven ordered source reads, one
approved synthetic receipt, status verification, native recurring Task
materialization, and an independently reviewed Markdown artifact.

This case used the live Go Noema development instance. The home, HVAC system,
warranty, quotes, service history, receipt, and provider labels were synthetic.
No technician, provider, warranty desk, repair service, or payment service was
contacted.

## Case and fixture

- Fixture: `2026-09-09-home-maintenance-api-v1`
- Fixture URL: `https://ads-beast-palace-mug.trycloudflare.com`
- Documentation: `https://ads-beast-palace-mug.trycloudflare.com/docs`
- Case: `home-maintenance-001`
- Owner: `Jordan Lee (synthetic)`
- Current date: `2026-09-09`
- Time zone: `America/Los_Angeles`
- Scope: one personal Noema workspace
- Fixture implementation:
  [`run-mock-home-maintenance-api.ts`](../../../../../scripts/acceptance/run-mock-home-maintenance-api.ts)

The fixture exposed a profile, HVAC manual, warranty, baseline service receipt,
maintenance constraints, three quotes, an existing-plan record, one bounded
receipt write, and a status record. It cannot dispatch a technician, authorize a
repair, contact a provider, or charge money.

## Connector setup

Task `task:63e981de5b318420aad523fc845ce014` completed with planner
`run:9c2cf9a04cf8f9adc8a60b80471b0628`, initial executor
`run:b54b4d93d145f6363f2d3cc929a0cc33`, correction reviewer
`run:9cbdd2a559622868d61461ee7bb348b2`, correction executor
`run:0c6a595a99fccbf53350520adab584c8`, and final reviewer
`run:ed88697da974dd55a2b2fa9c8b304fc5`. The task opened the documentation,
called `adapter.definition_template` first, and proposed one reviewed API
definition.

The submitted proposal returned `review_required` with pending semantic digest
`5c36849d28c8af63bd4ff197aea2cec48be5bdc34f4fa7c00cde368ba3eafad2`. The
operator accepted that digest. The current reviewed definition has semantic
digest `7c71fcbb210fda295f70ff2baae024f4843767989dcdac808e9ef3b33fffb5d0`,
definition `definition:synthetic_home_maintenance_api`, revision `v1`, exact
origin `https://ads-beast-palace-mug.trycloudflare.com/`, and source reference
`https://ads-beast-palace-mug.trycloudflare.com/docs`.

The active connection is `a73576e79054566be207b8f4d9909d9d`, exposed as
`personal-a73576e7`. It is ready at connection revision 2 and policy revision 2.
Reads are allowed automatically. The only write,
`record_hvac_service_receipt`, is always approval-gated.

The reviewed operations were:

| Operation | Method and path | Behavior |
| --- | --- | --- |
| `get_maintenance_profile` | GET `/v1/profile` | Read-only, automatic |
| `get_hvac_manual` | GET `/v1/manual` | Read-only, automatic |
| `get_hvac_warranty` | GET `/v1/warranty` | Read-only, automatic |
| `get_last_hvac_service` | GET `/v1/service-history` | Read-only, automatic |
| `get_maintenance_constraints` | GET `/v1/constraints` | Read-only, automatic |
| `list_maintenance_quotes` | GET `/v1/quotes` | Read-only, automatic |
| `get_existing_maintenance_plan` | GET `/v1/existing-plan` | Read-only, automatic |
| `record_hvac_service_receipt` | POST `/v1/service-receipts` | Approval-gated synthetic write |
| `get_maintenance_status` | GET `/v1/status` | Read-only, automatic |

Setup made no `/v1` calls and no state-changing calls. The operator's health and
documentation probes are excluded from the service ledger.

## Maintenance plan execution

Task `task:53ae59e787b1f32a507066a83ec7dbc0` completed with planner
`run:02104b7d9fadb2363a682bb0d2c33ca4`, initial executor
`run:ad8a7316a7c6dcdf6ccd3cb699b48ee7`, resumed executor
`run:2a11ed8bac23153dae30fc24be1aa9b7`, retry executor
`run:b4467221cc8f7707945d5b36c8b8c8dd`, and reviewer
`run:c7fb6553a8910fb386d6f25f0b266831`. The resumed executor had one transient
provider failure after saving its result. The automatic retry finished the task
without repeating connector calls. The final task stage is `Done`.

The seven source reads ran once each, in the required order:

| Sequence | Operation | Result |
| ---: | --- | --- |
| 1 | `get_maintenance_profile` | Case, date, owner, boundary, property, budget, and source locator |
| 2 | `get_hvac_manual` | Asset, manufacturer, model, service and filter intervals, duration, safety note, and locator |
| 3 | `get_hvac_warranty` | Exact serial match, coverage dates, coverage summary, and locator |
| 4 | `get_last_hvac_service` | Baseline receipt dated 2026-03-12 for 160 USD |
| 5 | `get_maintenance_constraints` | Safe window, avoid dates, quiet hours, access hours, and 300 USD limit |
| 6 | `list_maintenance_quotes` | Two feasible quotes and one outside the safe window |
| 7 | `get_existing_maintenance_plan` | Due date 2026-09-08 and no existing recurring Task |

Noema calculated `2026-03-12 + 180 days = 2026-09-08`. The due date was one
day before the fixture current date. It selected `quote-001` at 185 USD because
the fixture marked it feasible at the safe-window start, it includes a filter,
and it is warranty-eligible. It kept `quote-002` at 260 USD as the alternate.
It rejected cheaper `quote-003` at 140 USD because 2026-09-11 is outside the
safe window and has unknown warranty eligibility. The selected service leaves
`300 - 185 = 115 USD`.

The source contains one inconsistency. Access is described as weekdays, but
2026-09-12 is a Saturday. The fixture still marks `quote-001` feasible. Noema
preserved the inconsistency and put a fresh source reread in the recurring Task
procedure.

The task presented gate `gate:097c81f0469928bb11238d49bd1cbc8c`. After approval,
action `action:845e5eb9e8feeba9bb93ce6ce17b78f5` called the POST once with the
exact approved arguments:

```text
asset_id: hvac-001
service_date: 2026-09-12
vendor_label: CoolAir Service (synthetic)
service_type: seasonal_tune_up
amount_usd: 185
warranty_id: warranty-hvac-001
notes: Synthetic service receipt only; no real dispatch or payment.
```

The synthetic response was receipt `service-receipt-001`, status
`recorded_in_synthetic_maintenance`, submission count `1`, next due date
`2027-03-11`, and locator `home://service-receipts/service-receipt-001`.

The final `get_maintenance_status` call ran once. It returned case
`home-maintenance-001`, asset `hvac-001`, derived due date `2026-09-08`,
selected quote `quote-001`, selected amount 185 USD, budget limit 300 USD,
budget remaining 115 USD, receipt `service-receipt-001`, submission count 1,
status `recorded_in_synthetic_maintenance`, next due date `2027-03-11`, and
locator `home://status/home-maintenance-001`.

The fixture request log, excluding health and documentation probes, contains
exactly this service ledger:

1. GET `/v1/profile`
2. GET `/v1/manual`
3. GET `/v1/warranty`
4. GET `/v1/service-history`
5. GET `/v1/constraints`
6. GET `/v1/quotes`
7. GET `/v1/existing-plan`
8. POST `/v1/service-receipts`
9. GET `/v1/status`

This proves that the retry did not duplicate the write or any source read.

## Native recurring Task

The operator materialized one native personal Task after the verified result.
The background executor did not claim to create it.

- Task: `task:12d0a7f7825f1d0c573daa73d6291e20`
- Title: `HVAC seasonal maintenance — synthetic home`
- Task stage: `Inbox`
- Recurrence: `recurrence:3f1af04f7a0ab29b3aff3331c83f97e9`
- First run: `2026-09-12T16:00:00Z`
- Time zone: `America/Los_Angeles`
- Cron: `0 9 12 3,9 *`
- Overlap policy: `SKIP`
- Missed-run policy: `RUN_ONCE`
- Lifecycle: `ACTIVE`
- Recurrence revision: `1`
- Next projected run: `2027-03-12T17:00:00Z`
- Initial occurrence: `MATERIALIZED` at `2026-09-12T16:00:00Z`

The cron expression explicitly names March and September. Each occurrence is a
review trigger. A future run must reread current records, check the Saturday
and weekday-access inconsistency, and request approval before any write.

## Sourced artifacts

The execution task created one small evidence-ledger artifact while assembling
the result:

- Artifact: `artifact:26edbc20e1cc1b2d3ddc798ebbbd0136`
- Version: `artifact_version:4ebd17046fd185c437430559f53c6709`
- Title: `Synthetic HVAC maintenance evidence ledger`
- Kind: `evidence-ledger`
- Size: 4,172 bytes
- Media type: `text/markdown`
- Preview: `MARKDOWN`
- Download: `/artifacts/versions/4ebd17046fd185c437430559f53c6709/download`
- Content SHA-256: `9d0699c6b1cf6b38b02b15869e395990564f7f4be78a698e6cf1a3723bb09ec4`

The artifact task `task:6530266ca3b58283557e8483dc459688` created exactly one
final local Markdown artifact. Its planner was
`run:2e97044dc224e4836a12f92713f0ec17`, its executor was
`run:0798d222f4798b783aada3d4d8454c37`, and its reviewer was
`run:686bf51f39e432e7e4c4d128997cc624`. No connector was available to this
artifact task.

- Artifact: `artifact:93c1a263e29ebc14f126f37e424961b6`
- Version: `artifact_version:5671e526f928b9f00f26e5d5490046fc`
- Title: `Synthetic Home Maintenance Final Review`
- Filename: `synthetic-home-maintenance-final.md`
- Size: 7,051 bytes
- Media type: `text/markdown`
- Preview: `MARKDOWN`
- Download: `/artifacts/versions/5671e526f928b9f00f26e5d5490046fc/download`
- Content SHA-256: `0b4e09226c7b138c81098abb20f853e17b90c73a87b5062c1bd4af10b423d4bb`

The artifact preview was inspected through `artifactVersionDetail`. The file
contains the action boundary, all required source facts and locators, the due
date arithmetic, all three quote decisions, receipt and budget results, the
recurring Task, and the future-run safeguards. It preserves the unavailable
serial-number boundary and does not claim an executor-created recurrence.

The artifact records that the authorized verified-facts brief did not repeat
the manual safety-note wording. It does not invent or paraphrase that wording.
The full safety note is preserved in the connector result and execution result.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover the service and propose a connector | Pass; one reviewed nine-operation API definition |
| Keep setup read-only | Pass; no `/v1` or state-changing call occurred during setup |
| Read all source records once in order | Pass; seven reads ran once in the specified order |
| Derive the due date and compare quotes | Pass; 2026-09-08, quote-001 selected, quote-002 retained, quote-003 rejected |
| Keep the unsafe operation behind approval | Pass; one task gate and one governed action were approved |
| Record one synthetic receipt | Pass; exact POST returned receipt service-receipt-001 and count 1 |
| Verify final maintenance status | Pass; one status read returned 115 USD remaining and next due 2027-03-11 |
| Materialize native recurring work | Pass; one personal Task and active recurrence were created by the operator |
| Save one complete sourced brief | Pass; one reviewed 7,051-byte Markdown artifact with preview |
| Avoid real-world action | Pass; no technician, provider, repair, warranty, payment, or shared-workspace action occurred |

The temporary fixture and tunnel were stopped after evidence collection. The
temporary host mapping was removed.
