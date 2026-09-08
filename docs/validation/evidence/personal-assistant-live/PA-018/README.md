# PA-018 — Relationship cadence

Verdict: **Pass**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8` plus synthetic contact
preferences supplied in the request

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Execution

Turn `turn:b71b3c5615fd5b2e19b0b76ceb174703` saved a private **Thoughtful
contact reminders** project and two recurring Noema Tasks. Sam's monthly
reminder was initially set for September 25, and Priya's quarterly reminder
for October 1. Jordan Kim was marked no-contact, with no Task created.
Both reminder documents prohibit automatic messages.

Turn `turn:887af97cdfad55e9c1807cf0745a7613` inspected the existing Sam Task
and moved its first occurrence to October 2 after the supplied occasion change.
It changed the recurring rule from the 25th to the 2nd, left Priya's Task
unchanged, and preserved Jordan Kim's no-contact status. No message was sent.

The resulting Task records are `task:33fb3fc9cc0f69161d141cceede5b6ce`
(revision 3, Sam) and `task:bac6c88132e0d254124c74b14dba66db` (revision 1,
Priya).

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Save feasible reminders for different cadences | Pass | Sam monthly and Priya quarterly Tasks have durable UTC schedules. |
| Honor the no-contact preference | Pass | No Jordan Kim reminder or Task was created. |
| Update the changed occasion | Pass | Sam's existing Task now starts October 2 and recurs monthly on the 2nd. |
| Leave unrelated cadence unchanged | Pass | Priya's Task remains revision 1 with an October 1 quarterly schedule. |
| Avoid unsolicited outreach | Pass | Both Task documents prohibit automatic messages; no send tool ran. |
| Avoid duplicate reminders | Pass | The update inspected and revised the existing Sam Task. |

## Limitation

The contact preferences and occasion change were synthetic facts supplied in
the test turns. The reminders and recurrence state were saved through live
Noema Task and project tools.
