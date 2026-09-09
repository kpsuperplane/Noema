# PA-067 create the final home-repair brief

Create exactly one local Markdown artifact named
`synthetic-home-repair-final.md`. This is a document task only. Do not use the
home-repair connector, create a connection, create a shared workspace, or call
any external service.

## Required content

Write a concise but complete decision record for Jordan Lee's synthetic main
water-line repair. Label every record and every result as synthetic. State the
boundary: no contractor contact, real repair authorization, home entry,
insurance binding, charge, invoice payment, bank transfer, or money movement.

Preserve the current date 2026-09-09, timezone America/Los_Angeles, property
`Jordan's synthetic home`, project `Main water-line repair`, deadline
2026-09-30, and budget limit 10000 USD. Include source locator
`home://profiles/home-repair-001`.

Include all three bids, not only the selected one:

| Bid | Vendor | Cost components | Insurance and schedule | Decision |
| --- | --- | --- | --- | --- |
| bid-001 | Apex Home Repair (synthetic) | 8200 USD base + 450 USD disposal; permit included; normalized 8650 USD | valid through 2026-09-18; 2026-09-14 09:00 to 2026-09-18 17:00 local | selected |
| bid-002 | Cedar Works (synthetic) | 9150 USD with disposal and permit included; normalized 9150 USD | valid through 2027-06-30; 2026-09-16 13:00 to 2026-09-19 local | alternate |
| bid-003 | Budget Repair (synthetic) | 7600 USD base + 500 USD permit; normalized 8100 USD | insurance expired 2026-08-31; 2026-09-12 10:00 to 2026-09-15 local | rejected |

Preserve locators `home://bids/bid-001`, `home://bids/bid-002`, and
`home://bids/bid-003`. Explain that bid-003 was rejected for expired insurance,
even though its normalized cost was lowest. Preserve the scope locator
`home://scope/scope-001`, including included work (replace 12 feet of copper
line, patch drywall, pressure-test, and leave the area safe), excluded cosmetic
paint and unrelated plumbing defects, and the completion standard.

Preserve the policy and constraints. The synthetic policy is
`home-policy-001` from Harborline Home (synthetic), active through 2026-09-18,
with locator `home://insurance/home-policy-001`. The documented extension is
`insurance-extension-001`, effective 2026-09-19 through 2027-09-18, premium 280
USD, status available for approval before the write, and locator
`home://insurance-extensions/insurance-extension-001`. State that the changed
project end requires this extension. Include access weekdays 09:00-17:00,
quiet hours 20:00-08:00, no-work date 2026-09-13, the synthetic family-access
reason, the 2026-09-30 deadline, the all-in budget rule, and locator
`home://constraints/home-repair-001`.

Preserve the initial plan `repair-plan-001`, status
`ready_for_bid_selection`, and locator `home://plans/repair-plan-001`.

## Approved synthetic records

Record that bid-001 was accepted after a distinct approval. Preserve booking
`booking-001`, normalized amount 8650 USD, schedule, scope-001, submission
count 1, and locator `home://bid-acceptances/booking-001`. State that the exact
approval note prohibited contractor contact and real repair authorization.

Record the post-acceptance change read. Change `change-001` describes hidden
water damage, adds six feet of line and mold-resistant drywall, adds 900 USD and
two work days, and proposes 2026-09-22. It required a separate approval and was
approved once with locator `home://changes/change-001` and submission count 1.

Record the insurance extension approval separately. Preserve the 280 USD
premium, dates, policy and extension IDs, status
`extended_in_synthetic_project`, submission count 1, and locator
`home://insurance-extensions/insurance-extension-001`.

Record completion separately. Preserve completion `completion-001`, date
2026-09-22, evidence `completion-photo-set-001`, completed scope
`scope-001+change-001`, final contractor amount 9550 USD, status
`completed_in_synthetic_project`, submission count 1, and locator
`home://completions/completion-001`. State that this is not proof of a real home
repair.

Record payment separately. Preserve payment receipt `payment-001`, invoice
`invoice-001`, date 2026-09-23, amount 9550 USD, method
`synthetic_bank_transfer`, status `recorded_in_synthetic_ledger`, submission
count 1, and locator `home://payments/payment-001`. State that no money moved.

## Arithmetic and final status

Show the arithmetic explicitly:

- selected bid: 8200 + 450 = 8650 USD;
- contractor total after approved change: 8650 + 900 = 9550 USD;
- all-in total including insurance premium: 9550 + 280 = 9830 USD;
- budget remaining: 10000 - 9830 = 170 USD.

Preserve the one-time final status locator `home://status/home-repair-001` and
state that `get_repair_status` returned accepted bid-001, normalized bid total
8650 USD, approved change 900 USD, insurance premium 280 USD, contractor total
9550 USD, project total 9830 USD, budget remaining 170 USD, completion-001,
payment-001, and final status `paid_in_synthetic_ledger`.

Include a short approval timeline. It must show distinct approvals before bid
acceptance, scope change, insurance extension, completion evidence, and payment
receipt. Do not imply that the operator approved real-world actions.

## Acceptance criteria

- Exactly one local Markdown artifact is created with the requested filename.
- The artifact includes every bid, each cost component, every required scope and
  constraint, policy expiry and extension, all five approved synthetic records,
  arithmetic, final status, and every listed `home://` locator.
- The artifact states the synthetic-only boundary and does not claim real work,
  insurance, contact, payment, or money movement.
- The artifact task uses no connector or external service.
- The artifact is previewable as Markdown and receives an independent review.
