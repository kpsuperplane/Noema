#!/usr/bin/env bun

// Synthetic meal-planning and grocery service for PA-069.
// It cannot place a real order, expose a real household, or move money.

const port = Number(process.env.NOEMA_MEAL_PLANNING_PORT ?? "3791");
const fixtureVersion = "2026-09-09-meal-planning-api-v1";

const profile = {
  case_id: "meal-planning-001",
  fixture_version: fixtureVersion,
  owner_label: "Jordan Lee (synthetic)",
  week_start: "2026-09-14",
  week_end: "2026-09-20",
  household_label: "Jordan and Casey (synthetic household)",
  goal: "Plan one week of dinners and a grocery cart that respects the nut allergy, uses pantry food and leftovers, and stays within the 90 USD budget.",
  decision_boundary: "Do not expose a real household or allergy, place a real grocery order, authorize a real charge, or move money.",
  allergy_rule: "Avoid peanuts and tree nuts. Check the label for every purchased item before serving.",
  budget_limit_usd: 90,
  source_locator: "meal://profiles/meal-planning-001",
};

const pantry = [
  { item_id: "pantry-pasta", name: "whole-wheat pasta", quantity: 16, unit: "oz", source_locator: "meal://pantry/pantry-pasta" },
  { item_id: "pantry-chickpeas", name: "chickpeas", quantity: 2, unit: "can", source_locator: "meal://pantry/pantry-chickpeas" },
  { item_id: "pantry-tomatoes", name: "diced tomatoes", quantity: 1, unit: "can", source_locator: "meal://pantry/pantry-tomatoes" },
  { item_id: "pantry-black-beans", name: "black beans", quantity: 1, unit: "can", source_locator: "meal://pantry/pantry-black-beans" },
  { item_id: "pantry-lentils", name: "brown lentils", quantity: 16, unit: "oz", source_locator: "meal://pantry/pantry-lentils" },
  { item_id: "pantry-rice", name: "long-grain rice", quantity: 32, unit: "oz", source_locator: "meal://pantry/pantry-rice" },
  { item_id: "pantry-tortillas", name: "corn tortillas", quantity: 8, unit: "each", source_locator: "meal://pantry/pantry-tortillas" },
  { item_id: "pantry-onion", name: "yellow onion", quantity: 1, unit: "each", source_locator: "meal://pantry/pantry-onion" },
  { item_id: "pantry-garlic", name: "garlic", quantity: 1, unit: "bulb", source_locator: "meal://pantry/pantry-garlic" },
  { item_id: "pantry-oil", name: "olive oil", quantity: 1, unit: "bottle", source_locator: "meal://pantry/pantry-oil" },
  { item_id: "pantry-spices", name: "dried herbs and spices", quantity: 1, unit: "set", source_locator: "meal://pantry/pantry-spices" },
];

