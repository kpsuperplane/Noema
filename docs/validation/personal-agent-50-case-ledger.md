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
| 2 | Identify unanswered questions in recent email threads | PASS | `turn:18c9e319a369bd68f962`; `task:18c9e31eaa7bf635f9fe`; `submission:18c9e369d469dbd0102fe` | Reviewer-approved read-only run resolved the account identity, exhausted two result pages, and individually assessed 24 messages across 23 threads. No qualifying unanswered direct question was found; no external content changed. |
| 3 | Find inbox scheduling requests and propose calendar times | PASS | `turn:18c9e33ea62a3d61fdbd`; `task:18c9e343976fd423fe56`; `submission:18c9e378a3484d521042a` | Reviewer-approved read-only run inspected all 24 returned messages across 23 threads and the full Aug 8–22 primary-calendar window. No qualifying scheduling request was present, so it correctly returned no invented slots and documented the connector count discrepancy. |
| 4 | Find decisions and unresolved issues for an upcoming meeting | PASS | `turn:18c9e34e1b767d1fff8e`; `task:18c9e35234bcd134fff3`; `submission:18c9e367a9e81d5a102a9` | Reviewer-approved read-only run identified the Aug 14 Weekly Review occurrence, followed both linked Notion pages, separated one confirmed decision from three open issues with known owners, and documented the timing conflict and zero supported Gmail matches. |
| 5 | Identify overbooked days and suggest calendar changes | PASS | `turn:18c9c5c3bb9af3f7168f` | Typed empty event result for August; correctly reported no overloaded days and made no changes. |
| 6 | Find and reserve a two-hour focus block | PASS | `turn:18c9c5db0d0a349b190a` | Created `[Noema Validation 06] Focus Block` for 2026-08-10 09:00–11:00 UTC; event `v2ienu5710gg4cb29haks78lvs`; provider receipt confirmed. |
| 7 | Move a flexible event for a higher-priority commitment | PASS | `turn:18c9c5e5fe20b5701a4f` | Verified and moved the case-6 event to 13:00–15:00 UTC; no attendees or conflicts; provider receipt confirmed. |
| 8 | Add travel time around an in-person event | PASS | `turn:18c9c5f342ab943a1bd5` | Created linked travel blocks at 12:30–13:00 and 15:00–15:30 UTC; events `l4uv1u6jbu3ptvdtff71qf7k44` and `3j9jctvqgj0p837rcfrt5junq0`; target event unchanged. |
| 9 | Add preparation time before important meetings | PASS | `turn:18c9c606fd122ae61e19` | Created a linked preparation block at 12:00–12:30 UTC; event `3ho7o40spmhkn7os5626ltedp8`; no conflict. |
| 10 | Create a tentative hold from an email discussion | PASS | `turn:18c9e455a5c65a5711db6`; `task:18c9e45c00e3783411e6f`; `submission:18c9e4daf93f54cf12ca8` | Reviewer-approved run exhaustively inspected the July 10–August 8 Gmail window and found no qualifying human future-meeting proposal. It resolved and checked the primary Calendar, documented that the active mutation cannot set a genuine tentative status, and correctly made no write. |
| 11 | Create a Notion follow-up list from response-needed email | PASS | `turn:18c9e390b59f45bb10692`; `task:18c9e3954e0e4b9c10711`; `submission:18c9e3c42679e57610c46` | Reviewer-approved run inspected 24 messages across 23 threads, found no qualifying human response request, and created verified Notion page `3b6b5138-8d91-81ec-91b3-f8b4902f64b9` with the required structured empty state and uncertainty. |
| 12 | Add confirmed email deadlines to Calendar | RUNNING | `turn:18c9e3cdb7f067f710d71`; `task:18c9e3d224296e4a10dd6`; gate `gate:18c9e40b0665c0c0114f3` | Gmail review found one confirmed future date-only obligation and no duplicate. The task correctly refused to invent a time; it awaits approval/adoption of pending Calendar definition `af0738bf748899fe0e642bf16155ee5b5f3fae030b18343da7de7dbd0ac122c8`, which adds true all-day creation. No event has been created. |
| 13 | Save an email-thread summary in Notion | PASS | `turn:18c9e3997ccfed1f1077b`; `task:18c9e39dab936c341080d`; `submission:18c9e3cc296ffdd910d3e` | Reviewer-approved run chronologically summarized the longest available validation-window thread (two messages), separated facts and inference, and verified idempotent Notion page `3b6b5138-8d91-8120-8135-ef793e75af7e` with all required sections. |
| 14 | Log receipts and renewals in Notion | PASS | `turn:18c9e3e4314c3e5e10fea`; `task:18c9e3eb378604d3110e0`; `submission:18c9e40f7705c04e11571` | Reviewer-approved run inspected all ten Aug 1–8 messages across nine threads, found no qualifying receipt or renewal, and verified Notion page `3b6b5138-8d91-81f5-9a1e-e9a05d760328` with an explicit empty log, review queue, exclusions, and currency policy. |
| 15 | Track packages or reservations and calendar dates | PASS | `turn:18c9e3f9f7b5138f112b1`; `task:18c9e3ff140e77bd11350`; `submission:18c9e44b8edf20e111c93` | Reviewer-approved read-only run searched the full window and focused delivery/travel/booking terms, inspected six false positives, and found no active future calendar-worthy confirmation. It resolved and inspected the primary Calendar but made no write. |
| 16 | Create a newsletter reading digest in Notion | PASS | `turn:18c9e3a2cd212465108b2`; `task:18c9e3b0be625a1410a66`; `submission:18c9e3e8c8c8111a11088` | Reviewer-approved run inspected all 24 returned messages, selected three deduplicated later-reading threads, and verified Notion page `3b6b5138-8d91-818b-844f-eb5bca0cb97a` with topic groups, summaries, source references, methodology, uncertainty, and reading order. |
| 17 | Turn a recurring Notion responsibility into a calendar series | PASS | `turn:18c9c64e0be0519625f4` | Read the fixture, deduplicated by exact title, and created confirmed weekly series `vifa6l0o2cml8u0btk65faqtd4` with the Notion source link. |
| 18 | Create a Notion meeting agenda from email | PASS | `turn:18c9e440ef97711111b5a`; `task:18c9e4457fe74eec11be9`; `submission:18c9e45c90fd404211e8a` | Reviewer-approved run read four exact Gmail sources, consolidated three repeated OAuth advisories, and verified Notion agenda `3b6b5138-8d91-81a7-bb9d-e318f12bc763` with ordered dependencies, decisions, questions, gaps, citations, and Kevin as the sole known participant. |
| 19 | Add meeting-note action items to a Notion project | PASS | `turn:18c9c65ad891a10d2764` | Read the meeting notes and appended an idempotently marked project-actions section with exact owners, dates, status, and provenance; provider update confirmed. |
| 20 | Convert Notion deadlines into calendar milestones | PASS | `turn:18c9c7e3f5a56c1352e2` | Read two firm milestones, checked duplicates/capacity, created confirmed events `aapa16cf2953sak90msso12odc` and `v33pbjnebcg71eo7r0n5juc4dg`, and read back `remindersUseDefault: true` with no custom overrides. |
| 21 | Reserve a recurring Friday review linked to Notion | PASS | `turn:18c9c67674af450b2a73` | Checked four Fridays, found no overlap at 15:00–16:00 UTC, and created confirmed linked series `7i4fsnttqfisbflafghbrs63h8`. |
| 22 | Create a Notion relationship brief from email | PASS | `turn:18c9e480e49194a4121bf`; `task:18c9e485f202fee11225a`; `submission:18c9e4a2da3bbd1a125aa` | Reviewer-approved run used the one qualifying human relationship source, avoided inferring identity or relationship from automated mail, and verified idempotent Notion page `3b6b5138-8d91-8165-9074-c0e8d37f16db` with facts, interactions, commitments, follow-ups, references, and explicit gaps. |
| 23 | Schedule a meeting requested by email | PASS | `turn:18c9e48a5909212d122e9`; `task:18c9e49a34793cfa1249c`; `submission:18c9e4b4bae068cd1280a` | Reviewer-approved run inspected eight unique targeted messages/threads in the inclusive July 10–August 8 window. None was a qualifying human request with attendee, duration, timezone, and future timing, so it correctly created no event and made no external changes. |
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
- `426139c8` also follows the structured task ID returned by a successful direct
  `task.delegate` result, closing the race where a foreground turn completed
  before its later `TaskReference` notification reached the transcript.
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
- Case 12 demonstrated that the timed-only Calendar mutation contract cannot
  represent a source-supported date-only obligation without inventing a time.
  Reviewer-approved proposal task `task:18c9e427eb2ec98d11862` produced directly
  linked pending successor
  `af0738bf748899fe0e642bf16155ee5b5f3fae030b18343da7de7dbd0ac122c8`.
  The inspected diff adds only `create_all_day_event`, using inclusive
  `start.date`, exclusive `end.date`, no timezone/dateTime fields, no custom
  reminders, and the existing bounded receipt; human approval remains required.
