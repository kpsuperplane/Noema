# Personal Agent 50-Case Live Validation Ledger

This ledger tracks live validation against the `.noema-dev` instance at
`http://127.0.0.1:3737`. The source task contracts are in
[`docs/common-personal-agent-tasks.md`](../common-personal-agent-tasks.md).

Statuses are `NOT_RUN`, `RUNNING`, `PASS`, or `FAIL`. A pass requires a
terminal foreground result or an approved terminal delegated-task result,
correct use of the connected systems, and no unrequested external effects.
Conversation, turn, task, run, action, and artifact identifiers are durable
ordinary-information evidence. Private source content is not copied here.

| # | Task | Status | Durable evidence | Notes |
| ---: | --- | --- | --- | --- |
| 1 | Summarize important emails received today by topic | PASS | `turn:18c9c383cf5fff5f12e0` | Read four messages; grouped relevant results; no writes or pending actions. |
| 2 | Identify unanswered questions in recent email threads | RUNNING | Foreground `turn:18c9c56eee78e7c6d67`; task `task:18c9c57422b074f6e00`; run `run:18c9c57422f57b44e0d` | Initial attempt failed on missing headers/body and empty arrays. Connector corrected and adopted. Rerun is suspended on exact Gmail OAuth intervention `cap_auth:18c9c576cde33bd7e55`. |
| 3 | Find inbox scheduling requests and propose calendar times | FAIL | `turn:18c9c402b0e1c02f2130` | Pre-fix run failed on Gmail and Calendar empty-array transforms. Both connectors are corrected; rerun required after Gmail auth resumes. |
| 4 | Find decisions and unresolved issues for an upcoming meeting | NOT_RUN | — | — |
| 5 | Identify overbooked days and suggest calendar changes | PASS | `turn:18c9c5c3bb9af3f7168f` | Typed empty event result for August; correctly reported no overloaded days and made no changes. |
| 6 | Find and reserve a two-hour focus block | PASS | `turn:18c9c5db0d0a349b190a` | Created `[Noema Validation 06] Focus Block` for 2026-08-10 09:00–11:00 UTC; event `v2ienu5710gg4cb29haks78lvs`; provider receipt confirmed. |
| 7 | Move a flexible event for a higher-priority commitment | PASS | `turn:18c9c5e5fe20b5701a4f` | Verified and moved the case-6 event to 13:00–15:00 UTC; no attendees or conflicts; provider receipt confirmed. |
| 8 | Add travel time around an in-person event | PASS | `turn:18c9c5f342ab943a1bd5` | Created linked travel blocks at 12:30–13:00 and 15:00–15:30 UTC; events `l4uv1u6jbu3ptvdtff71qf7k44` and `3j9jctvqgj0p837rcfrt5junq0`; target event unchanged. |
| 9 | Add preparation time before important meetings | PASS | `turn:18c9c606fd122ae61e19` | Created a linked preparation block at 12:00–12:30 UTC; event `3ho7o40spmhkn7os5626ltedp8`; no conflict. |
| 10 | Create a tentative hold from an email discussion | NOT_RUN | — | — |
| 11 | Create a Notion follow-up list from response-needed email | NOT_RUN | — | — |
| 12 | Add confirmed email deadlines to Calendar | NOT_RUN | — | — |
| 13 | Save an email-thread summary in Notion | NOT_RUN | — | — |
| 14 | Log receipts and renewals in Notion | NOT_RUN | — | — |
| 15 | Track packages or reservations and calendar dates | NOT_RUN | — | — |
| 16 | Create a newsletter reading digest in Notion | NOT_RUN | — | — |
| 17 | Turn a recurring Notion responsibility into a calendar series | PASS | `turn:18c9c64e0be0519625f4` | Read the fixture, deduplicated by exact title, and created confirmed weekly series `vifa6l0o2cml8u0btk65faqtd4` with the Notion source link. |
| 18 | Create a Notion meeting agenda from email | NOT_RUN | — | — |
| 19 | Add meeting-note action items to a Notion project | PASS | `turn:18c9c65ad891a10d2764` | Read the meeting notes and appended an idempotently marked project-actions section with exact owners, dates, status, and provenance; provider update confirmed. |
| 20 | Convert Notion deadlines into calendar milestones | PASS | `turn:18c9c7e3f5a56c1352e2` | Read two firm milestones, checked duplicates/capacity, created confirmed events `aapa16cf2953sak90msso12odc` and `v33pbjnebcg71eo7r0n5juc4dg`, and read back `remindersUseDefault: true` with no custom overrides. |
| 21 | Reserve a recurring Friday review linked to Notion | PASS | `turn:18c9c67674af450b2a73` | Checked four Fridays, found no overlap at 15:00–16:00 UTC, and created confirmed linked series `7i4fsnttqfisbflafghbrs63h8`. |
| 22 | Create a Notion relationship brief from email | NOT_RUN | — | — |
| 23 | Schedule a meeting requested by email | NOT_RUN | — | — |
| 24 | Create calendar holds for Notion project milestones | PASS | `turn:18c9c682f65542cf2be1` | Read both milestones, checked duplicates and capacity, and created confirmed 90-minute holds `2ben9d34r9huh7pbtlfbcg9ojg` and `eem95d6bu0o7jcto0l1ql5ldvc` with source links. |
| 25 | Update an event with agenda, context, and Notion link | PASS | `turn:18c9c696d5d75c4c2e0c` | Fetched both sources and updated only the linked weekly-series description; provider receipt for `7i4fsnttqfisbflafghbrs63h8` at `2026-08-08T08:18:28.740Z`; no attendee notifications applied. |
| 26 | Schedule follow-up time for assigned Notion actions | PASS | `turn:18c9c6a3712c0d422f83` | Selected only Kevin's earliest-due action and created confirmed linked focus block `mqvt2o61gtvqv14hu30vub5a34` before its deadline; Momo's work was excluded. |
| 27 | Create recurring Notion meeting notes and link the series | PASS | `turn:18c9c6b22bf06782312b` | Created and fetched child page `3b6b5138-8d91-817c-9fb8-e46d41238954`, linked it to series `7i4fsnttqfisbflafghbrs63h8`, and confirmed final 15:00–16:00 recurrence by readback. An initial zero-duration update was corrected in the same turn; the shared timezone projection was fixed afterward. |
| 28 | Turn an email thread into a linked meeting and agenda | NOT_RUN | — | — |
| 29 | Create a Notion project page from an email thread | NOT_RUN | — | — |
| 30 | Create a Notion page for an upcoming event | NOT_RUN | — | — |
| 31 | Produce a morning brief across all three systems | NOT_RUN | — | — |
| 32 | Show cross-system changes since yesterday | NOT_RUN | — | — |
| 33 | Produce a shutdown summary and tomorrow preview | NOT_RUN | — | — |
| 34 | Build a weekly preview across all three systems | NOT_RUN | — | — |
| 35 | Detect calendar conflicts and minimize changes | PASS | `turn:18c9c776a1ee1cca4723` | Detected the exact 30-minute hard overlap, fetched authoritative priority/flexibility details, and ranked one-event resolutions; correctly selected a 30-minute shift as the minimum and made no writes. |
| 36 | Prepare a meeting brief from invite, email, and Notion | NOT_RUN | — | — |
| 37 | Build profiles of today's meeting attendees | NOT_RUN | — | — |
| 38 | Compare meetings with stated priorities | PASS | `turn:18c9c78bd354133b4959` | Followed two Calendar continuations, assessed all nine commitments against authoritative Notion priorities, fetched detailed evidence, separated context-poor from low-value, and made no writes. |
| 39 | Write an end-of-week meeting and follow-up summary | NOT_RUN | — | — |
| 40 | Build a Notion contact page and link meetings | NOT_RUN | — | — |
| 41 | Plan the day around meetings, email deadlines, and priorities | NOT_RUN | — | — |
| 42 | Turn today's Notion tasks into calendar blocks | PASS | `turn:18c9c7227eb31af83d9b` | Read three exact Kevin tasks, checked the day and duplicates, and created confirmed blocks `6om1oj7o1rl5do05c5i5b1k1kc`, `0ahj1m4so1ahuindnit2rii0k8`, and `hj39v3cfr0eoap2n9jisk29b38` with estimates and breaks preserved. |
| 43 | Identify commitments at risk across all three systems | NOT_RUN | — | — |
| 44 | Reschedule focus blocks after a Notion deadline change | PASS | `turn:18c9c7424963677f4139` | Confirmed the Notion deadline change, moved only linked event `hj39v3cfr0eoap2n9jisk29b38` from Aug 8 to conflict-free Aug 10 16:00–16:30 UTC, updated its due-date description, and verified preserved fields by readback. |
| 45 | Update a Notion project status from email and calendar activity | NOT_RUN | — | — |
| 46 | Create a cross-system research brief in Notion | NOT_RUN | — | — |
| 47 | Maintain a Notion decision log from email and notes | NOT_RUN | — | — |
| 48 | Build a Notion travel itinerary and calendar reservations | NOT_RUN | — | — |
| 49 | Review stale Notion tasks using recent email evidence | NOT_RUN | — | — |
| 50 | Generate a monthly review in Notion | NOT_RUN | — | — |

