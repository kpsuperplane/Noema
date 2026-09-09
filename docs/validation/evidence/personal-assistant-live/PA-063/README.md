# PA-063 care changes

Verdict: Pass after a fixture-route correction and a complete retest.

This case used the live Go Noema development instance. The service, person,
observations, recipients, messages, and receipts were synthetic. No real
care, clinician, message channel, payment, or shared workspace was used.

## Case and fixture

- Fixture: `2026-09-09-care-change-scope-api-v1`
- Fixture origin: `https://live-atlanta-favor-midlands.trycloudflare.com/`
- Documentation: `https://live-atlanta-favor-midlands.trycloudflare.com/docs`
- Case: `care-change-001`
- Person: `Jordan Lee (synthetic)`
- Window: `2026-09-08 through 2026-09-09`
- Workspace: one personal Noema workspace
- Fixture implementation:
  [`run-mock-care-change-scope-api.ts`](../../../../../scripts/acceptance/run-mock-care-change-scope-api.ts)

The fixture records a baseline temperature observation, a newer observation,
one supplied threshold rule, two authorized recipients with different scopes,
two approval-gated writes, and a final status read. It rejects diagnosis,
prescribing, treatment changes, real contact, payment, and shared workspaces.

## Connector setup

Setup task: `task:5947d752e170d13874159b278876c73d`.

The task called `adapter.definition_template` once and
`adapter.propose_definition` once. The proposal returned
`review_required` with pending digest
`ffbe69c423c721ba9a3cae8bfc595c57a0a235b09eda4581a2c28de8db499edb`. The
operator accepted that exact proposal. No service endpoint ran during setup.

The reviewed active digest is
`93379cc021fe424d79dca24dcac1534fb9edac7e73548c1d5906b6ef4343d642`.

- Definition: `definition:synthetic_care_change_scope_api`
- Revision: `v1`
- Connection: `92de4cc836635ecaf968055b36fe58e6`
- Connection slug: `personal-92de4cc8`
- Connection revision: `2`
- Policy revision: `2`
- Read policy: `allow_automatically`
- Unsafe action policy: `always_ask`

The proposal contained exactly these ten operations:

| Operation | Route | Behavior |
| --- | --- | --- |
| `get_care_change_profile` | GET `/v1/profile` | Read-only, automatic |
| `get_first_daily_observation` | GET `/v1/observations/first` | Read-only, automatic |
| `get_latest_daily_observation` | GET `/v1/observations/latest` | Read-only, automatic |
| `get_escalation_rule` | GET `/v1/escalation-rule` | Read-only, automatic |
| `get_family_recipient` | GET `/v1/recipients/family` | Read-only, automatic |
| `get_clinical_recipient` | GET `/v1/recipients/clinical` | Read-only, automatic |
| `get_existing_care_updates` | GET `/v1/updates` | Read-only, automatic |
| `send_family_care_update` | POST `/v1/family-update` | Unsafe, approval-gated |
| `send_clinical_care_escalation` | POST `/v1/clinical-escalation` | Unsafe, approval-gated |
| `get_care_change_status` | GET `/v1/status` | Read-only, automatic |

The executor also verified the exact operation order, behavior flags, no
pagination, generated flat-object responses, exact POST templates, and a
largest computed schema of 1,301 bytes. The reviewer approved the setup.

## Initial execution and correction

The first execution task was `task:8ac30a0549c4097412fd9f15c3f864c1`. It
completed the six source reads, both approved writes, and the final status
read. Its seventh source read, `get_existing_care_updates`, returned one
`404 not_found`. The reviewer approved the result because the read was
attempted once and the task reported the limitation without inventing data.

Inspection showed that the running fixture process had started before the
`/v1/updates` route was added to the script. The operator restarted the
synthetic process without changing the public origin. Direct local and public
requests then returned the intended empty update register:

```json
{"existing_update_count":0,"existing_update_summary":"No prior synthetic care updates exist for this case.","source_locator":"care-change://updates/care-change-001/current"}
```

