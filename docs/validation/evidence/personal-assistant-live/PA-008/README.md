# PA-008 — Weekly review

Verdict: **Pass after bounded-range correction and full source review**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Setup

The native Task store contained two Projects:

| Project | ID | Description |
| --- | --- | --- |
| Launch project | `project:a792173ec0873ecd68ba39f71b5cd3c3` | Launch work and client delivery. |
| Learning project | `project:83b2679cb6c6030ea1152ae4ac94d0d0` | Study and learning commitments. |

Each Project had one completed Task with a saved result and review:

- `Launch retrospective` records that the launch shipped on September 5;
- `Study session` records two hours of study completed on September 6.

Each Project also had one open Task with a next-week deadline:

- `Confirm launch metrics`, two hours, due September 16 at 16:00 Pacific;
- `Complete study exercises`, three hours, due September 15 at 16:00 Pacific.

The synthetic Calendar held five records during September 7–13 and six records
during September 14–20. The next-week records include September 15–17 leave,
two overlapping September 16 reviews, the September 17 site visit and demo,
and the September 18 weekly review.

## Execution

The first request was natural: “Help me review the week and get ready for next
week.” Noema combined both weeks into one Calendar request. The fixture
returned its defined pages, then rejected the next page with HTTP 400 because
the fixture exposes only three page tokens. Noema reported a partial review
and asked for the user's desired outcomes. It had not read Tasks or Projects.

The correction in turn `turn:ee031cf660251072404338094a639e77` required two
separate bounded Calendar ranges and a read of the current Tasks and both
Project documents. Noema then:

1. read September 7–13 through all three available Calendar pages;
2. read September 14–20 through all three available Calendar pages;
3. inspected the completed and open Task documents; and
4. read both Project documents before writing the review.

The final review separated completed outcomes from open work. It carried the
September 15 and 16 deadlines forward, identified leave and overlapping
meetings, and calculated that the September 9 work stack could not fit before
the 16:00 stop. It recommended placing the three-hour study Task on Monday,
September 14, and resolving the launch-metrics deadline before leave.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Review a full current week | Pass | The corrected turn read September 7–13 with separate bounded Calendar pagination. |
| Review a full next week | Pass | The corrected turn read September 14–20 with separate bounded Calendar pagination. |
| Separate completed outcomes from activity | Pass | The review cites the completed Launch retrospective and Study session results, and does not count them as open work. |
| Carry open commitments | Pass | The review lists the September 9 stack and both next-week Project Tasks with estimates and deadlines. |
| Attribute work to both Projects | Pass | Launch project and Learning project are named with their document descriptions and Task membership. |
| Reconcile capacity | Pass | The review calculates the September 9 overload and identifies leave, overlaps, and usable next-week windows. |
| Preserve date-only leave | Pass | Personal leave remains September 15–16 as a date block, not a UTC instant. |
| Surface the September 16 conflict | Pass | Client launch review and Client design review are reported as a 30-minute overlap during leave. |
| Produce an actionable carry-forward | Pass | The review recommends a Monday study block and an explicit decision on the launch-metrics deadline. |
| Keep the case read-only | Pass | The corrected review created no Calendar, Task, Project, message, or external write. |

## Evidence and limitations

The durable records contain the initial partial review, the correction, six
Calendar reads, both Project reads, completed Task results and reviews, open
Task inspections, and the final weekly review. The fixture trace confirms
separate bounded reads after the initial invalid combined-page request.

The first combined range exposed a fixture page-token limit. The accepted run
uses two source-bounded reads, which is the safe behavior when one broad range
cannot be paged to completion.
