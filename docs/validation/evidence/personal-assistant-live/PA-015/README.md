# PA-015 — Audience updates

Verdict: **Pass**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8` plus synthetic budget facts in
the request

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Execution

The request asked for separate internal and customer-safe launch updates. It
included a synthetic spend of 1,200 against a 1,500 budget and required that
these internal figures stay out of the customer version. It also required the
stale September 20 date to be labelled, not guessed, and prohibited sending.

Turn `turn:a17102a91a7f3810376d3f94420b683f` created two local Markdown
artifacts through Noema's native artifact tool:

- `artifact:eb802e8b1e7705436f324d055c6fd9d7`, version 1,
  `launch-internal-update-2026-09-08.md` (1,538 bytes).
- `artifact:e71f52dbcf5c6654fde3d45692d555d4`, version 1,
  `launch-customer-safe-update-2026-09-08.md` (723 bytes).

The internal draft contains the budget and analytics risk. The customer draft
contains the approved September 18 target, labels September 20 as an obsolete
draft, and omits budget, spend, remaining-budget, and personnel information.
No message was sent or shared.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Produce two separate audience-specific updates | Pass | The turn created two distinct named Markdown artifacts. |
| Resolve or label the date conflict | Pass | Both drafts keep September 18 and label September 20 as obsolete. |
| Keep shared facts consistent | Pass | Both drafts use the same approved target, analytics risk, and readiness status. |
| Exclude internal costs from customer text | Pass | The customer artifact contains no budget or spend figures. |
| Save both drafts | Pass | Both artifact IDs and version 1 records are durable in Noema. |
| Avoid sending or sharing | Pass | The final response and tool trace show local artifact creation only. |

## Limitation

The budget numbers were synthetic facts supplied in the request. Project and
launch context came from the connected Noema project, Gmail, Calendar, and
Notion sources.