## Corrected retest

The complete retest task was `task:f25c899ba236d81809c612d25acabd37`.
The reviewer approved its result.

The corrected fixture request ledger, excluding operator `/health` probes, was
exactly:

1. GET `/v1/profile`
2. GET `/v1/observations/first`
3. GET `/v1/observations/latest`
4. GET `/v1/escalation-rule`
5. GET `/v1/recipients/family`
6. GET `/v1/recipients/clinical`
7. GET `/v1/updates`
8. POST `/v1/family-update`
9. POST `/v1/clinical-escalation`
10. GET `/v1/status`

All seven reads completed once and in order before either write. The corrected
update register returned count `0`, the supplied empty-register summary, and
locator `care-change://updates/care-change-001/current`.

The baseline was `observation-001`, `temperature_c=37.2 C`, on
`2026-09-08 09:00` while resting. The latest was `observation-002`,
`temperature_c=38.4 C`, on `2026-09-09 09:00` while resting. Under supplied
`rule-001`, the threshold result was `met`. No medical interpretation was
added.

### Approved family update

Gate: `gate:a8fb21bf5e4c626853c8049b420a4330`.

Action: `action:3d6e581206f91679958d2b5cb394143c`.

The approved call sent exactly once:

- Recipient: `recipient-001`, Maya Chen (synthetic)
- Scope: `basic_schedule_and_support_only`
- Message: `Care update: Jordan's schedule includes a check-in this evening. Please record the next observation in this personal workspace.`
- Update: `family-update-001`
- Receipt: `family-receipt-001`
- Delivery: `sent_to_synthetic_inbox`
- Submission count: `1`
- Locator: `care-change://updates/family-update-001`

The message contained no observation, diagnosis, symptom, medicine, or
treatment detail.

### Approved clinical escalation

Gate: `gate:60367adfa11ea2b2c11ab8ed81d192b7`.

Action: `action:1c2481e9c01232be69ef67c6a0182759`.

The approved call sent exactly once:

- Recipient: `recipient-002`, Dr. Morgan (synthetic)
- Scope: `clinical_observation_escalation_only`
- Rule: `rule-001`
- Observation IDs: `observation-001, observation-002`
- Escalation: `clinical-escalation-001`
- Receipt: `clinical-receipt-001`
- Threshold: `met`
- Delivery: `sent_to_synthetic_clinical_inbox`
- Submission count: `1`
- Locator: `care-change://updates/clinical-escalation-001`

The message preserved both observations and the rule reference. It requested
no diagnosis or treatment change.

### Final status

The final status read ran exactly once after both writes. It confirmed latest
observation `observation-002`, threshold `met`, both recipient scopes, both
synthetic delivery statuses, receipts `family-receipt-001` and
`clinical-receipt-001`, and submission count `1` for each write. Its locator is
`care-change://status/care-change-001`.

## Handoff artifact

Artifact task: `task:037a5dc9a943d10456eb055a6ff54a25`.

The task created exactly one local Markdown artifact and the reviewer approved
its readback:

- Filename: `synthetic-care-change-scope-handoff-final.md`
- Artifact: `artifact:bdc25a28c31475e832d7ab6cfdfd9d94`
- Version: `artifact_version:35ba4ad5af0f87ab443141864e76b0c7`
- Media type: `text/markdown`
- Size: 6,391 bytes
- Preview: Markdown, verified through `artifactVersionDetail`
- Download route: `/artifacts/versions/35ba4ad5af0f87ab443141864e76b0c7/download`

The artifact preserves the first-run 404 limitation honestly. This README
records the corrected retest as the final complete service evidence.

## Safety boundary

No real person, clinician, care provider, message service, payment, treatment,
diagnosis, or shared workspace was involved. All values and locators are
synthetic `care-change://` records. The temporary fixture, tunnel, and
keepalive sessions are stopped after evidence capture.
