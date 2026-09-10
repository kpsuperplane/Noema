# PA-078 direct Go recheck

Updated: 2026-09-10

This is the clean direct-Chat recheck of the Northstar entry-readiness case.
It uses the provider-neutral fixture. No legal decision was made.

## Run record

- Backend source revision: `91a8e2ba`.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Executor turn: `824` (`pa-078-live-20260910-clean`).
- Verification turn: `830` (`pa-078-live-20260910-status`).
- Connector: `synthetic_case_workflow_api_v1_personal-2d79d721`.
- Fixture: `2026-09-10-case-workflow-api-v3`.
- Approval: `action:083fed380c21420f2245169f7ce9cf43`, revision 1.
- Fixture action: `pa-078-action-001`.

## Observed behavior

The agent read `case://PA-078/context` and the pending status before it wrote.
The context supplied the 2026-10-13 arrival, the 2027-01-15 passport expiry,
the 2026-08-01 six-month rule, and the 2026-09-20 Cedar notice. The approved
write used action `update-entry-checklist`.

The final read returned:

- `status`: `verified_synthetic_checklist`
- `action_count`: `1`
- `receipts`: `entry-checklist-001`
- Passport validity remains short through `2027-04-13`.
- Cedar transit requires a visa for Northstar travelers.
- Legal clearance was not granted.

## Acceptance

| Criterion | Result |
| --- | --- |
| Read the current case context through the accepted connector | Pass |
| Apply the pinned rule and updated notice | Pass |
| Keep the passport gap visible | Pass |
| Keep legal clearance open | Pass |
| Require review before the synthetic write | Pass |
| Return one checklist receipt | Pass |
| Verify the final status without a duplicate action | Pass |
| Keep the no-real-provider boundary | Pass |

## Current v4 direct recheck

The later all-range recheck used turns `851`, `852`, and `854`, action
`action:edcc282b136130c6c76cc81726834f71`, and fixture action
`pa-078-action-001`. It returned `verified_synthetic_checklist` with one
`entry-checklist-001` receipt. The [v4 ledger](../direct-recheck-ledger-v4.json)
is the current call-count authority; this section supersedes the older v3
turn and connector details above.
