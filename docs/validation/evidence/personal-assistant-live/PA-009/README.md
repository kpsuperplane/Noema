# PA-009 — Deadline tracking

Verdict: **Pass after source disambiguation, one approved overdue notice, and
restart recovery**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Setup

The case used account A in the synthetic Gmail fixture and the connected
synthetic Notion MCP server. The Notion connection completed browser OAuth,
discovered three tools, and retained its read-only policy after the backend
restart:

- MCP server: `mcp_server:1e041eac4708761774cdcc984cb5a666`
- Gmail connection: `6665bd552f0c9ac133502f67fa9ef36b`
- Notion policy: `allow_automatically` for sharing and
  `reviewer_may_approve` for unsafe actions

The initial records included Sam's $85 reimbursement to Kevin, Kevin's launch
report promise to Priya, and Kevin's tentative $40 conference repayment to
Jordan. Gmail contained a repeated Sam reminder. Notion stated that the
reminder was a duplicate.

## Execution

The accepted initial review in turn `turn:775eee94a16604f1aeef1235c1855144`
read the connected Gmail and Notion sources. It listed one record for each
obligation, treated Sam's reimbursement as firm, and kept Jordan's repayment
tentative. It made no writes.

The fixture then added the controlled launch-report update. A first refresh
mistook a later reply-planning message for the obligation date. The correction
in turn `turn:aa6c09778e01f54de53a1a198f110966` read the matching obligation
message and Notion update, moved the current deadline from September 14 to
September 16, and retained September 14 as history. It left the unrelated
September 9 reply-planning commitment out of the obligation record.

The overdue manager reminder was sent only after approval. Action
`action:6d861e160a5f92e99493120b2d21b780` succeeded at revision 1 and returned
the synthetic receipt `account-a-sent-27` in existing thread `a-thread-work`.
No other obligation thread was sent.

The fixture then added the exact $85 payment receipt. Turn
`turn:4014a2e8bb7c0243c569d6dba376117d` read both sources and closed only
Sam's reimbursement. Priya's launch-report and launch-metrics obligations
stayed open. Jordan's $40 repayment stayed open and tentative.

The live Go server was stopped and restarted through the development launcher
after the receipt stage. The relay returned `{"state":"authenticated"}`
after recovery. The Notion server, policy, and approved action remained
durable. The post-restart quiet check in turn
`turn:61a6a9911685fdd1711b186fd631513f` made no writes or sends.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Reconcile one obligation across Gmail and Notion | Pass | Turns `turn:775eee94a16604f1aeef1235c1855144` and `turn:4014a2e8bb7c0243c569d6dba376117d` read both connected sources. |
| Deduplicate Sam's reminder | Pass | The initial review treated the reminder as the same $85 reimbursement. |
| Keep the conference repayment tentative | Pass | The initial and final reviews kept Jordan's $40 item tentative. |
| Apply the controlled deadline update | Pass after correction | Turn `turn:aa6c09778e01f54de53a1a198f110966` used the Gmail and Notion update and retained the old date as history. |
| Issue one overdue notice | Pass | Approved action `action:6d861e160a5f92e99493120b2d21b780` produced receipt `account-a-sent-27`; the fixture trace shows one new send. |
| Close only the exact receipt-backed item | Pass | Turn `turn:4014a2e8bb7c0243c569d6dba376117d` matched Sam, Kevin, and $85 in both receipts. |
| Preserve other open obligations | Pass | Priya remained open; Jordan remained open and tentative; no other records changed. |
| Keep the unchanged check quiet | Pass | The post-restart turn made no message, memory, or provider write. |
| Recover after backend restart | Pass | The relay recovered, and the authenticated three-tool Notion server and policy persisted. |
| Keep the case synthetic and account-scoped | Pass | All provider reads and the one send targeted fixture account A. |

## Evidence and limitations

The durable conversation contains the source reads, correction, approved send,
fixture receipt, exact payment match, restart follow-up, and final quiet check.
The fixture trace records the one new Gmail send after the overdue reminder was
approved. Earlier sends in the trace belong to the already accepted PA-004
case and were not repeated here.

The first refresh exposed a general ambiguity when older obligation records and
later reply-planning messages share a subject. The accepted correction used the
controlled obligation update and preserved the later message as a separate
conversation.