const recipes = [
  {
    recipe_id: "recipe-001",
    name: "Tomato chickpea pasta",
    servings: 4,
    allergens: [],
    ingredients: [
      { item_id: "pantry-pasta", quantity: 16, unit: "oz" },
      { item_id: "pantry-chickpeas", quantity: 1, unit: "can" },
      { item_id: "pantry-tomatoes", quantity: 1, unit: "can" },
      { item_id: "pantry-onion", quantity: 0.5, unit: "each" },
      { item_id: "pantry-garlic", quantity: 0.25, unit: "bulb" },
      { item_id: "pantry-oil", quantity: 1, unit: "portion" },
      { item_id: "pantry-spices", quantity: 1, unit: "portion" },
    ],
    preparation_note: "Cook four servings. Serve two and reserve two labelled nut-free leftovers.",
    source_locator: "meal://recipes/recipe-001",
  },
  {
    recipe_id: "recipe-002",
    name: "Sheet-pan chicken and vegetables",
    servings: 4,
    allergens: [],
    ingredients: [
      { item_id: "grocery-chicken", quantity: 1, unit: "package" },
      { item_id: "grocery-potatoes", quantity: 1, unit: "package" },
      { item_id: "grocery-broccoli", quantity: 1, unit: "package" },
      { item_id: "pantry-oil", quantity: 1, unit: "portion" },
      { item_id: "pantry-spices", quantity: 1, unit: "portion" },
    ],
    preparation_note: "Cook four servings. Serve two and reserve two labelled nut-free leftovers.",
    source_locator: "meal://recipes/recipe-002",
  },
  {
    recipe_id: "recipe-003",
    name: "Black bean tacos",
    servings: 4,
    allergens: [],
    ingredients: [
      { item_id: "pantry-tortillas", quantity: 8, unit: "each" },
      { item_id: "pantry-black-beans", quantity: 1, unit: "can" },
      { item_id: "grocery-romaine", quantity: 1, unit: "package" },
      { item_id: "grocery-tomatoes", quantity: 1, unit: "package" },
      { item_id: "grocery-cheese", quantity: 1, unit: "package" },
      { item_id: "grocery-cilantro", quantity: 1, unit: "package" },
    ],
    preparation_note: "The attendance update raises this meal from two to four diners; cook the full four-serving batch.",
    source_locator: "meal://recipes/recipe-003",
  },
  {
    recipe_id: "recipe-004",
    name: "Lentil curry with rice",
    servings: 6,
    allergens: [],
    ingredients: [
      { item_id: "pantry-lentils", quantity: 8, unit: "oz" },
      { item_id: "grocery-coconut-milk", quantity: 1, unit: "package" },
      { item_id: "grocery-carrots", quantity: 1, unit: "package" },
      { item_id: "pantry-onion", quantity: 0.5, unit: "each" },
      { item_id: "pantry-rice", quantity: 8, unit: "oz" },
      { item_id: "pantry-spices", quantity: 1, unit: "portion" },
    ],
    preparation_note: "Cook six servings on Friday. Serve two and reserve four for Sunday leftovers.",
    source_locator: "meal://recipes/recipe-004",
  },
  {
    recipe_id: "recipe-005",
    name: "Peanut satay noodles",
    servings: 4,
    allergens: ["peanuts"],
    ingredients: [
      { item_id: "pantry-pasta", quantity: 16, unit: "oz" },
      { item_id: "grocery-peanut-sauce", quantity: 1, unit: "package" },
    ],
    preparation_note: "Do not select for this household because it contains peanuts.",
    source_locator: "meal://recipes/recipe-005",
  },
  {
    recipe_id: "recipe-006",
    name: "Pesto pasta",
    servings: 4,
    allergens: ["tree_nuts"],
    ingredients: [
      { item_id: "pantry-pasta", quantity: 16, unit: "oz" },
      { item_id: "grocery-pesto", quantity: 1, unit: "package" },
    ],
    preparation_note: "Do not select for this household because it contains tree nuts.",
    source_locator: "meal://recipes/recipe-006",
  },
];

const schedule = {
  week_start: "2026-09-14",
  week_end: "2026-09-20",
  days: [
    { date: "2026-09-14", dinner_status: "home", diners: 2, recipe_id: "recipe-001", leftover_servings: 2, source_locator: "meal://schedule/2026-09-14" },
    { date: "2026-09-15", dinner_status: "away", diners: 0, recipe_id: "none", leftover_servings: 0, source_locator: "meal://schedule/2026-09-15" },
    { date: "2026-09-16", dinner_status: "home", diners: 2, recipe_id: "recipe-002", leftover_servings: 2, source_locator: "meal://schedule/2026-09-16" },
    { date: "2026-09-17", dinner_status: "home", diners: 2, recipe_id: "recipe-003", leftover_servings: 2, source_locator: "meal://schedule/2026-09-17" },
    { date: "2026-09-18", dinner_status: "home", diners: 2, recipe_id: "recipe-004", leftover_servings: 4, source_locator: "meal://schedule/2026-09-18" },
    { date: "2026-09-19", dinner_status: "away", diners: 0, recipe_id: "none", leftover_servings: 0, source_locator: "meal://schedule/2026-09-19" },
    { date: "2026-09-20", dinner_status: "home_leftovers", diners: 2, recipe_id: "recipe-004", leftover_servings: 0, source_locator: "meal://schedule/2026-09-20" },
  ],
  source_locator: "meal://schedule/week-2026-09-14",
};

