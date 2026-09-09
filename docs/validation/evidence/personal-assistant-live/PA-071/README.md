# PA-071 synthetic pet-care planning and scheduling

Verdict: Pass after a reviewed API connector, seven ordered source reads,
identity and policy checks, one approval-gated synthetic write, one
confirmation read, and direct call-ledger verification.

This case used the live Go Noema development instance. The pets, owner, care
plans, vaccines, boarding, travel, visit options, refill, booking, and
confirmations were synthetic. No real clinic, boarding facility, travel
provider, pharmacy, appointment, prescription, charge, payment, or money
movement occurred.

## Case and fixture

- Case: `pet-care-001`
- Fixture: `2026-09-09-pet-care-api-v1`
- Fixture source:
  [`run-mock-pet-care-api.ts`](../../../../../scripts/acceptance/run-mock-pet-care-api.ts)
- Origin: `https://iso-scan-authorization-ruling.trycloudflare.com/`
- Documentation: `https://iso-scan-authorization-ruling.trycloudflare.com/docs`
- Definition: `definition:synthetic_pet_care_api_v1`
- Accepted semantic digest:
  `23681a58fb0ecba53e6dc60f6097809c07cb63c02f9c3e8464bb9709dad9dc0b`
- Active connection: `be6439434be1224531bd5150201025a5`
  (`personal-be643943`)
- Connection revision: 2; policy revision: 2
- Policies: automatic reads and always-ask writes

## Connector setup

The setup task `task:d9aaeca8e00aa63d5e684dba0cec000f` opened the exact live
documentation before connector work. After one clarification, it made one
no-argument `adapter.definition_template` call and one
`adapter.propose_definition` call. The pending digest was
`ea0f94cf5f0b7ef8ed2c3d1b6c65d5d9357567917eb841d0eb1e11b3f92d7bd1`; the
operator accepted it as canonical digest `23681a58...` above. Setup made no
`/v1` request and no state-changing request.

The first setup task `task:8921fbc1c2773ae374e548687473a938` was rejected by
the documented response-size limit for `list_pet_visit_options` (33,918 bytes
against the 32 KiB limit). It made no service-route call. The retry tightened
only that operation's bounds. The live documentation also stated that an
exact repeated POST is idempotent, so the operator resolved the clarification
by declaring the write idempotent while retaining `retry: never`.

The reviewed connector contains exactly nine operations:

| Operation | Method and route | Behavior |
| --- | --- | --- |
| `get_pet_profile` | GET `/v1/profile` | automatic read |
| `list_pets` | GET `/v1/pets` | automatic read |
| `get_vet_plans` | GET `/v1/vet-plan` | automatic read |
| `list_pet_refills` | GET `/v1/refills` | automatic read |
| `get_boarding_requirements` | GET `/v1/boarding-requirements` | automatic read |
| `get_pet_travel_plan` | GET `/v1/travel` | automatic read |
| `list_pet_visit_options` | GET `/v1/visit-options` | automatic read |
| `schedule_pet_care` | POST `/v1/care-actions` | always-ask synthetic write |
| `get_pet_care_confirmations` | GET `/v1/confirmations` | automatic read |

## Execution

The first execution task `task:724875a4d99379bb30439ce837dfbf49` exposed a
fixture-contract mismatch before approval. The generated transforms expected
stable response names that the fixture did not yet expose. It was canceled
after the seven reads and before any write. The fixture then added documented
compatibility response names and was restarted, which reset its in-memory
request ledger.

The clean retry task was
`task:98f576813adeef778998e1ac3a739c32`. Its planner was
`run:1d95c9949d7ec96f41d46bcaeb8ae42f`. The pre-approval executor was
`run:a55ede6f43108f513e29356a1bf3321b`; the approved continuation was
`run:67e8ab70e1d5a11f9f47d6390db674dc`. The final reviewer was
`run:5f90ffed425b17e9f51a3cbcc3fde7b6` after one provider-failed reviewer
attempt.

Noema read the seven sources in the required order. It kept the two pets
named Milo separate by stable ID, species, and synthetic microchip. Luau
checks matched the boarding pet, dates, vaccine deadline, feasible visit
options, rejected options, due refill, exact dose instruction, and exact
write body.

Task gate `gate:df4028598192da777d1e2c20c0138793` asked for approval of the
exact synthetic action. Governed action
`action:0853acd5518c51a561f5a5fa954f06fd` succeeded once after approval. The
confirmation read then succeeded once. The task reached Done and the reviewer
approved the result.

The action returned `care-action-001`, `submission_count=1`, visit records for
`pet-001` and `pet-002`, the exact unchanged `med-001` dose and quantity, and
locator `pet://care-actions/care-action-001`.

## Evidence

The measured run, call IDs, policy, artifact, and Luau details are in
[`task-measured-ledger.md`](task-measured-ledger.md). The complete seven
source records are in [`task-pet-care-evidence.md`](task-pet-care-evidence.md).
The exact approved body and returned confirmations are in
[`task-post-write-evidence.md`](task-post-write-evidence.md).

The execution artifact is
`artifact:8f8a71e7d51c251d37a65e12aff93aee`, version
`artifact_version:10c6dca80412ebd99870609f0944f1b8`. It is a 3,227-byte
Markdown evidence record with a working Markdown preview and download URL.
No external URL is present.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover and review one documented API connector | Pass |
| Keep connector setup read-only | Pass; no `/v1` setup call |
| Read seven sources exactly once in order | Pass on the clean retry |
| Keep same-name pet identities separate | Pass; IDs, species, and microchips preserved |
| Apply vaccine, travel, option, and refill rules | Pass; bounded Luau checks |
| Require approval before the write | Pass; task gate and governed action |
| Run the exact synthetic action once | Pass; submission count 1 |
| Verify one confirmation read after the write | Pass |
| Keep the no-real-world-action boundary | Pass |

Temporary fixture and tunnel processes must be stopped after this evidence is
committed. Preserve the Go server, socket relay, and unrelated fixture
processes used by other cases.
