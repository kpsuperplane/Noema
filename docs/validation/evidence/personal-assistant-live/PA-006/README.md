# PA-006 — Daily plan

Verdict: **Pass after correction and full-document reread**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Setup

The synthetic Calendar account had two fixed blocks on Wednesday, September
9, 2026, in America/Los_Angeles:

- Team planning, 09:00–11:00;
- Customer workshop, 11:30–13:30.

Three open native Tasks were captured through the Noema CLI. Each full
`TASK.md` states a two-hour estimate and a September 9, 16:00 Pacific
deadline:

| Task | ID |
| --- | --- |
| Draft launch brief | `task:ca0013eb9df8bc8306da9d38cc087552` |
| Review customer contract | `task:0bb240602dfbda6442113c9cc095047f` |
| Prepare customer demo | `task:87c2ae993c3617a730e49d3be27f7003` |

The request supplied a one-hour lunch preference and a 09:00 start. The
existing local-human Memory records a weekday 16:00 stop time, which was also
stated in the request. No external write was needed.

## Execution

The first natural request asked Noema to make a realistic plan. Noema asked
for the task and appointment details instead of reading its connected sources.
The operator corrected this by asking it to look up the current Tasks and
tomorrow’s Calendar first.

Noema then:

1. listed current Tasks and found exactly the three setup Tasks;
2. read the bounded Calendar interval for September 9;
3. inspected all three complete Task documents with `task.inspect`; and
4. recalculated the plan from those records.

The final plan used the seven hours from 09:00 to 16:00. Four hours are fixed
meetings and one hour is lunch, leaving two hours for Task work. It scheduled
the customer-demo Task around the customer workshop. It deferred the contract
review and launch brief instead of pretending that all six Task hours fit.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Read the connected Calendar | Pass | `list_events` returned both fixed blocks for September 9 with exact Pacific offsets. |
| Read full Task documents | Pass | Three `task.inspect` calls returned the complete documents, estimates, and deadlines. |
| Protect fixed commitments | Pass | Team planning and customer workshop remain unchanged in the schedule. |
| Protect a one-hour lunch | Pass | The plan reserves 13:30–14:30 as an uninterrupted lunch. |
| Honor the weekday stop time | Pass | The plan ends work at 16:00 and does not schedule work after it. |
| Account for capacity | Pass | Six hours of Task estimates exceed the two available work hours after meetings and lunch. |
| Defer excess work visibly | Pass | Contract review and launch brief are listed as deferred, each with its two-hour estimate. |
| Avoid invented free time | Pass | The schedule contains no work block during either meeting or lunch and no time after 16:00. |
| Keep the case read-only | Pass | No Calendar, Task, message, or external write action was created. |

## Evidence and limitations

The durable conversation records contain the correction, Calendar read, Task
list, three full Task inspections, and final schedule. Calendar data came from
the synthetic fixture account A. The plan does not claim that any deferred
Task was completed or that any deadline changed.
