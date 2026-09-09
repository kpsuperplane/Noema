# PA-047 — Retirement records

Verdict: **Pass**

Date: 2026-09-09

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`, server build revision `3231017e`.

## Fixture

- Service: synthetic retirement-records API, fixture
  `2026-09-09-retirement-records-api-v1`.
- Documentation: `https://narrow-mysimon-ment-technological.trycloudflare.com/docs`.
- Fixture source: [`scripts/acceptance/run-mock-retirement-records-api.ts`](../../../../../scripts/acceptance/run-mock-retirement-records-api.ts).
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Connection: `1b670aea772e7df3db0e35aaf907eb56`.
- The fixture is synthetic. It has no transfer, trade, investment, or money
  movement endpoint.

The owner profile is `retirement-owner-001`, labelled **Jordan Lee
(synthetic)**. Its stated goal is to reconcile retirement records and track a
rollover receipt. Its decision boundary is: **do not transfer money or make an
investment decision**.

## Connection setup and repair

Setup task `task:3c7d0e480804e0abd950f88bf469354f` read the documentation and
stopped at the review gate. The operator accepted the initial seven-operation
proposal as reviewed revision `v1`, digest
`1f1f4aa44ea6e26ec31708dafac4ae578f7ca10715885cf90936c3ebceab6443`.

The first read exposed response-contract mismatches. Accounts used
`annual_fee_bps`, fees used `fee_bps` and `annual_fee_usd`, transfers were
wrapped under `transfers`, status used `next_check_after`, and the receipt
status field was named `status`. The first repair proposal was pending digest
`c1c05d9f10895396c9673fc78188ba1ef946bc471e0d758fa4b32d791bb9b928`. The
operator accepted the exact revision as reviewed `v2`, digest
`b52c886998fec080e0b71a87ccfbc2c55226cf82e97076c0da501aec9a2ec637`.

The corrected read then succeeded. It showed that the initial transfer list
also needed its documented `receipt_status` field exposed. A second exact
revision changed only `list_rollovers`. Its pending digest was
`5f0b4fac7b83c9584a8b7e0a11b90f6e4f1003d79659cea41c080dd3fafe4a49`. The
operator accepted reviewed revision `v3`, digest
`5a75c7e963fe0ed1ecc7c8b135a2e13d09bc3b64cb7ace27e17feee71acbb5df`.

The active connection is ready at connection revision 4 and policy revision
2. It has seven available tools, automatic data sharing, `always_ask` for
unsafe actions, and no credentials because the fixture is unauthenticated.

## Execution

1. Initial review task `task:61bd9b9f973bce5228d1aa54e77b9ce2` read the owner
   profile and statements, but its account and fee reads failed with
   `response_transform_failed`; its transfer wrapper also returned an empty
   list. No write was attempted.
2. Corrected rerun task `task:75d6583b6ace5d0e5a144c5fd7987a73` read all seven
   required routes through reviewed `v2`. It returned both accounts, all three
   statement IDs, both fee records, `rollover-001`, its status, and its later
   receipt. Its saved review correctly totaled `111200`, but did not expose the
   initial `receipt_status` flag.
3. Receipt-field repair task `task:fdb89f85eaccd2b81e9170d3c584c139` proposed
   and stopped after the reviewed `v3` revision. A stalled duplicate repair
   attempt was cancelled and had no endpoint access.
4. Final review task `task:a8b2708202a4929587a4407ca93be261` reran all seven
   reads through reviewed `v3` and saved a sourced review at
   `/tmp/noema-codex/home/tasks/a8b2708202a4929587a4407ca93be261/RESULT.md`.
   The reviewer marked the result complete.

The final review reported:

- two distinct accounts: `retirement-account-24960` at `24960` and
  `retirement-account-86240` at `86240`;
- total balance `24960 + 86240 = 111200`;
- two statements for the first account were counted once by account identity;
- fees of 55 bps and `$137.28` versus 25 bps and `$215.60`, a 30 bps rate
  difference and `$78.32` annual-estimate difference;
- initial `rollover-001` status `sent` with `receipt_status: not_received`;
- subsequent status `sent` with `next_check_after: 2026-09-11`;
- later receipt `receipt-rollover-001` with status `received`.

## Independent fixture checks

The final run log shows exactly one GET for each required route:
`/v1/profile`, `/v1/accounts`, `/v1/statements`, `/v1/fees`,
`/v1/transfers`, `/v1/transfers/rollover-001/status`, and
`/v1/transfers/rollover-001/receipt`. The fixture received no POST, PUT, PATCH,
or DELETE request. Direct checks of the same routes returned the documented
synthetic source locators and values. No real financial account was contacted.

## Acceptance

| Criterion | Result |
| --- | --- |
| Deduplicate the two statements for the `24960` account and retain the distinct `86240` account | Pass — the final review identifies all three statement IDs, groups the two `24960` records under one account, and counts two accounts. |
| Total `111200` without duplication | Pass — it shows `24960 + 86240 = 111200`. |
| Compare fees | Pass — it reports both returned rates and annual estimates, plus supported differences. |
| Preserve the initial transfer uncertainty | Pass — it preserves `status: sent` and `receipt_status: not_received` before later reads. |
| Follow status to later receipt | Pass — it reads the exact transfer ID, records `next_check_after`, and then records the later `received` receipt. |
| Make no real transfer or investment decision | Pass — every call is read-only against the synthetic fixture, and no financial action was proposed or executed. |
