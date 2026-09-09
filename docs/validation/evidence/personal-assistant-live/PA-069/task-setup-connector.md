# PA-069 set up the synthetic meal-planning API

Use the live Go Noema development instance to inspect the public documentation
at the exact origin below and propose one API connector. This setup is
synthetic. Do not call a `/v1` route, place an order, authorize a charge, or
use a real household, allergy, grocery account, or address.

## Documentation source

- Adapter ID: `synthetic_meal_planning_api_v7`
- Definition ID: `definition:synthetic_meal_planning_api_v7`
- Display name: `Synthetic meal planning API`
- Origin: `https://funny-cove-marilyn-activated.trycloudflare.com/`
- Source reference: `https://funny-cove-marilyn-activated.trycloudflare.com/docs`
- Revision: `v1`
- Authentication: none
- Scope: one personal Noema workspace
- Fixture version: `2026-09-09-meal-planning-api-v1`

Read the documentation first. Use the returned page evidence directly. Do not
call `web.browse.switch_provider`; the configured browser provider is the
required source for this test. Then call `adapter.definition_template` once
with no arguments. After that call returns, do not call
`adapter.definition_template` again, even with operation IDs or a digest. Use
the first template response to build the proposal. Call
`adapter.propose_definition` exactly once. Do not retry or submit a second
proposal. Record the returned pending digest for operator review. The proposal
must be reviewed before any connector use.

## Required operations

The proposal must contain exactly these nine operations and no others:

1. `get_meal_profile`: GET `/v1/profile`; read-only, idempotent,
   non-destructive, closed-world. Preserve all profile fields, including the
   allergy rule, budget, week, goal, boundary, and `meal://profiles/...` locator.
2. `list_pantry_inventory`: GET `/v1/pantry`; read-only, idempotent,
   non-destructive, closed-world. Project `/items` as an array with at most 11
   items. Preserve item ID, name, quantity, unit, and source locator.
3. `list_meal_recipes`: GET `/v1/recipes`; read-only, idempotent,
   non-destructive, closed-world. Project `/recipes` as an array with at most 6
   items. Preserve recipe ID, name, servings, allergen array, ingredient
   array, preparation note, and source locator.
4. `get_meal_schedule`: GET `/v1/schedule`; read-only, idempotent,
   non-destructive, closed-world. Preserve week range, `/days` at most 7,
   dinner status, diners, recipe ID, leftover servings, and each locator.
5. `list_grocery_prices`: GET `/v1/prices`; read-only, idempotent,
   non-destructive, closed-world. Project `/items` as an array with at most 11
   items. Preserve item ID, name, package size, quantity, unit price, allergen
   statement, label-check requirement, and source locator.
6. `get_meal_constraints`: GET `/v1/constraints`; read-only, idempotent,
   non-destructive, closed-world. Preserve the 90 USD budget, both allergen
   exclusions, dinners-away dates, pantry-first and leftover rules, label-check
   rule, cart-only boundary, week range, and locator.
7. `get_attendance_update`: GET `/v1/attendance-update`; read-only,
   idempotent, non-destructive, closed-world. Preserve update ID, effective
   date, one change record, previous and new diners, reason, and locators.
8. `create_mock_grocery_cart`: POST `/v1/cart`; write, non-idempotent,
   non-destructive, open-world, `retry: never`, approval-gated. Accept JSON
   arguments `case_id` string, `week_start` string, `selected_recipe_ids`
   array, `attendance_update_id` string, `items` array, numeric
   `estimated_total_usd`, `budget_limit_usd`, `budget_remaining_usd`, string
   `allergy_note`, string `leftover_plan`, and string `approval_note`. The
   `items` argument is a string array of package IDs; each package quantity is
   the documented quantity from the price record (one in this fixture). The
   request must be described as a synthetic cart preparation only. It must
   never be described as a purchase or delivery.
