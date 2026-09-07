# PA-002 — Promise register

Verdict: **Pass**  
Run date: 2026-09-07  
Backend: Go development instance through `/tmp/noema-codex/graphql.sock`  
Fixture: `2026-09-07-gmail-v1-notion-mcp-v5`  
Fixture revision: `3e348d02`

## Scenario

Account A started with three explicit Kevin-linked promises:

- Sam owed Kevin $85 for a team dinner by 2026-09-10.
- Kevin owed Priya a launch report by 2026-09-14.
- Kevin owed Jordan $40 for a conference ticket by 2026-09-12.

Gmail also contained a reminder for the Sam reimbursement. Notion contained
the same three promises and stated that the reminder was a duplicate. Account
B had no obligation records.

The operator used the existing task rather than creating a second ledger:

| Phase | Task generation | Result |
| --- | ---: | --- |
| Initial register | 2 | Three unique promises, both directions, sources, and one deduplicated reminder. |
| Date refresh | 3 | Priya's deadline changed to 2026-09-16; 2026-09-14 remained superseded history. |
| Receipt refresh | 4 | Sam's $85 item closed from the explicit payment receipt; Priya and Jordan stayed open. |

Task ID: `task:7402a6a296037acae01edb395d39da01`. The final Task stage was
`Done`, with a completed execution and reviewer document.

## Recovery check

During generation 4, the live Noema backend process was stopped after the
executor had started. The existing launcher session was recreated. The same
Task execution continued from its saved state and reached reviewer approval.
No second task or duplicate source record was created.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Find three explicit promises | Pass | Final `RESULT.md` lists Sam, Priya, and Jordan with counterparties, amounts or deliverables, dates, status, and source IDs. |
| Represent both directions | Pass | Sam appears under **Others owe Kevin**. Priya and Jordan appear under **Kevin owes others**. |
| Preserve source evidence | Pass | Gmail `a-msg-008` through `a-msg-013` and Notion `notion-page-obligations` are cited. |
| Deduplicate the repeated email | Pass | `a-msg-008` and `a-msg-011` are one Sam reimbursement. The unchanged follow-up did not add a second item. |
| Replace one date without duplicating the promise | Pass | `a-msg-012` replaces Priya's 2026-09-14 date with 2026-09-16. The old date remains marked superseded. |
| Close only a receipted promise | Pass | `a-msg-013` states that Sam paid Kevin $85. Priya and Jordan remain open. |
| Preserve account scope | Pass | All case reads used account A. The fixture exposes no obligation records to account B. |
| Make no external changes | Pass | Provider traffic used Gmail reads and Notion read/session operations only. No messages, pages, reminders, or payments were written. |
| Recover after backend restart | Pass | Generation 4 continued after the Noema process restart and reached reviewer approval. |

## Final inspected result

The final register contains exactly three unique obligations:

1. Sam → Kevin, $85 team-dinner reimbursement — complete and receipt-backed.
2. Kevin → Priya, launch report — open, due 2026-09-16.
3. Kevin → Jordan, $40 conference-ticket payment — open.

The report states that the review was read-only and does not claim that Kevin
has no obligations outside the connected sources.
