# PA-024 — Approval coordination

Verdict: **Pass after synthetic review routing, two v2 approvals, and finalization**

Run date: 2026-09-08
Backend: Go development instance through `/tmp/noema-codex/graphql.sock`
Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`
Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Scenario

The fixture did not contain independent reviewer records, so the operator
supplied two clearly labelled synthetic replies. Alex and Priya reviewed the
resolved v2 packet. Their comments agree that the cost table stays in the
internal packet and is removed from customer-facing or other external copies.

The earlier v1 approval was kept as history. It was not used as approval for
v2. No message went to a real recipient.

| Item | Durable evidence |
| --- | --- |
| Initial approval copy | `artifact:1d026a05c5cd00b839cc4a89078a3c0e`, version `artifact_version:0250cd8d0b1d98a4786070e835346a5a` |
| Resolved v2 packet | `artifact:1f12247197e9ebb9d32c8cf3e70706c6`, version `artifact_version:abb61a307a200bb40b27e4f21b04be0c` |
| Review thread | Gmail fixture thread `a-thread-actions` |
| Review request receipt | `account-a-sent-29` |
| Alex approval receipt | `account-a-sent-30` |
| Priya approval receipt | `account-a-sent-31` |
| Final-version record | `artifact:1315f5c0492faf34957d16ca4510e102`, version `artifact_version:c3d18693b44257204d595d8d10f16c8f` |

Credential values and transient authorization data are not recorded.

## Execution

Turn `turn:ec8a848d6ef5fc6c0fa40058a297f9cd` created the initial local approval
copy. Turn `turn:2e5156423b61cd116c6aba99c74c2f41` reconciled the comments,
saved the resolved v2 packet, and routed one review request to both fixture
addresses. The send required a governed action. The operator approved the
exact action at revision 1, and the fixture returned receipt
`account-a-sent-29`.

Turn `turn:a1b98a0a33d7e3a18f97f918de2b859b` added the two synthetic reviewer
replies through the same Gmail connection. Each send required a separate
governed action at revision 1. The operator approved both exact actions. The
fixture returned `account-a-sent-30` for Alex and `account-a-sent-31` for
Priya.

Turn `turn:10983d4860f81f4ebb30ff02ad8774c7` reread the thread and checked the
two v2 approval messages. It then saved a final-version record that points to
the exact resolved v2 artifact without changing its content. It explicitly
kept the old v1 approval as history and sent no further messages.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Route the resolved v2 to both reviewers | Pass | `account-a-sent-29` addresses Alex and Priya in `a-thread-actions`. |
| Reconcile conflicting comments | Pass | The resolved v2 keeps the cost table internally and excludes it from external copies. |
| Require both v2 approvals | Pass | `account-a-sent-30` and `account-a-sent-31` both state approval of the resolved v2. |
| Do not reuse v1 approval | Pass | The final-version record labels v1 historical only and names the two v2 receipts as its approval basis. |
| Save the exact final version | Pass | The final-version record points to `artifact_version:abb61a307a200bb40b27e4f21b04be0c`; the resolved v2 file remains unchanged at 1,732 bytes with stored SHA-256 `6a9e3cf05ffdeb4a7183730691badcdaf001fc199df57d205e9e13f5ff8a4d31`. |
| Keep the operation inside the fixture | Pass | All sends used the synthetic Gmail connection. No real external recipient was contacted. |

## Independent file check

The read-only development home contains the resolved v2 file at:

`conversations/conversation_a39407c5e9686f34225273862829bf0f/artifacts/artifact_1f12247197e9ebb9d32c8cf3e70706c6/versions/1/objects/op-9b3bb44d3dad77a1658fea57c953a89797b235d48d1d3e19aa6d764a5b64876d/final-briefing-packet-resolved-v2-internal-review-2026-09-08.md`

Its actual bytes match the stored size and digest. The final-version record is
also present as a separate immutable local artifact.
