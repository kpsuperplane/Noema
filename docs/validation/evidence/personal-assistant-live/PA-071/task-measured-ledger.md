# PA-071 measured ledger

Measurements came from the live Go Noema development database and the clean
synthetic fixture process. Connector calls are counted independently from
planning, Luau, task-file, artifact, and review calls.

## Connector setup

| Item | Value |
| --- | --- |
| First setup task | `task:8921fbc1c2773ae374e548687473a938` |
| First setup result | Rejected response-size validation; no `/v1` call |
| Retry setup task | `task:d9aaeca8e00aa63d5e684dba0cec000f` |
| Retry planner | `run:11307b5a96b95b48dab2a34a1efdb620` |
| Retry executor | `run:9265cc7caac7ebaafb60de0769db87cd`, then `run:00bcb1d9a4793e75d02b87fec556559f` |
| Retry reviewer | `run:2a9f2609918e160c6cb483e73c0adfa2` |
| Pending proposal digest | `ea0f94cf5f0b7ef8ed2c3d1b6c65d5d9357567917eb841d0eb1e11b3f92d7bd1` |
| Accepted definition digest | `23681a58fb0ecba53e6dc60f6097809c07cb63c02f9c3e8464bb9709dad9dc0b` |
| Definition | `definition:synthetic_pet_care_api_v1` |
| Connection | `be6439434be1224531bd5150201025a5` (`personal-be643943`) |
| Connection revision / policy revision | `2 / 2` |
| Policies | `allow_automatically` reads; `always_ask` writes |

The setup retry opened the documentation, called the no-argument template
once, and submitted one proposal. The proposal contains the nine required
operations. It derives `transport_safe_read` for the eight GET operations and
`never` for the idempotent, non-read-only POST. It made no service-route call.

The first setup task failed because the initial generated response for
`list_pet_visit_options` could reach 33,918 bytes, above the 32 KiB limit. The
retry reduced only its generated bounds. The operator also resolved the live
documentation clarification by setting the exact-repeat POST to idempotent.

## Corrective execution attempt

| Item | Value |
| --- | --- |
| Task | `task:724875a4d99379bb30439ce837dfbf49` |
| Planner | `run:d4117d180bfc0241108571552418153c` |
| Executor | `run:3ec691cad2127485b6d231183e5f1ac0` |
| Outcome | Canceled before approval and before any write |

This attempt called the seven source operations once, but four generated
transforms did not match the fixture's original field names. The failed
operations were `get_vet_plans`, `list_pet_refills`,
`get_boarding_requirements`, and `list_pet_visit_options`. The fixture was
corrected to expose the documented compatibility names, then restarted before
the clean retry. No real service was contacted and no write occurred.

## Clean execution runs

| Run | Kind | Status | Tool calls |
| --- | --- | --- | ---: |
| `run:1d95c9949d7ec96f41d46bcaeb8ae42f` | planner | completed | 2 |
| `run:a55ede6f43108f513e29356a1bf3321b` | executor, before approval | completed | 10 |
| `run:67e8ab70e1d5a11f9f47d6390db674dc` | executor, after approval | completed | 8 |
| `run:6d7c643ea138f186588f533b432e52cd` | reviewer, first attempt | provider failed | 2 |
| `run:5f90ffed425b17e9f51a3cbcc3fde7b6` | reviewer, retry | completed | 2 |

The clean execution task was
`task:98f576813adeef778998e1ac3a739c32`. It reached Done at revision 16.
The task gate was `gate:df4028598192da777d1e2c20c0138793`. The governed action
was `action:0853acd5518c51a561f5a5fa954f06fd`, revision 1, and it succeeded
after explicit approval.

## Connector call sequence

The clean fixture process recorded exactly these nine service calls, in this
order:

| # | Operation | Method and route | Provider call ID |
| ---: | --- | --- | --- |
| 1 | `get_pet_profile` | GET `/v1/profile` | `call_kRjExnfx2u8k7MSrj4LQfE1E` |
| 2 | `list_pets` | GET `/v1/pets` | `call_j5kKaw8tkMdTLkiGt7VNMb9z` |
| 3 | `get_vet_plans` | GET `/v1/vet-plan` | `call_29C8Qe16N00jb3gi44M2aTGn` |
| 4 | `list_pet_refills` | GET `/v1/refills` | `call_OCOeR2WbS7nMoDPRDXFy4ewq` |
| 5 | `get_boarding_requirements` | GET `/v1/boarding-requirements` | `call_fohDc2qfRmzCyP1CRAVnk89P` |
| 6 | `get_pet_travel_plan` | GET `/v1/travel` | `call_LfNQ1bw2IJXgRUYafg6RnOp5` |
| 7 | `list_pet_visit_options` | GET `/v1/visit-options` | `call_DTWAc1P1DcLea5mj7DrAHHuH` |
| 8 | `schedule_pet_care` | POST `/v1/care-actions` | `call_9y7BfLRTDqoV9DIdGIHlAfBf` |
| 9 | `get_pet_care_confirmations` | GET `/v1/confirmations` | `call_EVteblIqCZPCxEGA6oPDYu76` |

The fixture also received health, documentation, root, and favicon probes.
Those are outside the nine service calls. No source operation or POST was
repeated after approval.

## Calculation and artifacts

- Pre-write Luau call: `call_VqepcWls5vgKh1DYx9tCM5Hn`. It returned
  `all_checks_passed=true`, `exact_body_valid=true`,
  `identities_distinct=true`, selected options `[option-001, option-003]`,
  and selected refill `refill-001`.
- Post-write Luau call: `call_MMDd21LqnoDIwpU6XV8Sxbci`, returning
  `confirmation_valid=true`, both stable IDs, all expected statuses, the exact
  dose and quantity, locator, and `submission_count=1`.

| Artifact | Version | Media / preview | Bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| `artifact:b5c2aaecd56fafad902b4a451bd58199` setup proposal receipt | `artifact_version:5f6e6e0746a2354d0b07c25c844e25df` | JSON | 854 | `f11de49d4de1d7b8e1c07efe3721aa5214b6552589ebb64d6b18d3732463670c` |
| `artifact:8f8a71e7d51c251d37a65e12aff93aee` execution evidence | `artifact_version:10c6dca80412ebd99870609f0944f1b8` | Markdown / MARKDOWN | 3,227 | `83b82e1226063374ae09131e860ffdc5c17574aacbe43ed2508363bf17e48521` |

The execution artifact was checked through `artifactVersionDetail`. It has a
working Markdown preview and download URL
`/artifacts/versions/10c6dca80412ebd99870609f0944f1b8/download`. No external
URL is present.
