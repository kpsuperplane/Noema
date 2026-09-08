# PA-003 — Incoming actions

Verdict: **Pass after repair and rerun**

Run date: 2026-09-07

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-07-gmail-v1-notion-mcp-v6`

Public fixture route: `https://noema.kevinpei.com/__pa-replay/`

## Scenario

Account A contained one agreed action in Gmail and Notion:

- Kevin sends the launch metrics to Priya by 2026-09-17.
- The agreed review is 2026-09-16, 11:00–11:30 America/Los_Angeles.

The Gmail thread also contained a repeated reminder. A separate Gmail message
and the Notion page mentioned a wider review on 2026-09-18 as a proposal only.
No agreement supported that date.

## Execution

The first broad inbox pass returned older messages and did not yet identify the
new action. A normal follow-up asked Noema to check the agreed launch-metrics
follow-up. Noema searched the connected Notion MCP server, found
`notion-page-actions`, and reported the agreed action while excluding the
2026-09-18 proposal.

The operator then asked Noema to put the agreed action on the task list and
calendar. Noema created the native Task
`task:b9ceca89aa4723a69749b654ddc2053c` and read the calendar before writing.
The first Calendar connector definition sent a flat request body and the
fixture returned HTTP 400. No event was created by that failed request.

Noema proposed the corrected Calendar definition. The operator inspected and
accepted reviewed semantic digest
`887e0e34eeca1384b3e68ace07dab644fb70181676b806df2de52ecffabdbd95`, revision
`2026-09-07-calendar-v8-create-event-argument-placeholders-review`. The create
operation uses the documented nested body:

```json
{
  "summary": { "$argument": "summary" },
  "start": { "dateTime": { "$argument": "start" } },
  "end": { "dateTime": { "$argument": "end" } }
}
```

The existing Calendar connection remained active as
`14fcc90aa752c28fea27be1a4de7f8b9`. Noema checked for an existing event,
requested approval, and the operator approved governed action
`action:96dc0a65a40eaa4ba25730fac07a1050` at revision 1. The write returned
event `account-a-created-5` with the expected title and times.

Noema then read the event through the same registered connection. A later
repeat of the starting request produced the actionable checklist without a
new Task or calendar write. A separate repeated-inbox check also listed the
existing Task and did not call `task.capture` or `calendar_create_event`.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Identify the agreed action | Pass | Gmail `a-msg-014` and Notion `notion-page-actions` contain the same action and due date. |
| Exclude the unagreed date | Pass | `a-msg-016` and the Notion page label 2026-09-18 as a proposal only. No event or Task uses that date. |
| Create one native Task | Pass | Task `task:b9ceca89aa4723a69749b654ddc2053c` is the only matching Inbox Task. Its revision 2 document contains the source action and event reference. |
| Create one agreed calendar item | Pass | Calendar response returned `account-a-created-5`, 2026-09-16 11:00–11:30 Pacific, title “Review launch metrics with Priya”. |
| Require approval for the write | Pass | Governed action `action:96dc0a65a40eaa4ba25730fac07a1050` succeeded only after the explicit multiple-choice approval. |
| Verify the external effect | Pass | Fixture trace records the account-A POST at HTTP 201. A later Noema Calendar read returned the same single event. |
| Repeated intake is idempotent | Pass | The repeated starting request and later inbox check created no second Task or event. |
| Preserve account scope | Pass | All Gmail, Notion, and Calendar calls used synthetic account A. |
| Avoid real external changes | Pass | All writes targeted the synthetic fixture only. No real service, purchase, booking, or payment was used. |

## Trace highlights

The fixture request trace records the failed flat-body attempt at
`2026-09-07T02:57:59.650435589Z` with HTTP 400, followed by the corrected
read at `03:05:02.995889689Z` and successful create at
`2026-09-07T03:05:12.869272522Z` with HTTP 201. The later verification read
returned the created event through Noema rather than reading fixture state
directly.

The initial discovery miss is retained as a product observation. The case
passed after an ordinary follow-up, connector correction, and exact rerun.
