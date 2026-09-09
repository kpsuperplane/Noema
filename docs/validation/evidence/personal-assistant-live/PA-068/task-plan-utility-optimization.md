# PA-068 plan synthetic utility optimization

Use the reviewed `synthetic_utility_optimization_api` connection in the
personal workspace. The service and every write are synthetic. Do not contact
a real utility or internet provider, change a real service, expose a real
address, authorize a real charge, or move money.

## Source review

Read each initial source operation exactly once, in this order:

1. `get_utility_profile`
2. `list_utility_usage`
3. `list_utility_plans`
4. `get_current_utility_service`
5. `get_utility_constraints`

Keep every returned fact and every `utility://` source locator. Do not use
outside facts. The fixture current date is 2026-09-09, the timezone is
`America/Los_Angeles`, and the required switch date is 2026-10-01.

## Reasoning and recommendation

Use all twelve monthly usage records. Their total is 7210 kWh. Compare all
three plans across the full twelve-month period, including introductory rates,
standard rates, base charges, equipment charges, reliability, data caps, and
billing notes.

- `plan-001` CivicGrid Secure + Fiber (synthetic): electricity is
  `7210 * 0.19 + 22 * 12 = 1633.90` USD. Internet is
  `45 * 3 + 70 * 9 = 765.00` USD. The annual new-plan total is `2398.90` USD.
  It meets the 99.95% uptime minimum, has no data cap, and has predictable
  billing.
- `plan-002` BudgetSpark + StreamNet (synthetic): electricity is
  `7210 * 0.15 + 18 * 12 = 1297.50` USD. Internet is
  `35 * 6 + 60 * 6 = 570.00` USD. Adding the 180 USD equipment charge gives
  `2047.50` USD. Reject it because uptime is 99.5% and the plan has a
  1000 GB monthly data cap.
- `plan-003` Cobalt Flex + FiberPlus (synthetic): electricity is
  `7210 * 0.17 + 25 * 12 = 1525.70` USD. Internet is `65 * 12 = 780.00` USD.
  Adding the 75 USD equipment charge gives `2380.70` USD. Reject it because
  uptime is 99.9%, below the required 99.95%, even though it has no cap.

Read the current service before recommending a switch. Its projected final
old-provider bill is `48.50 + 120.00 = 168.50` USD. Add that bill to the
selected plan total. The synthetic switch total is
`2398.90 + 168.50 = 2567.40` USD, which is within the 2600 USD budget and
leaves 32.60 USD.

Present only `plan-001` as the recommended synthetic switch. Ask for human
approval before the first write. After approval, call `switch_utility_bundle`
exactly once with these exact arguments:

```json
{
  "case_id": "utility-optimization-001",
  "selected_plan_id": "plan-001",
  "switch_date": "2026-10-01",
  "annual_new_plan_total_usd": 2398.9,
  "old_provider_final_bill_usd": 168.5,
  "annual_switch_total_usd": 2567.4,
  "approval_note": "Synthetic switch only; no provider contact or real service change."
}
```

Do not claim that a utility or internet provider was contacted or that a real
service changed. After the write, call `get_old_provider_final_bill` exactly
once. It must return `final-bill-001`, 48.50 USD prorated service, a 120 USD
early-exit fee, a 168.50 USD final amount, and
`finalized_in_synthetic_ledger`. Then call `get_utility_status` exactly once.
It must report `activation-001`, `plan-001`, the 2026-10-01 activation date,
2398.90 USD annual new-plan cost, 168.50 USD old-provider final bill, 2567.40
USD annual switch total, 32.60 USD remaining, one submission, and
`activated_in_synthetic_account`.

## Acceptance criteria

- The five initial source reads occur once each, in the listed order.
- The comparison preserves all twelve usage records, all three plans, every
  pricing component, introductory expiry, equipment charge, reliability,
  data-cap, billing, current-service, constraint, and source-locator field.
- The recommendation preserves the reliability preference and rejects plans
  that fail it, even when their price is lower.
- The selected synthetic switch is shown behind approval before the write.
- The synthetic POST operation runs once with the exact approved arguments.
- The old-provider final-bill read runs once after the write.
- The final status read runs once and reports the documented activation and
  arithmetic.
- The fixture records exactly eight service calls: five initial reads, one
  switch POST, one final-bill read, and one status read, in that order.
- No real utility, internet provider, address, charge, payment, or money
  movement occurs. No shared workspace is created.
