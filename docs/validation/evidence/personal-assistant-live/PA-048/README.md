# PA-048 — Credit correction

Verdict: **Pass**

Date: 2026-09-09

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`, server build revision `3231017e`.

## Fixture

- Service: synthetic credit-correction API, fixture
  `2026-09-09-credit-correction-api-v1`.
- Documentation: `https://assembled-pipeline-northeast-riders.trycloudflare.com/docs`.
- Fixture source: [`scripts/acceptance/run-mock-credit-correction-api.ts`](../../../../../scripts/acceptance/run-mock-credit-correction-api.ts).
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Connection: `e4179d194a632ca90f3b3319989e0995`.
- The fixture is synthetic. It has no real bureau, real credit-file, or money
  movement endpoint.

The profile is `credit-account-001`, labelled **Jordan Lee (synthetic)**. Its
goal is to correct an inaccurate late-payment report using a lender receipt.
Its decision boundary is: **do not contact a real bureau, change a real credit
file, or move money**.

## Connection setup and repair

Setup task `task:8bef4a2849b02f34a8d55006933dc3f8` read the documentation and
stopped at review. The operator accepted the initial five-operation proposal
as reviewed revision `v1`, digest
`5e7f661b20ed6f5b565c0dbb204e3aa6325bdc6b6696a32907addea10f77557`.

The first read found four gaps: the report response wrapper failed to decode,
the profile operation was absent, the schedule projection dropped
`posted: false`, and no dispute-submit operation existed. The first repair
proposal was pending digest
`46cd789666a6ddc5acae7797da58884bbe020f4f39b8f40e21b89d5d498f458e`.
Repair task `task:95a733a7f041e02b57cec7f4e071a896` detected that compiler bug
and was cancelled after the follow-up revision replaced the defective mapping.

That proposal was accidentally accepted as reviewed digest
`e4788ca243cbdfed4cbefd808dece8592599a6fa59843c0f4dde3c45c29f9294` before
the generated schedule transform was inspected closely. Its compiler output
still used a false-dropping expression. No write or service endpoint was
called during that repair run.

The follow-up repair loaded the exact active digest and replaced only the
schedule projection with an explicit boolean branch. It preserved all seven
operations, including the report wrapper, profile read, and dispute write.
The pending v3 digest was
`f36b38191ddf24bdd30d2f47e3bf961af55b1edabb2363f1d472a62358d4a958`.
The operator accepted reviewed revision `v3`, digest
`161438959f739dec1eb5cfcf831f9c6ba76472ef71b0b40cdd8d37346ac14738`.

The active connection is ready at connection revision 5 and policy revision
4. It has seven available tools, automatic data sharing, `always_ask` for
unsafe actions, and no credentials because the fixture is unauthenticated.

## Execution

1. The initial read turn `turn:c32bdaaca8b9f4128663b9c584545c35` saved a
   partial evidence artifact `artifact:be70ca9aca46c43fab49fbfd8c53ff12`.
   The report read failed with `response_transform_failed`, the profile tool
   was unavailable, and the schedule result omitted `posted`. The lender
   payment read succeeded. No dispute was submitted.
2. The corrected read turn `turn:62afc3517953323b001527e07e24161f` used v3
   and successfully read the profile, report, schedule, and lender receipt.
   It returned `posted: false` explicitly and stopped before the write.
3. The assistant staged the exact mock dispute in turn
   `turn:5c4ef8f1c5ebfde4d72b9f34c053735a`. It used report ID
   `credit-report-001`, payment ID `payment-145-2026-09`, and an
   evidence-based reason. The operation enablement gate
   `action:6c1e8813bd6cd2f936be2a0aa8775949` was approved.
4. The governed synthetic write
   `action:e17093bb70a3e37e29fcf2ff41bbd6ab` was approved once. It returned
   dispute ID `dispute-credit-001`, status `submitted`, and submission count
   `1`.
5. Verification turn `turn:97864d4b0c19c13192e97d06db508b58` read the dispute
   status and corrected report. It saved the final sourced review as artifact
   `artifact:1e791bb344efc7fc8dcac895e0efa13e`, version
   `artifact_version:1bd2dc20558282fe6e370c27b26a8f6d`, 1793 bytes,
   SHA-256 `8bda0efc1fe4d931693893a0a06eb7803290ee47c13a9cf30120db24d1011f31`.

The final review records the initial report, the schedule and receipt
distinction, the dispute receipt, the still-submitted dispute status, and the
later corrected report.

## Independent fixture checks

The fixture log records documentation and read calls, one POST to
`/v1/disputes`, one dispute-status read, and one corrected-report read. The
POST body contained exactly `report_id: credit-report-001`,
`payment_id: payment-145-2026-09`, and the evidence-based reason. Its response
reported `submission_count: 1`.

Direct checks returned:

- report `credit-report-001`, `$145`, `late_payment`, dated 2026-09-06;
- schedule `schedule-145-2026-09`, `$145`, due 2026-09-05,
  `schedule_status: scheduled`, `posted: false`;
- payment `payment-145-2026-09`, receipt `receipt-145-2026-09`, `$145`,
  `settled`, received 2026-09-04T14:30:00Z;
- dispute `dispute-credit-001`, `submitted`, submission count `1`;
- corrected report status `corrected`, reported status `current`, linked to
  `dispute-credit-001`.

An operator shell quoting mistake changed `$145` to `$45` in the first read
prompt. It did not change fixture data or the saved result. Later prompts used
safe quoting, and every final tool result and direct fixture response shows
the correct `$145` amount.

## Acceptance

| Criterion | Result |
| --- | --- |
| Distinguish the scheduled item from the lender payment | Pass — the review states `posted: false` for the scheduled $145 item and `settled` for the linked lender receipt. |
| Link the evidence to the exact report and payment IDs | Pass — it uses `credit-report-001` and `payment-145-2026-09`, linked by `schedule-145-2026-09`. |
| Submit one mock dispute after approval | Pass — one governed POST returned `dispute-credit-001` and `submission_count: 1`. |
| Verify the later corrected report | Pass — the corrected route returned `correction_status: corrected`, `reported_status: current`, and the same dispute ID. |
| Avoid real bureau contact, real credit-file changes, and money movement | Pass — every call targeted the synthetic fixture, and the saved review states the boundary. |
| Save a sourced final review | Pass — the final Markdown artifact contains source locators, IDs, dates, amounts, status values, and the correction outcome. |
