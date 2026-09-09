# PA-069 clean rerun: plan the synthetic weekly meals

Use the already reviewed `synthetic_meal_planning_api_v7` connection in the
personal workspace. The service is a local synthetic fixture reached through
its reviewed connector. Do not contact a real household, expose a real
allergy, place an order, request delivery, authorize a charge, make a payment,
or move money.

## Required service sequence

This is a fresh task and a fresh fixture process. Count only calls made by this
task. Do not use or repeat any call from another task or run.

Call these seven read operations exactly once, in this order:

1. `get_meal_profile`
2. `list_pantry_inventory`
3. `list_meal_recipes`
4. `get_meal_schedule`
5. `list_grocery_prices`
6. `get_meal_constraints`
7. `get_attendance_update`

Keep every returned field and every `meal://` locator. Do not call another
service operation while planning. Use only the returned fixture data.

## Planning rules

Use `code.run_luau` for arithmetic and selection checks. Reject recipes whose
allergen list contains `peanuts` or `tree_nuts`. Select exactly
`recipe-001`, `recipe-002`, `recipe-003`, and `recipe-004`.

Preserve Tuesday September 15 and Saturday September 19 as dinners away.
Apply attendance update `attendance-001` before finalizing Thursday portions.
Preserve the Monday and Wednesday leftovers. Use Friday's four lentil-curry
leftovers on Sunday. Select these nine quantity-one package IDs:

```text
grocery-chicken
grocery-potatoes
grocery-broccoli
grocery-romaine
grocery-tomatoes
grocery-cheese
grocery-cilantro
grocery-coconut-milk
grocery-carrots
```

The package total is 39.50 USD. The budget limit is 90.00 USD. The remaining
budget is 50.50 USD. Keep the label-check requirement for every package.

## Approval and write

Present the final plan and this exact cart body. Ask for human approval before
any write. If the task resumes after that approval, do not repeat the seven
reads or planning calls. Issue only the pending approved write.

After approval, call `create_mock_grocery_cart` exactly once with:

```json
{
  "case_id": "meal-planning-001",
  "week_start": "2026-09-14",
  "selected_recipe_ids": ["recipe-001", "recipe-002", "recipe-003", "recipe-004"],
  "attendance_update_id": "attendance-001",
  "items": ["grocery-chicken", "grocery-potatoes", "grocery-broccoli", "grocery-romaine", "grocery-tomatoes", "grocery-cheese", "grocery-cilantro", "grocery-coconut-milk", "grocery-carrots"],
  "estimated_total_usd": 39.5,
  "budget_limit_usd": 90,
  "budget_remaining_usd": 50.5,
  "allergy_note": "Synthetic labels checked for peanuts and tree nuts; no unsafe item selected.",
  "leftover_plan": "Monday pasta leftovers are optional lunch portions; Wednesday sheet-pan leftovers are reserved; Friday lentil curry provides Sunday's two servings.",
  "approval_note": "Synthetic cart only; no grocery order or payment."
}
```

The POST prepares a synthetic cart only. If the connector asks for a separate
governed-action approval, approve only this exact body and then continue.
After the POST succeeds, call `get_mock_cart` exactly once. Do not issue any
other connector call.

## Acceptance criteria

- Seven reads occur once each, in the listed order.
- The exact POST occurs once after approval.
- The exact post-write GET occurs once after the POST.
- The task's connector ledger has exactly nine calls: seven reads, one POST,
  and one GET, in that order.
- The receipt is `mock-cart-001`, status `prepared_in_synthetic_cart`, and
  submission count one. It has the four selected recipes, attendance update,
  nine package IDs, both totals, both safety notes, and its meal locator.
- The final plan preserves allergen filtering, pantry-first selection, away
  dates, attendance, leftovers, and arithmetic.
- No real account, order, delivery, charge, payment, or money movement occurs.

Do not rewrite the result to hide a failed criterion. Report the measured call
ledger and stop after `get_mock_cart`.
