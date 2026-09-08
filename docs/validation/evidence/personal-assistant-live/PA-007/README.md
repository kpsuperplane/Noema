# PA-007 — Disruption replan

Verdict: **Pass after delay, draft inspection, and unchanged rerun**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Setup

The accepted Wednesday, September 9 plan included these synthetic Calendar
records in America/Los_Angeles:

| Record | ID | Initial time | Constraint |
| --- | --- | --- | --- |
| Immovable appointment | `account-a-created-11` | 08:00–08:30 | Do not move |
| Team planning | `account-a-created-8` | 09:00–11:00 | First meeting; notify `team@example.test` if delayed |
| Customer workshop | `account-a-created-9` | 11:30–13:30 | Fixed |
| School pickup | `account-a-created-10` | 15:30–16:00 | Dependent pickup; keep fixed |

The accepted plan also scheduled `Prepare customer demo` at 11:00 and
`Follow up after team planning` at 14:30. The latter Task records that it
must follow the Team planning meeting. The existing lunch and weekday 16:00
stop preferences remained in force.

The setup changed only the synthetic fixture. It moved Team planning one hour
later, to 10:00–12:00. No real calendar or person was contacted.

## Execution

The plan was accepted in turn `turn:44be663015134799d76036ea728e4c10`.
Noema saved the two planned Task blocks through its native Task tools.

After the fixture delay, the natural request in turn
`turn:6d6aab88ba3e03a71a8000764fad9c5e` asked Noema to rework the day. Noema
read the full bounded Calendar interval, followed both Calendar pages, and
inspected the two affected Task documents. It moved the demo-prep Task to
09:00, kept the follow-up after the meeting and workshop, and left the fixed
appointment, workshop, pickup, lunch, and stop time unchanged.

The revised plan reported the 30-minute overlap between the delayed Team
planning meeting and the fixed workshop. It planned departure from Team
planning at 11:30 instead of moving the workshop or pickup. It produced a
complete unsent draft for `team@example.test` that explains the new time and
the 11:30 departure.

Turn `turn:2c273a6768adb3f25d92e7dcf3b20793` exposed the draft content for
inspection. It correctly produced no caregiver draft because the pickup did
not change. The unchanged follow-up in turn
`turn:7e3045c290c6f636335aca7dc5ecdc97` reread the Calendar and Tasks, made no
updates, and confirmed the same plan.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Start from an accepted plan | Pass | Turn `turn:44be663015134799d76036ea728e4c10` accepted the plan and saved its Task blocks. |
| Detect the one-hour delay | Pass | Calendar reads after the fixture change returned Team planning at 10:00–12:00. |
| Preserve fixed commitments | Pass | The immovable appointment, customer workshop, and school pickup stayed at their recorded times. |
| Recalculate dependent work | Pass | Demo preparation moved to 09:00; the team follow-up stayed after the delayed meeting and workshop. |
| Protect personal limits | Pass | Lunch remained 13:30–14:30 and no work was planned after 16:00. |
| Draft affected notices | Pass | The inspected team draft states the 10:00 start and 11:30 departure. |
| Avoid an unnecessary notice | Pass | No caregiver draft was made because pickup timing was unchanged. |
| Keep the unchanged check quiet | Pass | The follow-up made no Task or Calendar update and sent no note. |
| Avoid external sends | Pass | The fixture trace has no Gmail send after the earlier PA-004 sends. |

## Evidence and limitations

The durable conversation records contain the accepted plan, Calendar reads,
Task inspections, Task reschedule, revised schedule, draft text, and unchanged
rerun. The fixture trace confirms the provider reads and shows no Gmail send.

The active Gmail contract has a send operation but no draft-save operation.
Therefore the notice is retained as a Noema Chat draft, not a provider Draft.
It was not sent.
