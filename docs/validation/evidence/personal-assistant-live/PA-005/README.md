# PA-005 — Calendar audit

Verdict: **Pass after connector revision and rerun**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-gmail-v1-notion-mcp-v8`

Public fixture route: `https://noema.kevinpei.com/__pa-replay/`

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Setup

The case used a reviewed, credential-based Calendar API definition generated
from the fixture documentation. The final definition was revision `v3` with
semantic digest
`fb49b30d40489213f499028686d013dfdae5b06aabb9817ef04e1ea5b579fe02`.
The active connection was `4af2114615b7c8c61670ff8dddb4e27b`. Its policy
allowed read-only calls automatically, and it contained no write operation.
The synthetic bearer token stayed in the protected credential store.

The fixture was loaded with six account-A events for September 14–20:

- date-only personal leave from September 15 through September 17;
- two overlapping client meetings on September 16;
- a 20-minute site visit followed by a different-location customer demo with
  a 40-minute gap on September 17; and
- a Friday recurring review on September 18.

The setup used synthetic fixture PATCH and POST requests. No real calendar was
changed. Those setup requests are separate from the Noema acceptance read.

## Execution

The first detailed revision used a page size of one. Noema correctly consumed
the first three pages, but the fixture only defined page tokens through its
third page. The fourth page returned a provider `400` response. Noema then
reported the partial result instead of claiming full coverage.

Noema proposed and accepted a bounded v3 revision from the exact reviewed v2
digest. The revision retained the same GET operation, added bounded event
times, dates, time zones, recurrence rules, recurring-event IDs, locations,
attendees, and descriptions, and requested six records per page.

The rerun used one natural request:

> Check next week’s calendar for trouble. Look for conflicts, leave, and gaps
> that need travel or preparation. Give me the event times exactly as
> recorded, keep all-day dates as dates, include recurrence details, and say
> when travel time is unknown.

The final Noema turn fetched all six records. The fixture trace shows three
`GET /calendar/v3/calendars/primary/events` requests with `maxResults=6` and
`pageToken` values absent, `page-2`, and `page-3`. All returned HTTP 200.
No Noema write request occurred during the acceptance turn.

The Go adapter initially changed plain street addresses to `%20`-encoded
text while removing credential values. A narrow sanitization fix now keeps
ordinary locations unchanged. The focused regression test is
`TestDocumentCredentialAndDynamicSanitizationPreserveOrdinaryFields`.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Read a bounded next-week interval | Pass | The final turn used September 14–20 bounds and fetched all six records across three bounded pages. |
| Preserve recorded time zones | Pass | Timed events retain their `-07:00` offsets and `America/Los_Angeles` source zones in the tool results. |
| Preserve all-day dates | Pass | Leave is reported as `start_date: 2026-09-15` and `end_date: 2026-09-17`, not converted to UTC instants. |
| Preserve recurrence information | Pass | The Weekly review includes `RRULE:FREQ=WEEKLY;BYDAY=FR`. The source did not provide a separate `recurringEventId`, so Noema did not invent one. |
| Detect overlapping meetings | Pass | The launch and design reviews overlap from 10:30 through 11:00 on September 16. |
| Detect leave conflicts | Pass | Four work events are identified during the date-only leave interval. |
| Detect a travel and preparation gap | Pass | The site visit ends at 13:20, the demo starts at 14:00 at another address, and Noema reports a 40-minute gap with unknown travel time. |
| Surface preparation requirements | Pass | Launch notes, design notes, and customer-presentation preparation are listed from event descriptions. |
| Keep read-only behavior | Pass | The reviewed connection exposes only `list_events`; the acceptance turn created no action request. |
| Avoid real external changes | Pass | Every provider request targeted the synthetic fixture account A. |

## Evidence and limitations

The durable conversation records contain the prompts, three paginated Calendar
tool calls, their complete bounded results, the connector revisions, and the
final audit. The public fixture trace confirms the HTTP status and query for
each read.

The fixture has no route or geocoding operation. Therefore the result correctly
reports travel time as unknown and flags the location change instead of
inventing a duration. The source event also has a recurrence rule but no
separate `recurringEventId`; the result preserves that absence.
