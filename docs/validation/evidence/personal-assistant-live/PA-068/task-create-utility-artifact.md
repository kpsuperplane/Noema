# PA-068 create final utility comparison artifact

Create exactly one final Markdown artifact named
`synthetic-utility-optimization-final.md` in the personal workspace. This is a
document-only task. Do not call the utility connector or any external service.
Use the completed execution result and its evidence record as the source.

The artifact must state that the case is synthetic and that no real utility or
internet provider was contacted, no real service or address was changed, and
no charge, payment, or money movement occurred.

Include all of these facts and source locators:

- Profile: `utility-optimization-001`; owner Jordan Lee (synthetic); project
  Electricity and internet plan switch; current date 2026-09-09; timezone
  `America/Los_Angeles`; service address label Jordan's synthetic home; budget
  2600 USD; required switch date 2026-10-01; the full goal, reliability
  preference, decision boundary, and
  `utility://profiles/utility-optimization-001`.
- Usage: all twelve months from 2025-10 through 2026-09 with values
  540, 590, 640, 650, 610, 580, 520, 500, 560, 690, 710, and 620 kWh,
  preserving each `utility://usage/<month>` locator and the total 7210 kWh.
- Constraints: 99.95% minimum uptime, no data cap, predictable billing,
  12-month comparison, kWh unit, 2600 USD budget, 2026-10-01 switch date,
  and `utility://constraints/utility-optimization-001`.
- Plan-001: CivicGrid Secure + Fiber (synthetic); provider label; available
  status; 0.19 USD/kWh; 22 USD monthly base; internet 45 USD for three months,
  then 70 USD; introductory expiry 2026-12-31; zero equipment; 99.95% uptime;
  no cap; billing note; `utility://plans/plan-001`; electricity 1633.90 USD,
  internet 765.00 USD, annual new-plan total 2398.90 USD; qualifies.
- Plan-002: BudgetSpark + StreamNet (synthetic); all pricing, introductory
  expiry 2027-03-31, 180 USD equipment, 99.5% uptime, 1000 GB monthly cap,
  billing note, available status, and `utility://plans/plan-002`; annual total
  2047.50 USD; reject for uptime and cap.
- Plan-003: Cobalt Flex + FiberPlus (synthetic); all pricing, no introductory
  price, 75 USD equipment, 99.9% uptime, no cap, billing note, available
  status, and `utility://plans/plan-003`; annual total 2380.70 USD; reject for
  uptime.
- Current service: MetroCurrent Electric and HomeNet (synthetic), plan
  `current-001`, 214 USD monthly, contract end 2027-02-28, 2026-10-01 switch
  date, 48.50 USD prorated service, 120 USD early-exit fee, projected final
  bill 168.50 USD, and `utility://current/current-001`.
- Recommendation arithmetic: `2398.90 + 168.50 = 2567.40` USD and
  `2600.00 - 2567.40 = 32.60` USD. State that plan-001 is recommended because
  it is the only plan meeting the hard reliability and cap requirements.
- Approved synthetic receipt: activation `activation-001`, plan-001,
  activation date 2026-10-01, status `activated_in_synthetic_account`, one
  submission, 2398.90 USD new-plan total, 168.50 USD old-provider final bill,
  2567.40 USD annual switch total, and
  `utility://switches/activation-001`.
- Final bill: `final-bill-001`, bill date 2026-09-30, current plan
  `current-001`, provider label, 48.50 USD prorated service, 120 USD fee,
  168.50 USD final amount, status `finalized_in_synthetic_ledger`, one
  submission, and `utility://bills/final-bill-001`.
- Final status: activation-001, plan-001, 2026-10-01, 2398.90 USD,
  final-bill-001, 168.50 USD, 2567.40 USD, 2600.00 USD limit, 32.60 USD
  remaining, one submission, `activated_in_synthetic_account`, and
  `utility://status/utility-optimization-001`.
- Exact approved write arguments, including the approval note
  `Synthetic switch only; no provider contact or real service change.`
- The eight-call execution ledger in order: the five initial reads, the one
  switch write, the final-bill read, and the status read. State that each ran
  once and that approval preceded the write.

After creation, verify the artifact through the artifact detail query. Record
its artifact ID, version ID, byte size, Markdown media type, preview kind or
preview URL, and the stored content hash from the artifact-version metadata.
The preview must contain the plan comparison, arithmetic, receipt, final bill,
final status, source locators, and synthetic-only boundary.

## Acceptance criteria

- Exactly one Markdown artifact is created.
- The artifact contains every required fact, calculation, locator, approval
  boundary, and eight-call ledger listed above.
- No connector or external-service call is made by this task.
- The artifact preview is readable and matches the stored content hash.