const prices = [
  { item_id: "grocery-chicken", name: "boneless chicken thighs", package_size: "2 lb", quantity: 1, unit_price_usd: 12, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-chicken" },
  { item_id: "grocery-potatoes", name: "russet potatoes", package_size: "3 lb", quantity: 1, unit_price_usd: 4, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-potatoes" },
  { item_id: "grocery-broccoli", name: "broccoli", package_size: "1 lb", quantity: 1, unit_price_usd: 3, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-broccoli" },
  { item_id: "grocery-romaine", name: "romaine lettuce", package_size: "one head", quantity: 1, unit_price_usd: 3, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-romaine" },
  { item_id: "grocery-tomatoes", name: "fresh tomatoes", package_size: "2 lb", quantity: 1, unit_price_usd: 5, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-tomatoes" },
  { item_id: "grocery-cheese", name: "shredded cheese", package_size: "8 oz", quantity: 1, unit_price_usd: 5, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-cheese" },
  { item_id: "grocery-cilantro", name: "cilantro", package_size: "one bunch", quantity: 1, unit_price_usd: 2, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-cilantro" },
  { item_id: "grocery-coconut-milk", name: "coconut milk", package_size: "13.5 oz can", quantity: 1, unit_price_usd: 2.5, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-coconut-milk" },
  { item_id: "grocery-carrots", name: "carrots", package_size: "2 lb", quantity: 1, unit_price_usd: 3, allergen_statement: "none listed; check package label", label_check_required: true, source_locator: "meal://prices/grocery-carrots" },
  { item_id: "grocery-peanut-sauce", name: "peanut sauce", package_size: "one jar", quantity: 1, unit_price_usd: 4, allergen_statement: "contains peanuts", label_check_required: true, source_locator: "meal://prices/grocery-peanut-sauce" },
  { item_id: "grocery-pesto", name: "pesto", package_size: "one jar", quantity: 1, unit_price_usd: 4, allergen_statement: "contains tree nuts", label_check_required: true, source_locator: "meal://prices/grocery-pesto" },
];

const constraints = {
  budget_limit_usd: 90,
  allergen_exclusions: ["peanuts", "tree_nuts"],
  dinners_away_dates: ["2026-09-15", "2026-09-19"],
  pantry_first_rule: "Use pantry ingredients before buying groceries.",
  leftover_rule: "Cook planned batches and reserve leftovers for the scheduled meals.",
  label_check_rule: "Check every package label for peanuts and tree nuts before serving.",
  cart_only_boundary: "Prepare a synthetic cart only; do not place an order or move money.",
  week_start: "2026-09-14",
  week_end: "2026-09-20",
  source_locator: "meal://constraints/meal-planning-001",
};

const attendanceUpdate = {
  update_id: "attendance-001",
  effective_date: "2026-09-16",
  changes: [
    {
      date: "2026-09-17",
      previous_diners: 2,
      new_diners: 4,
      reason: "Two synthetic guests join Thursday dinner.",
      source_locator: "meal://attendance/2026-09-17",
    },
  ],
  source_locator: "meal://attendance/attendance-001",
};

let cart:
    | {
      cart_id: string;
      selected_recipe_ids: string[];
      attendance_update_id: string;
      items: string[];
      estimated_total_usd: number;
      budget_limit_usd: number;
      budget_remaining_usd: number;
      allergy_note: string;
      leftover_plan: string;
      status: string;
      submission_count: number;
      source_locator: string;
    }
  | undefined;

const requests: Array<{ method: string; path: string; body?: unknown }> = [];
const expectedApprovalNote = "Synthetic cart only; no grocery order or payment.";
const expectedAllergyNote = "Synthetic labels checked for peanuts and tree nuts; no unsafe item selected.";
const expectedLeftoverPlan = "Monday pasta leftovers are optional lunch portions; Wednesday sheet-pan leftovers are reserved; Friday lentil curry provides Sunday's two servings.";
const expectedItems = [
  { item_id: "grocery-chicken", quantity: 1 },
  { item_id: "grocery-potatoes", quantity: 1 },
  { item_id: "grocery-broccoli", quantity: 1 },
  { item_id: "grocery-romaine", quantity: 1 },
  { item_id: "grocery-tomatoes", quantity: 1 },
  { item_id: "grocery-cheese", quantity: 1 },
  { item_id: "grocery-cilantro", quantity: 1 },
  { item_id: "grocery-coconut-milk", quantity: 1 },
  { item_id: "grocery-carrots", quantity: 1 },
];
const expectedItemIds = expectedItems.map((item) => item.item_id);

const docs = `# Synthetic meal planning and grocery API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot place a grocery order,
authorize a charge, expose a real household or allergy, or move money. Work
only in one personal Noema workspace. The service has no routes beyond the
ones listed below.

## Source records

- GET /v1/profile returns the synthetic owner, week, household label, allergy
  rule, 90 USD budget, goal, decision boundary, and profile locator.
- GET /v1/pantry returns the complete pantry inventory and one locator per item.
- GET /v1/recipes returns six recipe records. Recipes 001 through 004 contain
  no listed nuts. Recipe-005 contains peanuts. Recipe-006 contains tree nuts.
  Each record includes servings, ingredients, allergen labels, preparation
  note, and source locator.
- GET /v1/schedule returns seven dated dinner records. Tuesday September 15
  and Saturday September 19 are dinners away. Friday's six-serving lentil
  curry provides four leftovers for Sunday.
- GET /v1/prices returns package sizes, prices, allergen statements, label-check
  requirement, and source locators. The nine safe grocery packages needed for
  the final plan cost 39.50 USD: 12 + 4 + 3 + 3 + 5 + 5 + 2 + 2.50 + 3.
  The peanut sauce and pesto are unsafe alternatives and must not be selected.
- GET /v1/constraints returns the 90 USD limit, peanut and tree-nut
  exclusions, two dinners away, pantry-first and leftover rules, label check,
  cart-only boundary, week range, and locator.

## Changing attendance

- GET /v1/attendance-update returns attendance-001. Thursday September 17
  changes from two to four diners because two synthetic guests join. Re-plan
  that meal for four servings. The update does not authorize a purchase.

## Synthetic cart

- POST /v1/cart accepts only the final safe recipe set, the attendance update,
  the nine safe grocery package identifiers, 39.50 USD, 50.50 USD remaining,
  the exact allergy note, the exact leftover note, and this exact approval note:
  ${expectedApprovalNote}
- The first accepted request returns mock-cart-001, status
  prepared_in_synthetic_cart, submission_count 1, and a cart locator. Repeating
  the exact request returns the same cart and keeps submission_count at one.
- GET /v1/cart returns 409 until the synthetic cart is recorded. Afterward it
  returns the cart, item list, total, budget remainder, allergy note, leftover
  plan, status, and locator. It never places an order.

There are no other routes. Do not propose a real grocery order, delivery,
charge, payment, or household action.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function sameItems(value: unknown) {
  if (!Array.isArray(value) || value.length !== expectedItemIds.length) return false;
  if (!value.every((item): item is string => typeof item === "string")) return false;
  return JSON.stringify([...value].sort()) === JSON.stringify([...expectedItemIds].sort());
}

function cartResult() {
  return cart;
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    let body: unknown;
    if (request.method !== "GET") body = await request.json().catch(() => undefined);
    const entry = {
      method: request.method,
      path: `${url.pathname}${url.search}`,
      ...(body === undefined ? {} : { body }),
    };
    requests.push(entry);
    console.log(JSON.stringify(entry));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "meal-planning", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/pantry") return json({ items: pantry });
    if (request.method === "GET" && url.pathname === "/v1/recipes") return json({ recipes });
    if (request.method === "GET" && url.pathname === "/v1/schedule") return json(schedule);
    if (request.method === "GET" && url.pathname === "/v1/prices") return json({ items: prices });
    if (request.method === "GET" && url.pathname === "/v1/constraints") return json(constraints);
    if (request.method === "GET" && url.pathname === "/v1/attendance-update") return json(attendanceUpdate);
    if (request.method === "POST" && url.pathname === "/v1/cart") {
      const value = (body ?? {}) as Record<string, unknown>;
      const selected = value.selected_recipe_ids;
      const selectedOkay = Array.isArray(selected)
        && selected.length === 4
        && new Set(selected).size === 4
        && ["recipe-001", "recipe-002", "recipe-003", "recipe-004"].every((id) => selected.includes(id));
      if (
        value.case_id !== profile.case_id
        || value.week_start !== profile.week_start
        || !selectedOkay
        || value.attendance_update_id !== attendanceUpdate.update_id
        || !sameItems(value.items)
        || value.estimated_total_usd !== 39.5
        || value.budget_limit_usd !== 90
        || value.budget_remaining_usd !== 50.5
        || value.allergy_note !== expectedAllergyNote
        || value.leftover_plan !== expectedLeftoverPlan
        || value.approval_note !== expectedApprovalNote
      ) return json({ error: "cart must use the exact safe recipes, packages, totals, and synthetic-only notes" }, 400);
      if (!cart) {
        cart = {
          cart_id: "mock-cart-001",
          selected_recipe_ids: selected as string[],
          attendance_update_id: attendanceUpdate.update_id,
          items: expectedItemIds,
          estimated_total_usd: 39.5,
          budget_limit_usd: 90,
          budget_remaining_usd: 50.5,
          allergy_note: expectedAllergyNote,
          leftover_plan: expectedLeftoverPlan,
          status: "prepared_in_synthetic_cart",
          submission_count: 1,
          source_locator: "meal://carts/mock-cart-001",
        };
      }
      return json(cartResult(), cart.submission_count === 1 ? 201 : 200);
    }
    if (request.method === "GET" && url.pathname === "/v1/cart") {
      if (!cart) return json({ error: "synthetic cart has not been prepared" }, 409);
      return json(cartResult());
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic meal-planning API listening on http://127.0.0.1:${server.port}`);