9. `get_mock_cart`: GET `/v1/cart`; read-only, idempotent, non-destructive,
   closed-world. Preserve the cart ID, selected recipes, attendance update,
   item list, totals, allergy and leftover notes, status, submission count, and
   `meal://carts/...` locator.

Every operation must accept only `application/json`, use bounded response
projections, include explicit string and array limits, and use
`pagination: {kind: "none"}`. The definition must remain below the platform
schema-size limit. No operation may include an undocumented route or a real
grocery action.

### Compact response bounds

The response limit is computed from the largest permitted response. Use these
small bounds in the proposal. They cover every value in this fixture and keep
each operation below the 32 KiB platform limit. Do not use broad defaults such
as 512 or 1024 bytes for every string.

| Operation | Required response bounds |
| --- | --- |
| `get_meal_profile` | Strings: `case_id`, `fixture_version`, `week_start`, `week_end` 32 bytes; `owner_label`, `household_label` 96; `allergy_rule`, `goal`, `decision_boundary` 256; `source_locator` 96. |
| `list_pantry_inventory` | `maxItems: 11`; fields `item_id` 64, `name` 96, `unit` 24, `source_locator` 96 bytes. |
| `list_meal_recipes` | `recipes.maxItems: 6`; recipe `recipe_id` 24, `name` 64, `preparation_note` 128, `source_locator` 64; `allergens.maxItems: 2`, item strings 24; `ingredients.maxItems: 7`, each object has `item_id` 24, numeric `quantity`, and `unit` 12. |
| `get_meal_schedule` | `days.maxItems: 7`; `week_start` and `week_end` 32; day `date` 32, `dinner_status` 32, `recipe_id` 64, `source_locator` 96; top-level `source_locator` 96. |
| `list_grocery_prices` | `maxItems: 11`; fields `item_id` 32, `name` 64, `package_size` 32, `allergen_statement` 64, `source_locator` 64 bytes. |
| `get_meal_constraints` | `allergen_exclusions.maxItems: 2`, item strings 48; `dinners_away_dates.maxItems: 2`, item strings 32; `week_start` and `week_end` 32; each rule text 256; `source_locator` 96. |
| `get_attendance_update` | `changes.maxItems: 1`; `update_id` 64, `effective_date` 32; change `date` 32, `reason` 256, `source_locator` 96; top-level `source_locator` 96. |
| `create_mock_grocery_cart` and `get_mock_cart` | `selected_recipe_ids.maxItems: 6`, item strings 64; `items.maxItems: 11`, item strings 64; IDs and status 64; `allergy_note` and `leftover_plan` 256; `source_locator` 96. |

For custom schemas, express those limits with the exact camel-case keys below.
Use arrays of short strings only for the documented allergen and date arrays.
For recipes, preserve the documented ingredient objects with `item_id`,
`quantity`, and `unit`; do not flatten them to strings. The ingredient object
must set `additionalProperties: false` and require all three properties. Keep
required fields and `additionalProperties: false` at every object level.

When a custom response schema is needed, use the manifest's exact camel-case
keys: `additionalProperties`, `maxBytes`, `maxItems`, and `items`. Do not use
snake-case variants such as `additional_properties`, `max_bytes`, or
`max_items`. The custom schema must declare every required property and must
set `additionalProperties` to `false`.

Argument objects have a smaller fixed shape. They may contain only `name`,
`description`, `location`, `type`, `required`, and `enum_values`. Do not add
`max_bytes`, `max_items`, or other bound fields to an argument. Put response
limits in the response recipe or custom output schema instead.

## Acceptance criteria

- The documentation page is opened before the proposal is built.
- One no-argument `adapter.definition_template` call is made before the
  proposal. No second template call is made.
- Exactly one `adapter.propose_definition` call returns `review_required`.
- The proposal contains exactly the nine named operations and paths above.
- All GET operations are automatic reads. Only `create_mock_grocery_cart` is a
  write and it is approval-gated.
- Setup makes no `/v1` request and no state-changing request.
- The operator can accept the inspected pending digest and configure the
  resulting personal connection for automatic reads and always-ask writes.
