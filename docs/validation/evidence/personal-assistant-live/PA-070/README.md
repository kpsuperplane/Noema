# PA-070 vehicle lifecycle and synthetic service booking

Verdict: Pass after a reviewed API connector, six ordered source reads,
Luau checks, one approval-gated synthetic booking, receipt and status reads,
and direct ledger verification.

This case used the live Go Noema development instance. The vehicle, owner,
recall, registration, service options, booking, and service were synthetic.
No real dealer, registry, insurer, repair shop, appointment, charge, payment,
or registration action occurred.

## Case and fixture

- Case: `vehicle-lifecycle-001`
- Fixture: `2026-09-09-vehicle-lifecycle-api-v1`
- Fixture source:
  [`run-mock-vehicle-lifecycle-api.ts`](../../../../../scripts/acceptance/run-mock-vehicle-lifecycle-api.ts)
- Origin: `https://astronomy-interventions-romantic-cost.trycloudflare.com/`
- Documentation: `https://astronomy-interventions-romantic-cost.trycloudflare.com/docs`
- Definition: `definition:synthetic_vehicle_lifecycle_api_v1`
- Accepted semantic digest:
  `3aa04993ff3dd40a7886f5f87679c95345199d427e7ef871036a7f08b40ed02a`
- Active connection: `e4ed0f7af9753be572b95c86ef87fd80`
  (`personal-e4ed0f7a`)
- Connection revision: 2; policy revision: 2
- Policies: automatic reads and always-ask writes

## Connector setup

Setup task `task:1cb68b0ca4b21f229c083d3801971961` opened the documentation
before connector work. It made one no-argument
`adapter.definition_template` call and one
`adapter.propose_definition` call. The pending proposal digest was
`e047bf5617a68658c68327acdc5dd98958ea155c748bff6e806ba6d63848c3a7`; the
accepted definition has canonical digest `3aa04993...` above. Setup made no
`/v1` request and no state-changing request.

Two earlier setup attempts were canceled after input validation errors. One
included a server-derived `retry` field. The other included a generated
response field that the proposal input does not accept. Neither attempt
called a service route. The final proposal corrected both inputs.

The reviewed connector contains exactly these operations:

| Operation | Method and route | Behavior |
| --- | --- | --- |
| `get_vehicle_profile` | GET `/v1/profile` | automatic read |
| `list_vehicle_service_history` | GET `/v1/service-history` | automatic read |
| `list_vehicle_recalls` | GET `/v1/recalls` | automatic read |
| `get_vehicle_registration` | GET `/v1/registration` | automatic read |
| `list_vehicle_service_options` | GET `/v1/service-options` | automatic read |
| `get_vehicle_constraints` | GET `/v1/constraints` | automatic read |
| `book_vehicle_service` | POST `/v1/service-bookings` | always-ask synthetic write |
| `get_vehicle_service_receipt` | GET `/v1/service-receipt` | automatic read |
| `get_vehicle_status` | GET `/v1/status` | automatic read |

## Execution

The execution task was
`task:2897d1caa7a12305beaa92469f260b76`. Its planner was
`run:42ec01a12d4760eafd69b6e436c80b5f`. The pre-approval executor was
`run:95b9c80557960a5fa4e36b2f1d35ef6e`. The approved continuation was
`run:83d92424d7faf58d76c0d17354c0d4d2`. The reviewer was
`run:c6be6a22b5adcfd13760a9d8276207b2`.

Noema read the six sources in the required order. Luau calculated
`43,620 + 5,000 = 48,620`, matched `RC-2026-04` to synthetic serial
`SYNTH-VALE-001`, rejected the nonmatching recall, rejected the incomplete and
wrong-vehicle options, and selected `option-001`. It found the 325 USD option
within the 500 USD limit and before the 2026-09-30 registration deadline.

Noema opened task gate `gate:2fc8038e65154663682f8e267367d6a4`. The operator
approved the synthetic-only booking. Governed action
`action:a592cf8c67bd7affd566758135c29f6c` then succeeded once with the exact
body in [`task-post-write-evidence.md`](task-post-write-evidence.md).

The receipt returned `service-receipt-001`, status
`booked_in_synthetic_record`, submission count one, 325 USD, completed
`RC-2026-04`, next service at 53,620 miles on 2027-09-16, registration status
`ready_for_renewal`, and the required receipt locator. The status read matched
those values and returned `vehicle://status/vehicle-lifecycle-001`.

## Measured ledger and artifacts

The fixture and task-run database contain exactly nine vehicle-service calls:
six reads, one approved POST, one receipt read, and one status read. The full
call IDs, payloads, setup measurements, artifact metadata, and Luau call IDs
are in [`task-measured-ledger.md`](task-measured-ledger.md).

The complete initial source payloads are in
[`task-vehicle-evidence.md`](task-vehicle-evidence.md). The complete approved
POST, receipt, status, and post-write validation are in
[`task-post-write-evidence.md`](task-post-write-evidence.md).

The execution artifacts are Markdown evidence ledgers. Both have a working
Markdown preview and download URL through `artifactVersionDetail`. No external
URL is present.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover and review one documented API connector | Pass |
| Keep connector setup read-only | Pass; no `/v1` setup call |
| Read six sources exactly once in order | Pass |
| Preserve source fields and `vehicle://` locators | Pass; complete ledger |
| Calculate due mileage and apply recall and option rules | Pass; Luau `all_pass: true` |
| Require approval before the write | Pass; task gate and governed action |
| Run the exact synthetic booking body once | Pass; submission count 1 |
| Verify receipt and status after the write | Pass; one read each |
| Keep the no-real-world-action boundary | Pass |

Temporary fixture and tunnel processes must be stopped after this evidence is
committed. Preserve the Go server, socket relay, and unrelated fixture
processes used by other cases.
