# PA-045 — Claim reconciliation

Verdict: **Pass**

Date: 2026-09-09

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`, server build revision `3231017e`.

## Fixture

- Service: synthetic claim-reconciliation API, fixture
  `2026-09-09-claim-reconciliation-api-v1`.
- Documentation: `https://licensing-fruit-ball-hour.trycloudflare.com/docs`.
- Fixture source: [`scripts/acceptance/run-mock-claim-reconciliation-api.ts`](../../../../../scripts/acceptance/run-mock-claim-reconciliation-api.ts).
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Connection: `b3cf93571e04a2df440c0e3e237b5479`.
- The fixture is synthetic. Its follow-up route records a mock request and
  never sends money. The settlement control is operator-only and is not a
  Noema connection operation.

The account is `account-claim-001`, labelled **Jordan Lee household
(synthetic)**. It contains one home claim, `claim-home-001`. The loss is
$3,000, the deductible is $500, and the insurer has paid $2,000. The
documented outstanding amount is therefore $500. A $3,000 repair estimate and
two receipts for $1,200 and $1,800 support the loss.

## Connection setup

Noema inspected the exact documentation route and called
`adapter.definition_template` before proposing the connector. The first
temporary tunnel expired after the proposal was accepted, so its failed read
is retained as a transport-recovery observation. No claim endpoint was called
through that expired connection.

Noema then inspected the replacement documentation and the existing reviewed
definition. It proposed an exact-digest revision instead of creating an
unlinked duplicate. The replacement proposal contained exactly five
operations:

- `get_profile` — `GET /v1/profile`.
- `list_claims` — `GET /v1/claims`.
- `list_claim_evidence` — `GET /v1/claims/{claim_id}/evidence`.
- `submit_claim_follow_up` — `POST /v1/claims/{claim_id}/follow-ups`.
- `get_follow_up_status` — `GET /v1/follow-ups/{follow_up_id}`.

The pending replacement digest was
`ee28bd372950d3387247f6dfcb0fd59fa214220fe1e11ce72bfc2697cbfa605d`. The
operator reviewed it as revision `v2`, producing reviewed digest
`8e40cecd9d27b2a3c15c681015324d839980bd0ac1f0e656269853ea85684b72`.
The connector uses no credentials because the documentation specifies no
authentication. Claims are bounded to one record and evidence to four
records. All projected strings have byte limits.

The connection is active and ready at connection revision 3 and policy
revision 2. It has all five operations available, with automatic data sharing
and `always_ask` for unsafe actions.

## Execution

1. `turn:3338d707d05e0ce8e444207d35baae5c` read the profile, claim, and
   evidence through the replacement connection. Each call completed.
2. Noema reconciled the amounts correctly:
   `$3,000 - $500 = $2,500 expected covered; $2,500 - $2,000 = $500`
   outstanding. It confirmed that the two receipts total $3,000.
3. Noema presented a multiple-choice approval prompt with the exact amount
   and reason. It did not submit before approval.
4. Governed action `action:f113a72191df7951e739bb002e98a3dc`, revision 1,
   targeted only `claim-home-001`, requested `500`, and used reason
   `Request the documented outstanding claim balance.` The operator approved
   that exact action.
5. The fixture received exactly one follow-up POST. It returned
   `follow-up-home-001`, status `pending`, and `submission_count: 1`.
6. The operator advanced the fixture-only settlement control. Noema then ran
   `turn:b66a6e030d841d3ea02f0190887ca85e`, read
   `follow-up-home-001`, and received status `paid` with `paid_amount: 500`.
7. Noema saved artifact `artifact:eb724a1a771fab206fb4d874944bf0f`, version
   `artifact_version:d77810ee9f365b7e9aaca5b938a419d6`, titled **Final
   Synthetic Claim-Reconciliation Report**. The Markdown file is 2,386 bytes
   and has content SHA-256
   `f13c2a3d6b2e72dd15ba9a163923d7a3c4b796a565fc1a65e1091a5db21e8723`.

## Independent fixture checks

The fixture log shows one documentation GET, one profile GET, one claims GET,
one evidence GET, one follow-up POST, one operator-only settlement POST, and
one follow-up-status GET. The follow-up POST contains exactly the approved
claim ID, amount, and reason. No second submission occurred. The settlement
route was never included in the connector definition.

The saved report includes the calculation, all three evidence records, the
follow-up receipt, the paid status, and the source locators. It states that
the records are synthetic and that no insurer was contacted or real money was
moved.

## Acceptance

| Criterion | Result |
| --- | --- |
| Reconcile loss, deductible, and prior payment | Pass — Noema calculated the documented $500 outstanding amount. |
| Assemble supporting evidence | Pass — the estimate and two receipts total $3,000 and are listed with source locators. |
| Require approval before follow-up submission | Pass — one governed action was presented and approved at revision 1. |
| Submit the approved follow-up once | Pass — the fixture shows one POST with amount $500, exact reason, and submission count 1. |
| Verify later status and payment amount | Pass — Noema read `paid` and `$500` after the fixture-only operator transition. |
| Save a sourced reconciliation report | Pass — the final Markdown artifact contains the calculation, evidence, receipt, status, and sources. |
| Avoid real insurer or financial activity | Pass — every service record is synthetic; no real insurer or payment endpoint was used. |
