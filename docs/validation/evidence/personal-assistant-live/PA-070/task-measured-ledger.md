# PA-070 measured ledger

All measurements below came from the live Go Noema development database and
the synthetic fixture log. The connector calls are counted independently from
planning, Luau, task-file, artifact, and review calls.

## Connector setup

| Item | Value |
| --- | --- |
| Setup task | `task:1cb68b0ca4b21f229c083d3801971961` |
| Planner run | `run:30124a8c8e7587b7784cc11310a0a6be` |
| Executor run | `run:c9cd9342c50d6549e04cf6eb730ab5db` |
| Reviewer run | `run:b5cbf0bd75f1d6b38a79518cd09ae0aa` |
| Pending proposal digest | `e047bf5617a68658c68327acdc5dd98958ea155c748bff6e806ba6d63848c3a7` |
| Accepted definition digest | `3aa04993ff3dd40a7886f5f87679c95345199d427e7ef871036a7f08b40ed02a` |
| Definition | `definition:synthetic_vehicle_lifecycle_api_v1` |
| Connection | `e4ed0f7af9753be572b95c86ef87fd80` (`personal-e4ed0f7a`) |
| Connection revision / policy revision | `2 / 2` |
| Policies | `allow_automatically` reads; `always_ask` writes |

The setup executor made one documentation open, one no-argument template
call, and one proposal call. It made no `/v1` request. Two earlier setup
attempts were canceled after input validation errors; they also made no
`/v1` request. The accepted definition has nine operations.

## Execution runs

| Run | Kind | Status | Tool calls |
| --- | --- | --- | ---: |
| `run:42ec01a12d4760eafd69b6e436c80b5f` | planner | completed | 2 |
| `run:95b9c80557960a5fa4e36b2f1d35ef6e` | executor, before approval | completed | 13 |
| `run:83d92424d7faf58d76c0d17354c0d4d2` | executor, after approval | completed | 9 |
| `run:c6be6a22b5adcfd13760a9d8276207b2` | reviewer | completed | 3 |

The execution task was `task:2897d1caa7a12305beaa92469f260b76`. It reached
Done at revision 12. The task gate was
`gate:2fc8038e65154663682f8e267367d6a4`. The governed action was
`action:a592cf8c67bd7affd566758135c29f6c`, revision 1, and it succeeded after
the explicit approval.

## Vehicle-service call sequence

The fixture log contains exactly these nine `/v1` calls, in this order:

| # | Operation | Method and route | Call ID |
| ---: | --- | --- | --- |
| 1 | `get_vehicle_profile` | GET `/v1/profile` | `call_wWu1H7CZh5VxfQGayPVpLu4O` |
| 2 | `list_vehicle_service_history` | GET `/v1/service-history` | `call_xxl95fKIToHBDkgKA37VRees` |
| 3 | `list_vehicle_recalls` | GET `/v1/recalls` | `call_JN8zila2C0RgEtL34T06ylQA` |
| 4 | `get_vehicle_registration` | GET `/v1/registration` | `call_L6LViskgDh1CPSCvoxDxqCuy` |
| 5 | `list_vehicle_service_options` | GET `/v1/service-options` | `call_zGfSLlqnE1XslvRSiHB3pmsw` |
| 6 | `get_vehicle_constraints` | GET `/v1/constraints` | `call_9tE0O7T5OiMHnvOxWHlFSzLh` |
| 7 | `book_vehicle_service` | POST `/v1/service-bookings` | `call_BK8UyBkMZaD69JUXodG3SZG4` |
| 8 | `get_vehicle_service_receipt` | GET `/v1/service-receipt` | `call_5we2yr5aIx8qDnhQSVnVHmoV` |
| 9 | `get_vehicle_status` | GET `/v1/status` | `call_eSh6VtaqStkRL3BxZKTGMUnj` |

The fixture also logged health and documentation probes during setup. Those
are not vehicle-service calls and are excluded from the nine-call count.

## Calculation calls

- Pre-write Luau call: `call_lXuYf1jvkvrG35hOrsW2JOsk`. It confirmed due
  mileage 48,620, the date window, the 500 USD limit, matching recall
  `RC-2026-04`, and selection/rejection reasons for all three options.
- Post-write Luau call: `call_y2DRAQ3sfhhn04COOsnOQZBU`. It returned
  `all_pass: true` and confirmed the booking, receipt, status, one submission,
  and the 5,000-mile interval.

## Persisted artifacts

| Artifact | Version | Media / preview | Bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| `artifact:19169698fba07927e7ed272e88a3396f` setup receipt | `artifact_version:2ebf818de6a3b1ee37746912c9e3a925` | Markdown / MARKDOWN | 800 | `27e6f26a1bcb216641b6578e506a08e3228efccb3d3e1d9dfadd8df7dd80e249` |
| `artifact:af556a2f1e935c5e3a8012aa5a3ea025` initial evidence | `artifact_version:2dbb7b83d930ea5809c9850efed97d76` | Markdown / MARKDOWN | 5,810 | `9fac6ed57b3759f5395bac5b2e82b3fe5a606cf80b13ac223c05d1ee3fcd0cf7` |
| `artifact:cf4338c764379359828c6a486b3df603` post-write evidence | `artifact_version:7ed99743203110e275ab94acd5aa4894` | Markdown / MARKDOWN | 2,809 | `a05511b3e5fbee95d1d27c47dab7431098a6deb5158e9037c26a85d1fbd13d7c` |

The two execution artifacts were inspected through
`artifactVersionDetail`. Both expose a Markdown preview and a download URL.
No external URL was present.
