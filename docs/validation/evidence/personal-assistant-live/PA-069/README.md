# PA-069 meal planning and a synthetic grocery cart

Verdict: Pass after a reviewed API connector, seven ordered source reads,
Luau checks, approval-gated synthetic cart preparation, one receipt read, and
direct ledger verification.

This case used the live Go Noema development instance. The meal data, people,
allergy rules, prices, cart, and service were synthetic. No real household,
grocery account, order, delivery, charge, payment, or money movement was used.

## Case and fixture

- Case: `meal-planning-001`
- Fixture: `2026-09-09-meal-planning-api-v1`
- Fixture source:
  [`run-mock-meal-planning-api.ts`](../../../../../scripts/acceptance/run-mock-meal-planning-api.ts)
- Origin: `https://funny-cove-marilyn-activated.trycloudflare.com/`
- Documentation: `https://funny-cove-marilyn-activated.trycloudflare.com/docs`
- Reviewed definition: `definition:synthetic_meal_planning_api_v7`
- Accepted semantic digest:
  `4e28f4de488a50643036e4971005aad702e5d7cc951eb0eb048f71d60bc44f91`
- Active connection: `545d2bc8764b24922882392ae0a3133d`
  (`personal-545d2bc8`)
- Connection revision: 2; policy revision: 2
- Policies: automatic reads and always-ask writes

The fixture exposes six source reads, one attendance read, one synthetic cart
write, and one cart receipt read. It cannot contact a real service or perform
a real purchase.

## Connector setup

Setup opened the public documentation, called the no-argument
`adapter.definition_template` once, and proposed one definition. Earlier
proposals exceeded the platform response limit. The accepted v7 proposal uses
compact bounded schemas and exactly nine operations. Setup made no `/v1`
request and no state-changing request.

| Operation | Method and route | Behavior |
| --- | --- | --- |
| `get_meal_profile` | GET `/v1/profile` | automatic read |
| `list_pantry_inventory` | GET `/v1/pantry` | automatic read |
| `list_meal_recipes` | GET `/v1/recipes` | automatic read |
| `get_meal_schedule` | GET `/v1/schedule` | automatic read |
| `list_grocery_prices` | GET `/v1/prices` | automatic read |
| `get_meal_constraints` | GET `/v1/constraints` | automatic read |
| `get_attendance_update` | GET `/v1/attendance-update` | automatic read |
| `create_mock_grocery_cart` | POST `/v1/cart` | always-ask synthetic write |
| `get_mock_cart` | GET `/v1/cart` | automatic read |

The reviewed manifest uses bounded JSON projections. Recipe ingredients remain
structured objects. The cart `items` argument is a string array of package
IDs, with quantity one supplied by each price record.

## Execution

The clean rerun task was `task:bb27b21c5353c0ccf545fbfe06200ecb`. The planner
was `run:5a51c923aaaf93ec12984b20c20eab39`. The initial executor was
`run:f09c2f97bf389f27be06067868211a4e`; the approval continuation was
`run:0989aafd515ff84164b2ad8c915139b8`.

Noema read the seven sources in the required order. Luau verified the exact
four-recipe safe set, rejected the peanut and tree-nut recipes, preserved both
away dates, applied the Thursday attendance change, checked leftovers, and
calculated 39.50 USD total with 50.50 USD remaining from a 90.00 USD budget.

Noema opened task gate `gate:eaab143374ab5b7052fbba704eccc2fb`. The operator
approved the synthetic-only cart. Governed action
`action:a0abc5fc9e28b131b1f4614eff91288b` then succeeded once with the exact
body in [`task-plan-meal-planning-rerun.md`](task-plan-meal-planning-rerun.md).
The receipt returned cart `mock-cart-001`, status
`prepared_in_synthetic_cart`, and `submission_count: 1`. It matched the
approved recipes, attendance update, nine package IDs, budget values, safety
note, leftover note, and `meal://carts/mock-cart-001` locator.

## Measured ledger

The authoritative task-run database and fresh fixture log contain exactly nine
task calls: seven reads, one approved POST, and one post-write GET. The full
ledger and source evidence are in
[`task-measured-ledger.md`](task-measured-ledger.md).

The pre-write source artifact is
`artifact:949f7c8599e30abe124d290f1bb1765e`, version
`artifact_version:b1b1ed2c4676b8cda14099f0e7475436`. It preserves the seven
source summaries, all required locators, and the Luau result.

The execution also created
`artifact:2b2886cd2fd3a47c7b6a7252d2a75622`, version
`artifact_version:f28921db73222aec7a528797ae16ac98`. Its receipt is correct,
but its generated paragraph miscounts the connector ledger as 16 calls. The
direct task-run and fixture records are the acceptance authority and show nine
calls. The automatic retry was stopped after that read-only verification, so
it made no additional connector call.

## Acceptance

| Criterion | Result |
| --- | --- |
| Discover and review one documented API connector | Pass |
| Keep setup read-only | Pass |
| Read every source exactly once in order | Pass; seven calls |
| Preserve allergy, pantry, away-date, attendance, and leftover rules | Pass |
| Reproduce budget arithmetic with Luau | Pass; 39.50 USD and 50.50 USD |
| Require approval before the write | Pass; task gate and governed action |
| Run the exact synthetic cart body once | Pass; one POST, submission count 1 |
| Verify the receipt after the write | Pass; one GET, matching receipt |
| Keep the no-real-transaction boundary | Pass |

Temporary fixture and tunnel processes remain available while the next case is
prepared. They must be stopped and their temporary host entry removed after
the final PA-069 evidence is committed.
