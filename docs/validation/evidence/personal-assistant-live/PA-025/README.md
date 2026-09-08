# PA-025 — Expense submission

Verdict: **Pass after adding a synthetic expense API connector and resuming the approved Task**

Run date: 2026-09-08
Backend: Go development instance through `/tmp/noema-codex/graphql.sock`
Fixture: `2026-09-08-expense-api-v1` (synthetic only)
Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Scenario

The first request found no expense or bank connection. The operator added one
small provider-neutral API fixture and connected it through Noema's normal
documentation, proposal, review, and policy flow. The fixture has four
receipts and four matching posted bank charges:

| Receipt | Amount | Decision |
| --- | ---: | --- |
| `receipt-040` — Northstar Office Supply | $40 | Include |
| `receipt-060` — Harbor Transit | $60 | Include |
| `receipt-060-duplicate` — Harbor Transit | $60 | Exclude as a duplicate of `receipt-060` |
| `receipt-025` — Personal Market | $25 | Exclude under the fixture policy |

The fixture cannot contact an employer, bank, merchant, or payment network.
No real money moved.

## Connector setup

| Item | Evidence |
| --- | --- |
| Fixture source | [`scripts/acceptance/run-mock-expense-api.ts`](../../../../../scripts/acceptance/run-mock-expense-api.ts) |
| Documentation URL used by Noema | `https://dana-invest-various-amendment.trycloudflare.com/docs` |
| Proposal turn | `turn:b3f115654b4a3ae5c4a21d2035d8f365` |
| Reviewed proposal digest | `9d6e279bbe96be4f1325e5bb49f21cf69807deb76c489662c2ba499cd264379f` |
| Accepted definition digest | `c8a9d142edbf372b841849410ae1ff8aa5c5dbfc4c2bd80077cee51800414200` |
| Connection | `d4d0e0fd1d8c989045d8b195b72f434e`, revision 2 |
| Connection policy | Sharing allowed automatically; unsafe actions always ask |

The proposal initially failed because the source URL used HTTP, the API origin
included a path, and the first response schemas were too large. Noema reported
each issue. The agent corrected them and produced the reviewed proposal.

## Execution

1. The initial request (`turn:362b7c52a9af50585475aee046f49145`) and its
   follow-up (`turn:9dc25d82d2f13187ca3d1c5676a116b1`) correctly stopped when no
   expense service was available. No claim was submitted.
2. Noema prepared a local draft (`artifact:d56d359e62be3b8df112e7d17743548a`)
   with a $100 total, the two eligible receipt IDs, and both excluded IDs. It
   asked for approval before submitting.
3. The approval message created Task
   `task:e6d716833cfe4b6477b00a2e484d70ee`. The first executor run reported a
   missing submission tool. After the connection policy was saved, the operator
   answered the recovery gate and Noema resumed the same Task.
4. Governed action
   `action:62052fcbe80032e20e9fd319e46949f1` contained exactly:

   ```json
   {
     "amount": 100,
     "receipt_ids": ["receipt-040", "receipt-060"],
     "excluded_receipt_ids": ["receipt-060-duplicate", "receipt-025"]
   }
   ```

   The operator approved this synthetic action. The connector returned
   `{"id":"reimbursement-001","status":"SUBMITTED"}`.
5. The Task executor and reviewer completed successfully. The Task finished at
   revision 17. Its `TASK.md` and `RESULT.md` record one submission and no
   duplicate claim.
6. Noema then read `reimbursement-001` through the connector in
   `turn:de7f38b69187c8da087e11beb7cdc1c0`. The result was `PAID`.
7. A final read-only turn (`turn:b94a63ffac47240c82c3953f79b80e62`) used the
   connector operations `list_receipts` and `list_bank_transactions`. Both
   returned all four records. It confirmed the $100 eligible total, the
   duplicate, the policy exclusion, and four `POSTED` bank transactions.

An independent read of the synthetic fixture also reported
`submissionCount: 1`. The fixture changes a submitted claim to `PAID` on its
first status read, as documented. This read did not contact a real service.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Reconcile receipts with bank records | Pass | `turn:b94a63ffac47240c82c3953f79b80e62`; four receipt and four bank records matched by ID and amount. |
| Exclude the duplicate $60 receipt | Pass | `receipt-060-duplicate` points to `receipt-060` and is excluded. |
| Explain and exclude the $25 policy item | Pass | `receipt-025` is marked policy-excluded with its fixture reason. |
| Request $100 exactly once | Pass | Draft, governed action, Task records, and fixture state all show one $100 claim. |
| Ask before the write | Pass | The draft asked for approval; the POST was held until `action:62052...` was approved. |
| Submit through the connected API | Pass | Tool result from `synthetic_expense_api_personal-d4d0e0fd.submit_reimbursement`. |
| Verify submitted and paid status | Pass | POST returned `SUBMITTED`; `get_reimbursement_status` returned `PAID`. |
| Avoid real financial activity | Pass | The endpoint is a local synthetic fixture exposed through a temporary test tunnel. |
