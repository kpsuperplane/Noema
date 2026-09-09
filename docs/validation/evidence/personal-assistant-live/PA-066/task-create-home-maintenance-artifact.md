# PA-066 create the final maintenance artifact

Create exactly one local Markdown artifact named
`synthetic-home-maintenance-final.md`. Do not call the synthetic API or any
other connector. Use the verified facts below and the completed plan result
provided in this task context.

The artifact is a final human-readable record of one synthetic HVAC maintenance
review. It must state at the top that no technician was dispatched, no provider
or warranty desk was contacted, no repair was authorized, and no money was
charged.

## Required contents

Include all of these facts and source locators:

- Case `home-maintenance-001`; owner Jordan Lee (synthetic); property Jordan's
  synthetic home; current date 2026-09-09; timezone America/Los_Angeles.
- Asset `hvac-001`, Northwind QuietCool 24, installed 2023-09-12. The source
  did not return the serial number. Do not invent one.
- Manual `manual-hvac-001`: service every 180 days, filter every 90 days,
  recommended March and September, duration 90 minutes, and its safety note.
- Warranty `warranty-hvac-001`: exact serial match; 2023-09-12 through
  2027-09-12; annual tune-up labor and covered parts; replacement filters
  excluded.
- Baseline receipt `service-receipt-000`: 2026-03-12, 160 USD, synthetic
  seasonal tune-up, no open defect.
- Due-date arithmetic: 2026-03-12 plus 180 days equals 2026-09-08, one day
  before the fixture current date.
- Constraints: safe window 2026-09-12 through 2026-09-30; avoid dates
  2026-09-10 and 2026-09-11 for the synthetic heat advisory; quiet hours
  20:00-08:00; weekday access 09:00-17:00; planned-service budget 300 USD.
- Quote-001: CoolAir Service (synthetic), 2026-09-12 at 09:00, 90 minutes,
  185 USD, filter included, warranty eligible, feasible.
- Quote-002: Northwind Certified (synthetic), 2026-09-15 at 13:00, 90 minutes,
  260 USD, no filter, warranty eligible, feasible; alternate.
- Quote-003: Budget HVAC (synthetic), 2026-09-11 at 10:00, 140 USD, filter
  included, warranty unknown; rejected because it is outside the safe window.
- The source inconsistency: quote-001 is marked feasible even though
  2026-09-12 is a Saturday and access is described as weekdays. Future runs
  must re-read current records instead of carrying feasibility forward.
- The exact approved synthetic receipt: asset `hvac-001`, service date
  `2026-09-12`, vendor `CoolAir Service (synthetic)`, type
  `seasonal_tune_up`, amount 185, warranty `warranty-hvac-001`, and note
  `Synthetic service receipt only; no real dispatch or payment.`
- Receipt result: `service-receipt-001`, status
  `recorded_in_synthetic_maintenance`, submission count 1, next due
  `2027-03-11`; status read selected quote-001, amount 185, limit 300,
  remaining 115, and derived due `2026-09-08`.

Use these locators verbatim: `home://profiles/home-maintenance-001`,
`home://assets/hvac-001`, `home://manuals/manual-hvac-001`,
`home://warranties/warranty-hvac-001`,
`home://service-receipts/service-receipt-000`,
`home://constraints/home-maintenance-001`, `home://quotes/quote-001`,
`home://quotes/quote-002`, `home://quotes/quote-003`,
`home://plans/maintenance-plan-001`,
`home://service-receipts/service-receipt-001`, and
`home://status/home-maintenance-001`.

## Native recurring Task

Record that the operator materialized one native personal Task after the
verified result:

- task ID `task:12d0a7f7825f1d0c573daa73d6291e20`
- title `HVAC seasonal maintenance — synthetic home`
- recurrence ID `recurrence:3f1af04f7a0ab29b3aff3331c83f97e9`
- first run `2026-09-12T16:00:00Z`
- timezone `America/Los_Angeles`
- cron `0 9 12 3,9 *`
- overlap `SKIP`; missed-run `RUN_ONCE`; lifecycle `ACTIVE`
- next projected run `2027-03-12T17:00:00Z`

Explain that `0 9 12 3,9 *` explicitly means March and September 12. State
that each occurrence is a review trigger. A future run must re-read records,
check the weekday inconsistency and current constraints, then request approval
before any synthetic write.

## Acceptance criteria

- Exactly one Markdown artifact is created.
- The artifact contains every required fact, arithmetic result, selected and
  rejected quote, receipt result, budget result, recurring Task detail, and
  source locator.
- It clearly separates synthetic fixture results from real-world actions.
- It does not invent the unavailable serial number or claim executor-created
  recurrence.
