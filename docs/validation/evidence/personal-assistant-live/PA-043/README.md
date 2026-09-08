# PA-043 — Subscription cancellation

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`, server build revision `55bf9ba6`.

## Fixture

- Service: synthetic subscription API, fixture
  `2026-09-08-subscription-api-v1`.
- Documentation: `https://deny-victor-barely-carnival.trycloudflare.com/docs`.
- Fixture source: [`scripts/acceptance/run-mock-subscription-api.ts`](../../../../../scripts/acceptance/run-mock-subscription-api.ts).
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Connection: `54973483f7ae5663350577508330f443`.
- The fixture cannot bill a card, cancel a real service, or expose a real
  account. The test-cycle endpoint is operator-only and is not a customer
  connection operation.

The account is `Jordan Lee (synthetic)`. It contains two active Streambox
plans. The monthly plan costs $18.99, has no commitment end, and has a $0
cancellation fee before 2026-10-01. The annual plan costs $179, remains
committed through 2027-01-01, and has a $120 cancellation fee.

## Connection and repairs

Noema inspected the documentation and proposed five reviewed operations:
profile, subscription list, cancellation, receipt lookup, and billing-event
lookup. The initial proposal was accepted as revision `v2`; pending digest
`b47022eae5f627da73123802ac29daf537a1160a0dc6733201c145160c092502` became
reviewed digest
`0af947177e7a13ff10dd94bc8e8f230d266a73b14c786d52be2d849287c94bb4`.

The first subscription read exposed a contract defect. The monthly record has
`commitment_end: null`, but the reviewed item schema required that field. A
focused repair removed only that field from the required list. Pending digest
`a5b53e397c5e342b2966a829cb342733d4133ebbbd74f9303a43e22a8090f9e4` became
reviewed revision `v4`, digest
`208a97c8d050124a06f09c1558c14ab6ec1732e0037afcde49891ff5888d60c3`.

The first receipt read returned only `cancellation_id`. Noema proposed a
focused projection for the receipt fields needed by this case. Pending digest
`e66ab5460863987b8c6db7795ff03b418d576b82c234b889b783a7ab520564db` became
reviewed revision `v5`, digest
`4a019ac1823196cf069102fc02b8596f6f40bcedb2ca01db9352ad01c251a2ba`.

The first billing-event read returned an empty list because the service uses
an `events` envelope while the reviewed transform looked for
`billing_events`. Noema proposed a mapping-only repair that also retained
subscription ID, plan, date, status, amount, currency, and source locator.
Pending digest
`781ba65269078ac0274a94a6856944dd7b2710b5d31355678aeb237b0a57a4bf` became
reviewed revision `v6`, digest
`f14b28e7a805687a55247632c633f39c25bef0d1e21aa8f26610037a3254f487`.

Repair tasks were `task:7a7e1dd88c0b60dcd9131720c944cbf6`,
`task:45dd65fefc4733f0423cfa4a58765fee`, and
`task:1187b4c5858370c979e55828cc4f8885`. The corresponding executor runs
were `run:0e810b62cfdad8f52a0ba696af84f2d8`,
`run:260b6030a892fcda62eddcf4f8367287`, and
`run:9ba9c9ef5e98ba046b0d0467b5469298`.

## Execution

1. `turn:aecad4be469c328e9d3b90dcdcfa0e8a` used the repaired list operation.
   It returned both plans and the exact monthly fee, date, and annual
   commitment.
2. `turn:87c36d467e071e0d44e395157a1ff285` prepared the cancellation and
   waited for approval. It targeted only `sub-stream-monthly` with reason
   `I no longer use it.`
3. Noema created governed action
   `action:24cfdce53a35743e329b9ccba2c0f924`, revision `1`, with the same
   arguments. The operator inspected and approved that exact action. It
   succeeded with cancellation ID `cancel-1`.
4. `turn:e767b8120b12e79f490a701f611a7f35` read the repaired receipt and saved
   artifact `artifact:f5e9b3084955dec84285f677f9164c59`, version
   `artifact_version:2fc485bbf6c6821f78b9628a28bf4e03`. The Markdown file is
   548 bytes and records the plan, effective date, $0 fee, confirmed status,
   stopped future charges, and source locator.
5. The operator advanced the synthetic cycle through the documented control
   endpoint. This was not proposed or exposed as a customer operation.
6. `turn:86686ea6fde3d1b7317f14280f9dc3e2` used the repaired billing-event
   operation and saved artifact
   `artifact:41399fef00966c503dd0a5586de4380a`, version
   `artifact_version:bb889534ff135f5dd1bb815acae64783`. It records the monthly
   event as `skipped_cancelled`, $0, and the annual event as `not_due`, $0.

The earlier inconclusive billing record
`artifact:67d64ea7d588d67688d2bdad58adf272` is retained as failed-attempt
evidence. It is not used for the pass.

## Independent fixture checks

The final fixture state returned:

- `sub-stream-monthly`: `canceled`, with no next billing date.
- `sub-stream-annual`: `active`, with next billing date `2027-01-01` and
  commitment end `2027-01-01`.
- 2026-10-01 monthly event: `skipped_cancelled`, amount `0`.
- 2026-10-01 annual event: `not_due`, amount `0`.

The request trace contains exactly one customer cancellation POST for
`sub-stream-monthly`, zero cancellation POSTs for the annual subscription,
and one operator-only cycle-advance POST. Receipt and billing reads completed
through the reviewed connection. No payment, real billing, or external
account change occurred.

## Acceptance

| Criterion | Result |
| --- | --- |
| Select the exact account and identify the monthly plan | Pass — profile and both subscription records were read through Noema. |
| Explain fee, next billing date, and annual commitment | Pass — monthly is $18.99/month with a $0 fee before 2026-10-01; annual is $179/year, committed through 2027-01-01 with a $120 fee. |
| Require approval before cancellation | Pass — one governed action was created and approved at revision 1. |
| Cancel only the monthly plan | Pass — fixture trace shows one monthly cancellation and no annual cancellation. |
| Save a complete receipt | Pass — receipt artifact contains ID, plan, effective date, fee, status, future-charge status, and source locator. |
| Verify later billing | Pass — repaired billing read and artifact show monthly skipped at $0 and annual not due at $0. |
| Avoid real financial activity | Pass — all service state is synthetic and no payment endpoint exists. |
