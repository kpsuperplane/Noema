# PA-010 — Goal review

Verdict: **Pass after week-boundary correction**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8` plus native Learning project and
Calendar records

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Setup

The Learning project contained a completed `Study session` Task with a saved
result and review. The result records two hours completed on Sunday,
September 6, 2026. The open `Complete study exercises` Task was due Tuesday,
September 15 at 16:00 Pacific. The Calendar contained the existing fixed
commitments and the saved user preference to stop weekday work at 16:00.

The user added a Friday-unavailable constraint in the starting request.

## Execution

The initial turn `turn:4069d4fa896bcda0c898dff02d13750c` read the Learning
project, Tasks, memory, and Calendar. It updated the open Task, but counted
the completed September 6 session outside the goal period and planned four
new hours.

The correction in turn `turn:a524c8cbeaa9348ecd6c5cacd10f3f85` supplied the
explicit Sunday–Saturday period containing September 6. Noema reread the
saved evidence and corrected the Task document:

- target: four hours for September 6–12;
- completed evidence retained: two hours on September 6;
- remaining: two hours;
- remaining session: Thursday, September 10, 16:00–18:00 Pacific;
- Friday remains unavailable.

No Calendar event was created. The open Task remained captured in the native
Learning project at revision 4 with the corrected plan in its Task document.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Use actual completion evidence | Pass | The corrected Task document retains the two-hour September 6 result and review. |
| Calculate the shortfall | Pass | The corrected document states a four-hour target, two completed hours, and two hours remaining. |
| Apply the Friday constraint | Pass | The corrected document states that Friday, September 11 is unavailable. |
| Save the revised plan | Pass | `tasks get task:f029d60b8a5a28266128812c87e63794` shows revision 4 and the corrected document. |
| Keep the plan within remaining capacity | Pass | One two-hour Thursday block is proposed, with existing Calendar commitments preserved. |
| Preserve completion evidence | Pass | The correction removed the erroneous extra Sunday plan, not the completed-session record. |
| Avoid an unnecessary external write | Pass | No Calendar event or provider mutation was created. |

## Evidence and limitation

The first pass exposed a week-boundary ambiguity. The accepted correction
made the period explicit and reconciled the saved Task against the completed
Task result. The case does not claim that a Calendar study event exists,
because the user asked to revise the saved plan and the corrected run did not
create one.
