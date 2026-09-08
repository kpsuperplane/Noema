# PA-020 — Project risks

Verdict: **Pass**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8` plus synthetic dependency facts
in the request

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Execution

The initial turn `turn:4d5aad4e4137c38ee9c88770cc400370` read the Launch project
and connected Notion notes. Given the synthetic A → B → C graph, it calculated
that B's two-day slip consumed C's one-day buffer and left a one-day downstream
risk. It kept a separate vendor-slip statement as unconfirmed and recommended
recovering one day on the B-to-C path through the project owner.

The update turn `turn:bd3b507e252c2c53a1762b90ed9ad0ef` reread the project and
reported that B recovered one day. It correctly recalculated a one-day net slip
against a one-day C buffer: no current forecast slip, but zero remaining
contingency. The vendor item remained unconfirmed. Both turns were read-only.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Calculate the downstream impact | Pass | Initial review shows B's two-day slip consumes C's one-day buffer and leaves one day late. |
| Distinguish confirmed slip from rumor | Pass | The vendor statement is explicitly labelled unconfirmed in both reviews. |
| Recommend the smallest intervention | Pass | The response recommends recovering one day on B → C, not broad replanning. |
| Recalculate after recovery | Pass | Updated review shows no current forecast slip and zero remaining buffer. |
| Preserve launch context | Pass | The September 18 target and open operational risks remain in the source check. |
| Avoid unapproved changes | Pass | No records, Tasks, or messages were changed. |

## Limitation

The dependency graph and schedule facts were synthetic inputs because the
fixture's seeded project notes do not contain a formal dependency graph.