- Reviewed Gmail definition `86af21d95aee7c802eda3fab256148ccba08272ae20a036b2b2ab35e61b77678`
  uses typed empty arrays, bounded message headers/body text, and an 18-message
  page within the 32 KiB worst-case response contract.
- `0ef98875` lets an existing OAuth connection adopt a reviewed manifest revision
  when only authorization-request options change, while keeping endpoints,
  scopes, client authentication, credential setup, and account identity exact.
  Reviewed Gmail successor
  `c4f8e07fa1ef79a36506241f7e09e3fd00c3e8ed9e766cf11d73324427bbe869`
  requests offline access, incremental scopes, and explicit consent; active
  connection revision 6 preserved credential, grant, and policy revisions.
  Stale validation requests `cap_auth:18c9c95c47fca43db2a` and
  `cap_auth:18c9c576cde33bd7e55` were cancelled without granting access; the
  corrected request completed and no Gmail authentication intervention remains.
- `5528a3dd` preserves adapter OAuth callback failures through the browser return
  path instead of presenting an ambiguous success, and `4389f922` accepts the
  combined scope string returned by incremental Google OAuth grants. The active
  Gmail connection now uses reviewed definition
  `6f55b913764a6d9a986726b317e54f8c8630011ca722a6084db3e2d0e8d96bf4`
  at connection revision 5; its bounded snippet-based message projection avoids
  the demonstrated invalid-MIME-body decoding failure. Case 2 then read every
  returned message without a transform or authentication failure.