## Shared corrections proven so far

- Shared Notion fixture page
  `3b6b5138-8d91-814f-82c7-ff9168472e15` was created once by
  `turn:18c9c614e33083881fa4` for the Notion and cross-system cases.

- `2ba077cd` adds exact turn correlation and safe read-only status recovery for
  the live GraphQL runner.
- `6fce64f8` adds bounded UTF-8 base64url decoding to the reviewed response
  sandbox.
- `4756bad2` lets large canonical adapter definitions be revised through exact
  existing-value replacements while preserving the normal pending-review and
  compiler path.
- `862ef26c` omits top-level null MCP arguments only when the reviewed tool
  schema declares the property optional and rejects null. Live read-only proof
  `turn:18c9c841d4b7984d103` completed both Notion private- and shared-page
  listings even though the model proposed `cursor: null`.
- `ad1d942a` lets reviewed nested JSON request templates omit missing or null
  optional properties and resulting empty array entries. `b0936a0a` follows a
  definition's complete replacement lineage when adopting a reviewed revision,
  including drafts revised before review.
- Reviewed Calendar definition `fe89586eae0a62ec683fd497edafce2ca6c7565207c14f7132d11789e1d62e78`
  uses explicit typed empty arrays in event/list transforms.
- Case 27 exposed missing IANA timezone fields in `get_event`. Reviewed successor
  `89b96bd880a0928b432399b4dd81a6df2ea160bf454cd813ab97b477b775d5a2`
  adds only bounded `startTimeZone` and `endTimeZone` projections; read-only proof
  `turn:18c9c707326ec2483a8b` returned `Etc/UTC` for both.
- Case 20 required auditable reminder evidence. Reviewed successor
  `61c63845b2900bb2702723fad6c67448ba589568d3de70d5414e11fe0cf941f9`
  adds only bounded default-reminder and override projections; both milestone
  readbacks proved the requested primary-calendar default policy was active.
- Reviewed Calendar successor
  `b9ada00d28021ba41a2197f4f5a648bf2dcd4563d2679cfbaf6171677661ea3c`
  changes only `set_event_attendees` from exactly four required addresses to
  one required and three optional addresses. The full manifest diff was
  inspected before approval; active connection revision 9 retained the same
  credential, grant, policy, scopes, operations, and request template.
- Reviewed Gmail definition `86af21d95aee7c802eda3fab256148ccba08272ae20a036b2b2ab35e61b77678`
  uses typed empty arrays, bounded message headers/body text, and an 18-message
  page within the 32 KiB worst-case response contract.
