# PA-069 measured live ledger

This note is the acceptance evidence for the clean rerun. It uses the
read-only SQLite task-run records and the fresh fixture log. It does not use
the model's generated call-count summary as the source of truth.

## Fresh fixture

- Fixture process: `scripts/acceptance/run-mock-meal-planning-api.ts`
- Fixture version: `2026-09-09-meal-planning-api-v1`
- Rerun log: `/tmp/noema-pa069-fixture-rerun.log`
- Task: `task:bb27b21c5353c0ccf545fbfe06200ecb`
- Initial executor: `run:f09c2f97bf389f27be06067868211a4e`
- Resumed executor: `run:0989aafd515ff84164b2ad8c915139b8`
- Task-gate approval: `gate:eaab143374ab5b7052fbba704eccc2fb`
- Governed-action approval: `action:a0abc5fc9e28b131b1f4614eff91288b`

The fixture process was restarted before this task. Its only non-probe
requests were the nine calls listed below. `/health` was a startup check and
is not a task operation.

## Connector ledger

| Order | Task run | Operation | HTTP route | Result |
| ---: | --- | --- | --- | --- |
| 1 | `run:f09c2f97bf389f27be06067868211a4e` | `get_meal_profile` | GET `/v1/profile` | completed |
| 2 | `run:f09c2f97bf389f27be06067868211a4e` | `list_pantry_inventory` | GET `/v1/pantry` | completed |
| 3 | `run:f09c2f97bf389f27be06067868211a4e` | `list_meal_recipes` | GET `/v1/recipes` | completed |
| 4 | `run:f09c2f97bf389f27be06067868211a4e` | `get_meal_schedule` | GET `/v1/schedule` | completed |
| 5 | `run:f09c2f97bf389f27be06067868211a4e` | `list_grocery_prices` | GET `/v1/prices` | completed |
| 6 | `run:f09c2f97bf389f27be06067868211a4e` | `get_meal_constraints` | GET `/v1/constraints` | completed |
| 7 | `run:f09c2f97bf389f27be06067868211a4e` | `get_attendance_update` | GET `/v1/attendance-update` | completed |
| 8 | `run:0989aafd515ff84164b2ad8c915139b8` | `create_mock_grocery_cart` | POST `/v1/cart` | completed after approval |
| 9 | `run:0989aafd515ff84164b2ad8c915139b8` | `get_mock_cart` | GET `/v1/cart` | completed |

The task-run database contains exactly these nine connector tool calls. The
fixture log contains the same nine `/v1` requests, in the same order. The POST
body matched the approved body in the task plan. The response returned
`mock-cart-001`, `prepared_in_synthetic_cart`, and `submission_count: 1`.

## Source and planning checks

The seven source records were preserved in the task artifact
`artifact:949f7c8599e30abe124d290f1bb1765e`, version
`artifact_version:b1b1ed2c4676b8cda14099f0e7475436`. The artifact contains the
source facts, every listed locator, the exact safe recipe set, rejected
allergens, nine package IDs, and the Luau arithmetic result.

The final receipt artifact is
`artifact:2b2886cd2fd3a47c7b6a7252d2a75622`, version
`artifact_version:f28921db73222aec7a528797ae16ac98`. Its receipt fields are
correct, but its generated call-count paragraph incorrectly says that the
ledger contains 16 calls. That paragraph is superseded by the direct task-run
and fixture records above; no connector repeat appears in either record.

## Acceptance decision

PA-069 passes the functional acceptance criteria. The clean rerun proves the
seven ordered reads, exact approved synthetic write, one post-write receipt
read, safe planning rules, and no-real-transaction boundary. The reviewer
rejected the task because its generated summary miscounted the ledger. The
operator stopped the automatic retry after independently verifying the
authoritative records, so no extra connector call occurred.
