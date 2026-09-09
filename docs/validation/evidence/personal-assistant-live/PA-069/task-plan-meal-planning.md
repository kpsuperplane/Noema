# PA-069 plan the synthetic weekly meals

Use the reviewed `synthetic_meal_planning_api_v7` connection in the personal
workspace. The service and every write are synthetic. Do not contact a real
household, expose a real allergy, place a grocery order, request delivery,
authorize a charge, make a payment, or move money.

## Source review

Read each initial source operation exactly once, in this order:

1. `get_meal_profile`
2. `list_pantry_inventory`
3. `list_meal_recipes`
4. `get_meal_schedule`
5. `list_grocery_prices`
6. `get_meal_constraints`

Keep every returned field and every `meal://` source locator. Do not use
outside facts. The fixture case is `meal-planning-001`, the week is
2026-09-14 through 2026-09-20, and the budget is 90 USD.

After those six reads, call `get_attendance_update` exactly once. Keep its
effective date, update ID, previous diner count, new diner count, reason, and
both locators. It changes Thursday September 17 from two diners to four.

## Plan and calculation

Use `code.run_luau` for the arithmetic and selection checks. Reject any recipe
whose allergen list contains `peanuts` or `tree_nuts`. The selected safe set
must be exactly `recipe-001`, `recipe-002`, `recipe-003`, and `recipe-004`.
Preserve Tuesday September 15 and Saturday September 19 as dinners away.
Apply the attendance update to Thursday before finalizing portions. Preserve
Monday and Wednesday leftovers and use Friday's four lentil-curry leftovers on
Sunday.

Select these nine safe grocery package IDs from the price record. Each has
documented quantity one:

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

The package total is `12 + 4 + 3 + 3 + 5 + 5 + 2 + 2.50 + 3 = 39.50` USD.
The remaining budget is `90.00 - 39.50 = 50.50` USD. Keep the label-check
requirement for every package. Do not select the peanut sauce or pesto.

Present the final plan and the exact synthetic cart body. Ask for human
approval before the first write. After approval, call
`create_mock_grocery_cart` exactly once with these exact arguments:

```json
{
  "case_id": "meal-planning-001",
  "week_start": "2026-09-14",
  "selected_recipe_ids": ["recipe-001", "recipe-002", "recipe-003", "recipe-004"],
  "attendance_update_id": "attendance-001",
  "items": [
    "grocery-chicken",
    "grocery-potatoes",
    "grocery-broccoli",
    "grocery-romaine",
    "grocery-tomatoes",
    "grocery-cheese",
    "grocery-cilantro",
    "grocery-coconut-milk",
    "grocery-carrots"
  ],
  "estimated_total_usd": 39.5,
  "budget_limit_usd": 90,
  "budget_remaining_usd": 50.5,
  "allergy_note": "Synthetic labels checked for peanuts and tree nuts; no unsafe item selected.",
  "leftover_plan": "Monday pasta leftovers are optional lunch portions; Wednesday sheet-pan leftovers are reserved; Friday lentil curry provides Sunday's two servings.",
  "approval_note": "Synthetic cart only; no grocery order or payment."
}
```

The operation prepares a synthetic cart only. It must never be described as a
purchase, delivery, or payment. After the POST succeeds, call `get_mock_cart`
exactly once. Confirm `mock-cart-001`, status
`prepared_in_synthetic_cart`, submission count one, the same four recipes, the
attendance update, the nine package IDs, both totals, both safety notes, and
`meal://carts/mock-cart-001`.

## Acceptance criteria

- The six initial reads occur once each, in the listed order.
- The attendance update read occurs once after the initial six reads.
- All six recipes, all pantry records, all seven schedule days, all eleven
  price records, all constraints, and all source locators remain available for
  the decision.
- The two allergen-bearing recipes and two unsafe grocery alternatives are
  rejected.
- The away dates, pantry-first rule, leftover schedule, and Thursday change
  are reflected in the final plan.
- The calculated total is 39.50 USD and the remaining budget is 50.50 USD.
- The cart POST is shown behind approval and runs once with the exact body.
- The cart GET runs once after the POST and verifies the returned receipt.
- The fixture records exactly nine service calls: seven reads, one POST, and
  one post-write read, in that order.
- No real household, allergy, grocery account, order, delivery, charge,
  payment, or money movement is used. No shared workspace is created.
