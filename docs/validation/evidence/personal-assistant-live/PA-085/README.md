# PA-085 direct Go recheck

Updated: 2026-09-10

This is the clean direct-Chat recheck of the Tuesday school-pickup case.
The school records and transport are synthetic.

## Run record

- Backend source revision: `91a8e2ba`.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Executor turn: `826` (`pa-085-live-20260910-clean`).
- Verification turn: `831` (`pa-085-live-20260910-status`).
- Connector: `synthetic_case_workflow_api_v1_personal-2d79d721`.
- Fixture: `2026-09-10-case-workflow-api-v3`.
- Approval: `action:92acee7706d7a5ee1e8e480c02dd748f`, revision 1.
- Fixture action: `pa-085-action-001`.

## Observed behavior

The agent read `case://PA-085/context` and the pending status before it wrote.
It detected the 15-minute pickup gap and the 22-minute school-to-school trip.
It assigned the other parent to Child A at Northstar School and approved
caregiver C to Child B at Cedar School. The approved write used action
`confirm-school-updates`.

The final read returned:

- `status`: `verified_synthetic_schedule`
- `action_count`: `1`
- `receipts`: `school-a-update-001,school-b-update-001`
- Both school updates were recorded.
- The 18-minute Cedar-to-home route also fits the selected assignments.

## Acceptance

| Criterion | Result |
| --- | --- |
| Read the current case context through the accepted connector | Pass |
| Detect the overlapping pickups | Pass |
| Preserve the custody constraint | Pass |
| Use the numeric travel checks | Pass |
| Require review before the synthetic write | Pass |
| Record separate updates for both schools | Pass |
| Verify the final status and both receipts | Pass |
| Keep the no-real-school boundary | Pass |

