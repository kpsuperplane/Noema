# Live Noema acceptance plan: connections and 100 assistant tasks

Date: 2026-09-06
Status: In progress. The direct local CLI is implemented and validated. Gmail,
Notion, and Calendar setup work is live. PA-001 through PA-003 are accepted.
PA-004 passes after a continuation-authorization repair and rerun. PA-005
passes after a reviewed Calendar connector revision and rerun. PA-006 passes
after a source-read correction and full Task-document reread. PA-007 passes
after a synthetic delay, draft inspection, and unchanged rerun. PA-008 passes
after bounded Calendar ranges and a full Project and Task review. PA-009
through PA-022 pass. PA-023 passes after a reviewed Gmail attachment
operation, scoped reauthorization, and a revised packet. PA-024 passes after
synthetic review routing, two v2 approvals, and finalization. PA-025 passes
after a synthetic expense API connector, governed claim submission, and status
verification. PA-026 passes after a native-Task handoff draft, restricted-note
exclusion, synthetic Gmail delivery, and read-back. PA-027 passes after a
documented synthetic credential API connector, response-contract repair, two
approved course enrollments, completion evidence, and one approved renewal
with status verification. PA-028 passes after a connector revision, a Go
adapter enablement repair, and a synthetic deadline and follow-up rerun.
PA-029 passes after a working-directory repair, two validated synthetic
application packets, four saved PDFs, two reviewed synthetic submissions, and
idempotent retries. PA-030 passes after a synthetic offers connector was
trimmed, accepted, policy-configured, and used for a fresh read before the
comparison. PA-031 passes after four focused synthetic products connector
revisions, fresh reads, landed-cost comparison, and a saved recommendation.
PA-032 passes after four bounded synthetic research connector revisions, a
complete six-source read, source-authority synthesis, and a saved brief.
PA-033 passes after a reviewed synthetic topic-monitor API revision, a saved
baseline, duplicate suppression, one thresholded change notice, and a quiet
unchanged rerun. PA-034 passes after a focused literature-index projection
revision, two live six-record reads, deduplication to nine works, retraction
handling, mixed-evidence synthesis, and a saved evidence table. PA-035 passes
after documented connection setup, a source-linked five-claim review, and a
saved fact-check report. PA-036 passes after five bounded connector revisions,
explicit nullable-serial handling, a full 20-record read, and independent
artifact inspection. PA-037 passes after a profile contract repair, a complete
ledger read, duplicate and refund reconciliation, equivalent-period analysis,
and independent artifact inspection. PA-038 passes after a bounded newsletter
API connection, duplicate suppression, budgeted topic selection, changed-story
detection, and independent artifact inspection. PA-039 passes after a focused
progress projection repair, prerequisite-aware rescheduling, two bounded plan
revisions, and a quiet unchanged rerun. PA-040 passes after a focused course
projection repair, a clean comparison rerun, and one approved synthetic
enrollment with receipt verification. PA-041 passes after a focused bill-ID
projection repair, a two-week cash-flow plan, duplicate-notice reconciliation,
and one approved synthetic payment with receipt verification. PA-042 passes
after two reviewed tax-API projection repairs, a complete five-record read,
duplicate and supersession handling, missing-form retrieval, and independent
packet inspection. PA-043 passes after three focused subscription-API mapping
repairs, one approved monthly cancellation, complete receipt capture, and a
post-cycle no-charge check. PA-044 passes after a reviewed synthetic coverage
API connection, two focused projection repairs, complete policy and asset
reads, and a sourced coverage-fit review. PA-045 passes after replacing an
expired temporary tunnel, reviewing the exact claim connector revision, one
approved synthetic follow-up, and paid-status verification. PA-046 passes
after three reviewed connector revisions, a repaired six-read packet, one
approved synthetic application, and receipt/renewal verification. PA-047
passes after three reviewed connector revisions, explicit duplicate statement
handling, fee comparison, and a final read-only rollover review. PA-048 passes
after two connector repairs, explicit `posted: false` preservation, one
approved synthetic dispute, and corrected-report verification. PA-049 passes
after three focused connector revisions, policy setup, one approved synthetic
appointment, one approved synthetic renewal, and later status verification.
PA-050 passes after three focused connector revisions, policy setup, one approved
synthetic complaint, and later status verification. PA-051 passes after four
focused estate-map connector repairs, policy setup, one approved synthetic
review update, later status verification, and a sourced estate artifact. PA-052
passes after ten reviewed connector revisions, five approval-gated synthetic
notices, final reconciliation, and a sourced estate-notice artifact. PA-053
through PA-100 remain not run.

Current ledger: [live acceptance evidence](../validation/evidence/personal-assistant-live/README.md).

## Objective and numbering

Use the running Go Noema development instance to complete realistic assistant tasks through its normal tools and connections.
A test operator interacts directly with Noema through GraphQL and examines each outcome before advancing.

Run **Setup case 1: Gmail API** and **Setup case 2: Notion MCP** first.
Then run **PA-001 through PA-100**, preserving the original [100-task assessment](../difficult-digital-personal-assistant-tasks.md) numbering.
There are 102 top-level cases. The setup cases are additional prerequisites, not replacements for daily briefing and promise tracking.

The user authorized accepting synthetic connection proposals on their behalf.
Approval must target the inspected proposal and current revision through the normal product interface.
Real bank transactions, purchases, flight bookings, and other real external changes remain outside the test authority.

## Corrected baseline

The earlier [100-case replay report](../validation/evidence/personal-assistant-replay/2026-09-06-live-rerun.md) is not valid acceptance evidence.
Its wrapper marked successful process exits as completed tasks.
Inspection found all 100 responses reported blocked fixture access, with two failed browser activities per case and no delegated Tasks.
Those attempts prove message delivery, not task completion or Go parity.
Later browser reads of minimal fixture metadata do not satisfy the task contracts below.

Implementation must mark the old report as superseded before recording new results.
Do not import its completion counts into the new ledger.

## Execution method

1. Check Git status and the existing development session.
2. Verify authenticated access through `/tmp/noema-codex/graphql.sock` using the documented authentication-status request.
3. Use the existing development home. Inspect it through `/tmp/noema-codex/home`; open SQLite in read-only mode.
4. Prepare the current case's synthetic records, expected outcomes, and allowed actions.
5. Send a natural human request through Noema Chat.
6. Inspect the exact turn, tool arguments, results, connection IDs, and any delegated Task.
7. Inspect each proposal or action request before accepting its exact revision.
8. Follow the same turn or Task through approval, correction, and completion.
9. Compare the result with the case's acceptance criteria and independent fixture state.
10. Save the verdict and evidence before starting the next case.

A blocked outcome starts investigation. It is not a pass or a reason to advance.
Diagnose the cause as fixture setup, connection setup, product failure, or a missing human decision.
Repair the demonstrated cause and rerun the same case.
Use predetermined synthetic answers for fixture questions.
Request the real user's input only when their authority or a real decision is actually missing.

The operator must read the substantive output. Keyword matching and process exit codes cannot decide acceptance.
Do not place the expected answer in Noema's prompt or its accessible fixture documentation.
Use normal conversation follow-ups. Avoid implementation instructions unless the case explicitly tests setup or recovery.

## Direct local CLI

Use the existing `noema` executable as the only operator interface.
The CLI sends GraphQL through the private local Unix socket and keeps the normal
Noema authorization boundary. It does not create a second test API.
The implementation and validation are in commits `3251b51a` and `f39a03b4`.
The complete command reference is [`docs/cli.md`](../cli.md).

| Command | Behavior |
| --- | --- |
| `noema status` | Verify the running server through the private socket. |
| `noema chat send [TEXT]` | Submit one message and stream its matching Chat events. `--file` reads the exact message. |
| `noema chat read` / `noema chat watch` | Read a transcript page or subscribe to future conversation events. |
| `noema tasks list/get/create/run/cancel/watch` | Inspect and change Tasks with current revision and generation checks. |
| `noema connections list` | Show registered API definitions, connections, and MCP servers. |
| `noema proposals list/accept DIGEST` | Inspect and accept one exact API proposal by semantic digest. |
| `noema interventions list` | Inspect pending setup, authentication, and action requests. |
| `noema actions list/resolve ID REVISION DECISION` | Inspect or resolve one exact governed action. |
| `noema api [DOCUMENT]` | Execute any current GraphQL query, mutation, or subscription without adding a wrapper. |
| `noema schema` | Print the schema exposed by the running server. |

Use schema-appropriate revision fields; adapter proposals may use semantic digests rather than integer revisions.
Return structured JSON by default. Optional summaries must link to the complete saved output.
Follow pagination. Do not truncate JSON or discard screenshots, artifacts, errors, or tool results silently.
Store large payloads separately with explicit references when necessary.
Keep credentials in protected stores and secure transport bindings. Never print them into evidence.
Scope intervention reads to the current case; unrelated old approvals must not decide its outcome.
Do not auto-approve, grade answers, retry mutations, or launch the next case.
An observation timeout means observation stopped. Inspect the existing execution before submitting anything again.
Prefer event subscriptions; use bounded status queries only where subscriptions are unavailable.

Verify three distinct CLI risks: complete paginated reads, stale approval rejection, and observation recovery without duplicate submission.
Reuse existing transport behavior where useful. Remove the replay loop and synthetic auto-approval path after their replacement works.

## Mock services and protocol fidelity

The mocks supply external records. Noema remains the real application, with its real model, tools, reviews, and storage.
Publish readable service documentation at stable mock URLs that Noema can fetch.
Keep fixture reset, fault controls, and expected answers outside the agent-facing service catalog.
Expose no private Noema socket or real user information through public fixture routes.

Before implementing each provider mock, save the official specification source and version used.
Capture the relevant request and response schemas, required headers, authentication scopes, pagination, and error examples.
Compare mock responses with those captured schemas. Unsupported operations must fail explicitly.
Do not return a generic `items` envelope for unrelated endpoints.
A provider-shaped label is not proof of a provider-compatible contract.

| Service | Required contract |
| --- | --- |
| Gmail API | Selected Gmail v1 profile, message list, message get, thread get, and attachment operations. Preserve IDs, labels, MIME structure, encoded bodies, timestamps, and pagination. |
| Gmail authentication | Mock authorization server with the selected OAuth flow, scope enforcement, expiry, refresh, denial, and account identity. Use protected bindings for synthetic credentials. |
| Notion MCP | Pin one official Notion MCP implementation or published server version. Match its actual tool names, schemas, discovery, results, errors, transport, and authentication. |
| Notion records | Match the selected Notion API version for pages, blocks, search, and database or data-source operations. Preserve version-specific IDs and pagination. |
| Calendar API | Add through Noema's documentation-to-proposal flow before PA-001. Match the selected Google Calendar v3 operations and their scopes, bounds, recurrence, and dates. |
| Other services | Add documented mock interfaces when the first dependent case needs them. Reuse established connections for later cases. |

Notion REST and Notion MCP are distinct contracts. Do not invent MCP tools and describe them as the official hosted server.
If a selected specification is unavailable, resolve that contract choice before claiming faithful emulation.
Authentication emulation cannot prove real Google or Notion consent compatibility. Record that limitation separately.

Use two synthetic accounts to expose accidental cross-account reads.
Create distinct provider-shaped record IDs; place a human-readable run marker in appropriate labels and metadata.
Keep seeds deterministic. Record the scenario version and reference time.
Do not replace the process clock globally. Use dated records and real short schedules for lifecycle checks.
Native Tasks, Projects, Memory, and Artifacts must use normal Noema write paths and remain in its development home.
Shared Noema workspaces are excluded; household coordination uses one personal workspace and authorized synthetic participants.

## Setup case 1 — Documentation to Gmail API connection

**Setup:** Publish a readable Gmail mock API guide and the pinned Gmail contract.
Provide two accounts with distinct messages. Account A has seven messages over three pages, two threads, a MIME attachment, and a known empty search.
Require OAuth and the documented scopes. Keep account B records inaccessible under account A's grant.
Provide one recoverable expired-token condition and one temporary rate-limit response.

**Request:** “Connect this mail service using its documentation, then show me the messages about the upcoming trip.”

**Acceptance:**

- Noema fetches the documentation in the live transcript and generates the API proposal itself.
- The proposal compiles and appears in Noema's normal review surface.
- Its origin, operations, arguments, response conversion, scopes, and pagination match the pinned mock contract.
- The test operator inspects and accepts that exact proposal. The operator does not write or install the adapter definition directly.
- Complete synthetic account authorization through the normal setup flow; grant only the documented access.
- A subsequent Noema turn calls the registered API connection. Reading the mock webpage alone does not satisfy this criterion.
- Fixture request records and Noema tool results prove the expected account, query, all required pages, full messages, and attachment access.
- Noema correctly reports the seven records, exact matching trip messages, and the empty-search result.
- Expiry and rate-limit recovery complete without changing account scope or losing records.
- Another turn reuses the saved connection without generating a duplicate proposal.
- A delegated Task can use the same connection under its permitted scope.
- Save documentation version, proposal revision, connection ID, turns, Task, request trace, and inspected output.

## Setup case 2 — Documentation to Notion MCP connection

**Dependency:** Setup case 1 has passed. Retain its connection.

**Setup:** Publish the pinned Notion MCP guide, supported discovery document, and actual MCP endpoint.
Seed two accessible pages, nested blocks across multiple pages of results, one related record collection, and a second account with inaccessible pages.
Provide synthetic authentication, expired credentials, an invalid cursor response, and a known empty search.
Discovery must return the pinned server's actual tools and schemas.

**Request:** “Connect this notes service from its documentation and summarize the launch project.”

**Acceptance:**

- Noema fetches the service documentation and initiates its normal MCP connection proposal or setup flow.
- The test operator accepts the exact proposed endpoint and access through the product, then completes synthetic authorization.
- Noema completes MCP initialization, tool discovery, and calls through the registered connection.
- The stored catalog matches the pinned server version; unknown tools and malformed arguments fail explicitly.
- Search finds the correct project. Page and block retrieval recover the full nested content across pagination.
- The summary cites the expected page IDs and preserves contradictory notes and missing evidence.
- Empty search, expired credentials, and invalid cursor produce correct outcomes; recovery retrieves the same authorized records.
- The second account's inaccessible page is not returned.
- A later turn and a delegated Task reuse the connection without duplicate installation.
- Save the setup request, accepted revision, connection ID, discovered catalog, tool calls, fixture requests, and inspected result.

## Downstream setup and shared acceptance

Before PA-001, connect the Calendar mock through the same documentation, proposal, acceptance, and verified-read sequence.
Set a synthetic user's timezone, working hours, priorities, and contact preferences through normal Noema conversations.
Every downstream case uses registered connections for available provider data.
Browser portals remain appropriate for booking, banking, forms, and other browser workflows.

The table below names minimum records. Expand their content enough to support the requested outcome.
Use a fixed answer key outside Noema's context for amounts, identities, deadlines, duplicates, and expected changes.
The setup column also determines required source capabilities. Connect any additional service before running the dependent case.

Every case inherits these requirements:

- Read current, non-empty required sources through real Noema tools. Preserve material record IDs and source links.
- Apply saved user constraints and distinguish facts, inferences, and missing information.
- Produce and inspect the actual deliverable, not merely an outline or limitation report.
- For delegated work, inspect request, execution, result, and review; Task completion alone is insufficient.
- For writes, inspect the approved values, fixture receipt, and subsequent status read. Verify the effect occurred exactly once.
- For monitoring, run baseline, unchanged, material-change, and resolved stages. Inspect actual notifications and saved state.
- For recovery, keep the existing turn or Task and inspect continuation after the injected interruption.
- For high-stakes cases, use synthetic rules and professional responses; verify the stated review boundary without making real regulated decisions.
- Missing information must be obtained from the seeded fixture or predetermined participant response before acceptance.
- An artifact case requires downloading and inspecting the generated content in its requested format.

Require actual restart recovery for PA-002, PA-009, PA-076, and PA-093 at their saved-state or uncertain-action boundary.
Verify ordinary later-turn continuity for all other multi-stage cases.
Mock device operations in PA-097 prove only the selected mock interface; physical device compatibility requires separate evidence.

## The 100 assistant cases

Rows without a recorded verdict start **Not run**. Requests below are natural
starting messages; follow-ups supply fixture decisions without revealing the
answer key.

| ID | Outcome | Setup and later change | Starting request | Case-specific acceptance |
| --- | --- | --- | --- | --- |
| PA-001 | Pass | Gmail: urgent request and duplicate reminder. Calendar: two meetings. Notion: deadline. Memory: 16:00 stop time. | Give me today's brief. | Reconcile four sources; count the deadline once; identify preparation, urgency, and work beyond capacity. [Evidence](../validation/evidence/personal-assistant-live/PA-001/) |
| PA-002 | Pass | Three promises with owners and dates; one repeated email. Later extend one date and supply one completion receipt. | Keep track of what I owe people and what they owe me. | Save three promises with sources; preserve the date replacement; close only the receipted promise; unchanged check creates no duplicate. [Evidence](../validation/evidence/personal-assistant-live/PA-002/) |
| PA-003 | Pass | Email and Notion repeat one agreed action; another paragraph only proposes a date. | Pick out what I need to do from these updates. | Create one native Task and one agreed calendar item; exclude speculation; repeated intake creates neither again. [Evidence](../validation/evidence/personal-assistant-live/PA-003/) |
| PA-004 | Pass after repair and rerun — [evidence](../validation/evidence/personal-assistant-live/PA-004/) | Five threads: urgent client ask, overdue promise, friend, newsletter, resolved request. Later add an inbound correction. | Help me get through my replies. | Order by urgency and relationship; draft correct replies; preserve the correction; close only after mock send receipts; unchanged check stays quiet. |
| PA-005 | Pass after connector revision and rerun — [evidence](../validation/evidence/personal-assistant-live/PA-005/) | Recurring event, all-day leave, two overlapping meetings, and 40-minute travel between 20-minute gaps. | Check next week's calendar for trouble. | Read bounded instances; preserve timezones and recurrence data; flag overlaps, leave, travel, and preparation without treating all-day dates as UTC instants. |
| PA-006 | Pass after correction and full-document reread — [evidence](../validation/evidence/personal-assistant-live/PA-006/) | Four hours of fixed meetings; six hours of work; lunch and 16:00 stop preference. | Help me make a realistic plan for tomorrow. | Read full Tasks and connected calendar; protect breaks; explicitly defer excess work; do not invent free time. |
| PA-007 | Pass after delay, draft inspection, and unchanged rerun — [evidence](../validation/evidence/personal-assistant-live/PA-007/) | Accepted daily plan; meeting moves 60 minutes; dependent pickup; one immovable appointment. | My first meeting is running late. Rework the day. | Update dependent items and draft affected notices; preserve fixed appointments; unchanged follow-up creates no repeated update. |
| PA-008 | Pass after bounded-range correction and full source review — [evidence](../validation/evidence/personal-assistant-live/PA-008/) | Seven days of events; completed and open native Tasks; two projects; next-week deadlines. | Help me review the week and prepare for next week. | Separate completed outcomes from activity; carry open commitments; reconcile next-week capacity; cite each project and calendar bound. |
| PA-009 | Deadline tracking | Two sources name one renewal; another date is tentative. Later pass due time, then add a receipt. | Keep these deadlines from slipping. | Track one firm renewal; exclude tentative date; issue one overdue notice; close on exact receipt; unchanged checks stay quiet. |
| PA-010 | Goal review | Goal: four study hours weekly. Actual events total two hours. New constraint removes Friday availability. | How am I doing on my learning goal? Adjust the plan. | Calculate two-hour shortfall; use actual completion evidence; revise the saved plan within remaining capacity. |
| PA-011 | Conflicting requests | Manager asks Friday delivery; client asks Thursday; dependency arrives Friday; priority preference favors existing contract. | Help me resolve these competing requests. | Identify impossible combination; explain priorities and dependencies; draft concrete escalation options without making commitments. |
| PA-012 | Conversation monitoring | Thread expects a reply at 15:00; unrelated reply arrives; later substantive answer arrives. | Watch this thread and let me know if I need to act. | Ignore unrelated change; alert once after missed deadline; close after substantive reply; unchanged check stays quiet. |
| PA-013 | Meeting coordination | Three calendars in different zones; working hours; one recurring exclusion; 45-minute meeting and 15-minute buffers. | Find a time for the three of us next week. | Find a valid shared slot; explain rejected alternatives; after scoped approval create one mock event and verify attendees and local times. |
| PA-014 | Meeting preparation | Invite, prior thread, decision page, and restricted personnel note; two unresolved decisions. | Prepare me for this meeting. | Produce sourced agenda and participant context; surface both decisions; exclude personnel details from shareable brief. |
| PA-015 | Audience updates | One project contains conflicting delivery dates, internal cost figures, and customer-safe facts. | Write an internal update and a customer update. | Resolve or label date conflict; keep shared facts consistent; exclude internal costs from customer version; save both drafts. |
| PA-016 | Meeting follow-through | Minutes include two decisions, three assigned actions, and an unaccepted suggestion; later one action completes. | Turn these meeting notes into follow-through. | Save decisions and three owned Tasks; exclude suggestion; follow up overdue action; close only completed action; no duplicate on reread. |
| PA-017 | Relationship brief | Two contacts share a name; distinct addresses and histories; upcoming meeting; confidential personal detail. | Remind me where things stand with Jordan before our meeting. | Resolve correct identity; summarize relevant history and promises; cite sources; omit irrelevant confidential detail. |
| PA-018 | Relationship cadence | Three contacts with different cadence preferences; one has declined contact; later an occasion changes. | Help me stay in touch thoughtfully. | Save feasible reminders; honor no-contact preference; update changed occasion; do not send repeated or unsolicited messages. |
| PA-019 | Project status | Native project with four Tasks; Gmail delay; Notion milestone conflict; spend 1200 against 1500 budget. | Where does this project stand? | Read full project and Task documents plus connections; report remaining 300 budget, schedule conflict, blockers, and owners with evidence. |
| PA-020 | Project risks | Dependency graph A→B→C; B slips two days; C has one-day buffer; separate unconfirmed rumor. | What is at risk in this project? | Identify one-day downstream overrun; distinguish rumor from confirmed slip; recommend smallest intervention; update when B recovers. |
| PA-021 | Decision history | Decision D1 selects vendor A; discussion mentions B; later approved D2 replaces D1 with B. | Keep our decision history straight. | Save D1 and D2 with rationale, owner, and source; mark replacement without deleting D1; exclude discussion-only proposal. |
| PA-022 | Ambiguous goal | Goal: launch club website; unknown audience and deadline; fixture operator has predetermined answers and 10-hour budget. | Help me get this website launched. | Ask material questions; incorporate supplied answers; save native project and sequenced Tasks with measurable completion and feasible effort. |
| PA-023 | Pass after Gmail attachment capability was added and the packet was revised — [evidence](../validation/evidence/personal-assistant-live/PA-023/) | Email attachment v1, approved Notion v2, spreadsheet totals, and missing appendix. | Assemble the final briefing packet. | Use v2; reconcile totals; identify missing appendix; obtain fixture appendix before final output; verify saved artifact content and download. |
| PA-024 | Pass after synthetic review routing, two v2 approvals, and finalization — [evidence](../validation/evidence/personal-assistant-live/PA-024/) | Two reviewers, document v1, conflicting comments; v2 resolves comments; only v1 has prior approval. | Get this document ready for approval. | Route v2 to mock reviewers; reconcile conflicts; require both v2 approvals; do not reuse v1 approval; save exact final version. |
| PA-025 | Pass after synthetic expense API connector, governed claim submission, and status verification — [evidence](../validation/evidence/personal-assistant-live/PA-025/) | Receipts 40, 60, 25; duplicate 60; policy excludes 25; bank confirms all three charges. | Submit my reimbursable expenses. | Request 100 once; explain excluded 25; link receipts; submit through mock portal after approval; verify submitted and paid status. |
| PA-026 | Pass after native-Task handoff draft, restricted-note exclusion, synthetic Gmail delivery, and read-back — [evidence](../validation/evidence/personal-assistant-live/PA-026/) | Departing role owns three open Tasks; stale access list; replacement contact; restricted notes. | Prepare a handoff for my replacement. | Save current responsibilities, decisions, blockers, contacts, and access requests; exclude restricted notes; verify mock delivery to exact recipient. |
| PA-027 | Pass after connector contract repair and full synthetic renewal — [evidence](../validation/evidence/personal-assistant-live/PA-027/) | 12 credits required; 8 valid, 2 duplicate, 2 expired; deadline in 30 days. | Help me renew my professional credential. | Calculate four valid credits missing; schedule eligible courses; collect completion evidence; submit mock renewal once and verify status. |
| PA-028 | Pass after connector revision, Go adapter enablement repair, and synthetic end-to-end rerun — [evidence](../validation/evidence/personal-assistant-live/PA-028/) | Five jobs; two violate location constraint; three applications with different stages; one changed deadline. | Organize my job search and next steps. | Exclude unsuitable roles; save three distinct stages and next actions; update deadline; verify mock follow-up receipt without duplicate submission. |
| PA-029 | Pass after working-directory repair and synthetic end-to-end rerun — [evidence](../validation/evidence/personal-assistant-live/PA-029/) | Verified resume, two job descriptions, contradictory old resume, required PDF fields. | Prepare applications for these two roles. | Use verified experience; tailor two packets without invented claims; validate required fields and exported PDFs; submit only scoped mock applications. |
| PA-030 | Pass after connector policy setup and fresh API read — [evidence](../validation/evidence/personal-assistant-live/PA-030/) | Offer A: 100000 salary plus 10000 bonus; B: 108000 salary; different commute, leave, and uncertain equity. | Help me compare these offers and negotiate. | Separate guaranteed and uncertain compensation; quantify commute and leave tradeoffs; apply preferences; draft questions and negotiation without accepting. |
| PA-031 | Pass after four connector projection revisions and fresh API reads — [evidence](../validation/evidence/personal-assistant-live/PA-031/) | Three products; 600 total budget; required compatibility; shipping and tax make cheapest sticker price exceed budget. | Find the best option for me. | Compute landed costs; reject incompatibility and over-budget choice; cite current mock listings; save recommendation and uncertainty. |
| PA-032 | Pass after four bounded connector revisions and a saved brief — [evidence](../validation/evidence/personal-assistant-live/PA-032/) | Six dated mock sources: primary report, correction, two summaries, dissent, and outdated article. | Bring me up to speed on this issue. | Read primary evidence and correction; separate dates and interpretations; explain dissent and coverage; cite material claims. |
| PA-033 | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-033/) | Baseline three sources; later duplicate article, then material rule change. | Watch this topic for changes that affect me. | Save baseline; suppress duplicate coverage; issue one relevant change notice with changed facts; unchanged check stays quiet. |
| PA-034 | Literature review | 12 papers across two mock indexes; three duplicate records; one retraction; conflicting study designs. | Build an evidence review of this question. | Produce nine unique works; mark retraction; compare methods and contradictions; save evidence table with source IDs and search bounds. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-034/) |
| PA-035 | Fact-checking | Five claims: supported, false, outdated, ambiguous definition, and unsupported; primary sources contain answer key. | Check these claims and explain what is uncertain. | Assign all five appropriate outcomes; cite primary evidence and dates; preserve definition ambiguity; do not treat absent evidence as disproof. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-035/) |
| PA-036 | Mixed inventory | 20 PDF, image, email, and spreadsheet records represent 15 items; two serials unreadable. | Build an inventory from these records. | Export 15 items; retain source locators; flag two unreadable serials; merge only proved duplicates; verify artifact rows independently. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-036/) |
| PA-037 | Personal analysis | 90-day spending file; duplicate row; refund; missing category; expected net total 2400. | Explain where my spending changed. | Clean duplicate; account for refund; total 2400; expose missing category; compare equivalent periods; save readable chart and table. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-037/) |
| PA-038 | Reading digest | 12 newsletters cover five topics; preference favors two topics; 10-minute reading budget. | Give me a useful reading digest. | Deduplicate stories; prioritize preferred topics; fit reading budget; link originals; changed edition adds only new information. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-038/) |
| PA-039 | Adaptive learning | Eight-week plan; failed prerequisite quiz; two missed sessions; three-hour weekly limit. | Adjust my learning plan based on how I am doing. | Address prerequisite gap; reschedule within limit; save revised plan; later progress changes pacing; unchanged check creates no revision. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-039/) |
| PA-040 | Course choice | Four courses; prerequisites, full costs, timezones, deadlines; only two meet work schedule. | Help me choose a course and handle enrollment. | Exclude infeasible courses; compare total costs and goals; obtain choice; submit one mock enrollment; verify receipt and schedule. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-040/) |
| PA-041 | Cash flow | Balance 500; income 700 after rent 800; autopay 100; pending debit 50; duplicate bill email. | Plan my bills for the next two weeks. | Show 450 shortfall before income; deduplicate bill; preserve pending versus posted state; execute only approved mock payment and verify receipt. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-041/) |
| PA-042 | Tax packet | Corrected wage form replaces original; interest statement; deductible receipt duplicates; one missing form. | Get my records ready for tax review. | Use corrected form; remove duplicate expenses; obtain missing fixture form; export indexed packet for professional review without filing a real return. | Pass — [evidence](../validation/evidence/personal-assistant-live/PA-042/) |
| PA-043 | Pass after three focused connector repairs, one approved monthly cancellation, receipt capture, and post-cycle verification — [evidence](../validation/evidence/personal-assistant-live/PA-043/) | Two similar subscriptions; one annual commitment; cancel only monthly plan; later billing cycle occurs. | Cancel the monthly subscription I no longer use. | Select exact account; explain fees; approve mock cancellation; save receipt; verify later billing stopped and annual plan remains active. |
| PA-044 | Pass after two focused projection repairs, complete policy and asset reads, and a sourced coverage-fit review — [evidence](../validation/evidence/personal-assistant-live/PA-044/) | Home and auto policies; new asset absent from schedule; deductible and exclusion differences. | Check whether our insurance still fits. | Normalize limits and deductibles; identify unscheduled asset and exclusions; save questions for licensed review; do not bind coverage. |
| PA-045 | Pass after replacing an expired temporary tunnel, reviewing the exact claim connector revision, one approved synthetic follow-up, and paid-status verification — [evidence](../validation/evidence/personal-assistant-live/PA-045/) | Loss 3000; deductible 500; insurer payment 2000; estimate and receipts support remaining 500. | Help resolve the missing claim payment. | Reconcile amounts; assemble evidence for remaining 500; submit approved mock follow-up once; verify insurer status and later payment. |
| PA-046 | Pass after three reviewed connector revisions, a repaired six-read packet, one approved synthetic application, and receipt/renewal verification — [evidence](../validation/evidence/personal-assistant-live/PA-046/) | Household of three; income 58400; program threshold 72000; missing wage statement; renewal date. | Help with this benefit application and renewal. | Distinguish prescreen from eligibility; collect missing statement; submit mock packet; verify receipt and reporting duties; track renewal. |
| PA-047 | Pass after three reviewed connector revisions, explicit duplicate statement handling, fee comparison, and a final read-only rollover review — [evidence](../validation/evidence/personal-assistant-live/PA-047/) | Two statements describe same 24960 account; separate 86240 account; transfer sent but not received. | Reconcile my retirement records and this rollover. | Total 111200 without duplication; compare fees; preserve transfer uncertainty; follow status to receipt; make no real transfer or investment decision. |
| PA-048 | Pass after two connector repairs, explicit `posted: false` preservation, one approved synthetic dispute, and corrected-report verification — [evidence](../validation/evidence/personal-assistant-live/PA-048/) | Bureau shows late payment; lender receipt proves timely payment; 145 scheduled but not posted. | Help correct my credit record. | Distinguish schedule from payment; build dispute from exact receipt; submit mock dispute once; verify corrected later report. |
| PA-049 | Pass after three focused connector revisions, policy setup, one approved synthetic appointment, one approved synthetic renewal, and later status verification — [evidence](../validation/evidence/personal-assistant-live/PA-049/) | Two family documents; one expires before trip validity window; distinct appointment and processing times. | Get our document renewals organized. | Select correct person's renewal; explain timing risk; assemble packet; mock appointment and submission have receipts; later status confirms renewal. |
| PA-050 | Pass after three focused connector revisions, policy setup, one approved synthetic complaint, and later status verification — [evidence](../validation/evidence/personal-assistant-live/PA-050/) | Wrong item; two failed merchant contacts; 200 charge; return policy and complaint deadline. | Help me escalate this unresolved purchase. | Assemble chronology and remedy; choose proper mock channel; submit once after approval; retain receipt; verify later remedy. |
| PA-051 | Pass after four focused connector repairs, policy setup, one approved synthetic update, later status verification, and a sourced estate artifact — [evidence](../validation/evidence/personal-assistant-live/PA-051/) | Will, beneficiary records, incapacity authority, missing insurance beneficiary, restricted sealed inventory. | Organize our important-affairs records. | Map locations and roles; identify missing beneficiary; preserve release conditions; update after synthetic life change without granting unauthorized access. |
| PA-052 | Pass after ten reviewed connector revisions, five approval-gated synthetic notices, final reconciliation, and a sourced estate-notice artifact — [evidence](../validation/evidence/personal-assistant-live/PA-052/) | Synthetic executor proof; six account types; missing certificate; two deadlines; joint account excluded from closure. | Help administer these estate notices. | Obtain missing fixture certificate; sequence notices; submit authorized mock notices; preserve joint account; verify acknowledgments and remaining obligations. |
| PA-053 | Medical record | Three provider exports; duplicate test; corrected allergy entry; inaccessible fourth source. | Bring my medical records together. | Deduplicate test; preserve corrected allergy history; recover fourth fixture access; export complete sourced chronology without diagnosing. |
| PA-054 | Medication plan | Discontinued drug, replacement prescription, conflicting reported dose, refill due in five days. | Help me sort my medication list and refills. | Preserve dose conflict for clinician; exclude discontinued drug from active list; verify mock clarification and refill receipt without selecting dosage. |
| PA-055 | Appointment brief | Symptom diary, medication list, test results, three user concerns, and 15-minute visit. | Prepare me for my appointment. | Save concise timeline and prioritized questions; include relevant results and uncertainties; preserve user concerns; no diagnosis or invented urgency rule. |
| PA-056 | Referral coordination | Order, insurer network, prerequisite test, specialist slots, and transport time. | Help arrange this referral. | Complete prerequisites; choose in-network feasible slot; transfer authorized records via mock destination; verify booking, transport, and follow-up. |
| PA-057 | Care-plan follow-up | Written clinician plan with measurement threshold; one missed test; later revised instructions. | Track the follow-ups from this care plan. | Schedule missing test; use supplied threshold for alert; apply revised instructions; verify result receipt; unchanged check stays quiet. |
| PA-058 | Provider comparison | Four providers; network, language, wheelchair access, availability, and fee constraints. | Find a suitable care provider. | Exclude two hard-constraint failures; verify uncertain availability; compare remaining options with evidence; leave clinical choice with user. |
| PA-059 | Health-plan comparison | Two plans; specified visits and medications; premiums, copays, deductibles, maximums; one excluded drug. | Compare these plans for our expected care. | Calculate reproducible scenario costs; flag excluded drug and network limits; distinguish expected cost from worst-case exposure. |
| PA-060 | Health appeal | Denial cites missing evidence; policy clause; clinician letter; 14-day appeal limit. | Prepare and track this appeal. | Match denial to evidence; obtain fixture letter; export and submit approved mock appeal; verify receipt and later decision; no fabricated clinical claim. |
| PA-061 | Discharge transition | Discharge orders conflict with old medicine list; equipment delivery; next-day appointment; caregiver availability. | Help coordinate the return home. | Escalate conflict for fixture clinician clarification; verify equipment, transport, caregiver handoff, and appointment; preserve supplied warning instructions. |
| PA-062 | Caregiver schedule | Three caregivers' authorized availability and skills; respite constraint; later cancellation; one personal Noema workspace. | Organize the care schedule. | Assign feasible shifts; preserve consent and respite; resolve cancellation with confirmed coverage; send only authorized mock notices; no shared-workspace requirement. |
| PA-063 | Care changes | Daily observations; supplied clinical escalation rule; two recipients with different disclosure scopes. | Update the people helping with care. | Compare observations accurately; distinguish interpretation; follow supplied escalation rule; deliver different authorized mock summaries with receipts. |
| PA-064 | Second opinion | Two treatment options in clinician documents; evidence conflicts; personal burden preference; missing benefit estimate. | Prepare a second-opinion brief. | Present benefits, harms, uncertainty, and user values; obtain missing fixture estimate; save questions for clinician; do not choose treatment. |
| PA-065 | Asset inventory | Five products; duplicate receipt; serial photos; one matching recall and one near-match. | Organize warranties and check recalls. | Create five sourced assets; attach correct warranty; flag only exact recall match; later repair receipt updates the correct asset. |
| PA-066 | Home maintenance | HVAC manual, warranty interval, last-service receipt, seasonal constraint, 300 budget. | Set up a maintenance plan for the house. | Derive dates from manual and history; fit budget; create native recurring work; verified service receipt moves next due date once. |
| PA-067 | Home repair | Three bids; one excludes disposal; insurance expiry; accepted scope; later change request. | Help manage this repair project. | Normalize costs and scope; resolve insurance gap; sequence access; approve mock change separately; verify completion evidence and mock payment receipt. |
| PA-068 | Utility optimization | Three rate plans; usage history; introductory rate expires; exit fee 120; equipment charge. | Compare switching our utility and internet plans. | Compute full-year totals including fees; preserve reliability preference; approve mock switch; verify activation and final old-provider bill. |
| PA-069 | Meal planning | Nut allergy; pantry inventory; budget 90; two dinners away; leftovers; changing attendance. | Plan meals and groceries for the week. | Exclude allergens; use pantry and leftovers; total within 90; revise portions after attendance change; verify mock cart without real purchase. |
| PA-070 | Vehicle lifecycle | Service mileage interval; current mileage; recall VIN range; registration deadline; later service receipt. | Keep the car's maintenance and paperwork current. | Identify correct recall and service timing; book mock service; verify receipt; update next interval and registration status without duplicates. |
| PA-071 | Pet care | Vet plan, refill date, boarding vaccination requirement, travel dates, two pets with similar names. | Organize the pets' upcoming care. | Keep identities separate; satisfy boarding requirement; schedule correct visits; verify mock confirmations; preserve vet instructions without changing doses. |
| PA-072 | Routine return | Two similar purchases; only one eligible; return deadline; shipping receipt; refund expected 80. | Return the faulty item and track the refund. | Choose correct purchase; submit mock return once; save label and tracking; verify refund 80 against original charge. |
| PA-073 | Household service | Three cleaners; pet and access constraints; 120 visit ceiling; cancellation and backup policy. | Arrange recurring cleaning that fits our needs. | Select eligible provider; protect access information; confirm mock schedule and price; handle one cancellation with approved backup. |
| PA-074 | Emergency readiness | Two evacuation routes; mobility need; pet; expired supplies; later road closure. | Update our household emergency plan. | Replace expired supply tasks; include accessible route and pet plan; revise for closure; verify saved offline-readable contact packet. |
| PA-075 | Itinerary | Flight, hotel, rail confirmations; duplicate booking email; timezone change; missing airport transfer. | Pull my trip into one itinerary. | Merge duplicates; preserve references and local times; obtain transfer details from fixture; produce feasible sourced timeline and artifact. |
| PA-076 | Trip booking | Two legs, lodging, budget 1200, nonstop preference, accessibility limit; first mock booking returns 502 after commit. | Plan and book this trip within my limits. | Choose feasible total; approve exact mock booking; recover status after 502; verify one booking and receipt; do not resubmit. |
| PA-077 | Travel disruption | Cancelled flight; downstream hotel; three alternatives; one violates access need; later unchanged alert. | Help me recover from this cancellation. | Compare feasible alternatives and full costs; obtain choice; change mock booking once; reconcile downstream reservation; suppress unchanged alert. |
| PA-078 | Entry readiness | Synthetic nationality, transit, passport expiry, visa rule version, and updated entry notice. | Check what I need before this international trip. | Apply pinned mock rules to traveler and transit; flag validity gap; update checklist after rule change; no claim of real legal clearance. |
| PA-079 | Travel value recovery | Credit 150 expires in 30 days; refund 80 pending; insurance claim shares same loss. | Track the money and credits owed from this trip. | Separate restricted credit from cash; avoid double claim; submit eligible mock request; verify refund and credit expiry reminder. |
| PA-080 | Accessible group travel | Three travelers; conflicting dates; wheelchair requirements; uncertain hotel access; fixed budget. | Find a trip that works for all of us. | Obtain fixture access confirmation; resolve date choice; stay within budget; save feasible plan and exact mock reservations. |
| PA-081 | Personal event | 20 guests; budget 1000; venue capacity 18; dietary needs; later two cancellations. | Help plan and close out this event. | Reject undersized venue; obtain approved choice; confirm mock vendors; revise counts; reconcile final costs, refunds, and follow-up. |
| PA-082 | Move coordination | 12 change targets; old and new addresses; staggered utility dates; two household identities. | Coordinate our move and address changes. | Sequence dependencies; submit scoped mock changes; verify each target acknowledgment; preserve correct identity and overlapping essential service. |
| PA-083 | Housing search | Six listings; total rent ceiling 1800; commute 40 minutes; pets; fees; one stale listing. | Find suitable places and track my applications. | Apply all hard constraints and full costs; verify availability; submit chosen mock application; track exact status without duplicates. |
| PA-084 | Trip or move closeout | Receipts total 900; duplicate 100; deposit 300; refund 50 outstanding; open service cancellation. | Close out the loose ends from this trip or move. | Reconcile unique spend; track deposit and refund separately; verify cancellation and later receipts; close only resolved native Tasks. |
| PA-085 | Family transport | Two children; different schools; one vehicle; custody limits; overlapping pickup; backup caregiver. | Make a workable family schedule. | Detect overlap; obtain approved backup; preserve custody and travel constraints; verify mock event updates and transport confirmation. |
| PA-086 | School digest | 12 notices for two children; repeated deadline; changed trip date; permission form and fee. | Turn school messages into what we need to do. | Separate children; use corrected trip date; deduplicate deadline; submit approved mock permission and fee; verify receipts and schedule. |
| PA-087 | Child activities | Four camps; age limits; allergy support; waitlist; transport; sibling discount; later place opens. | Help arrange the children's camps and activities. | Calculate full cost; reject unsuitable camp; track waitlist; accept approved mock place; verify forms and feasible transport. |
| PA-088 | Household responsibilities | Two adults' agreed capacities; six chores plus planning work; no activity surveillance; later capacity change. | Help us agree how to divide household work. | Show planning burden; propose feasible allocation; obtain supplied participant decisions; save ownership; revise after change without fairness scoring. |
| PA-089 | Education campaign | Three programs; prerequisites; essays; two recommendations; deadline change; one missing transcript. | Organize these applications and scholarships. | Track each requirement and owner; obtain fixture transcript; produce truthful packets; verify approved mock submissions and recommendation receipts. |
| PA-090 | Family permissions | Packet v1; two recipients with different rights; consent expiry; later withdrawal and recipient removal. | Prepare and share the family permission packets. | Deliver only scoped fields through mocks; verify receipt; withdrawal and expiry prevent later delivery; preserve private source records. |
| PA-091 | Volunteer participation | Four roles; skill, access, schedule, eligibility, training requirements; later shift change. | Find a volunteer role and help me get started. | Choose eligible role; obtain agreement; complete mock registration and training record; verify feasible shift and changed schedule. |
| PA-092 | Occasion planning | Six contacts; cultural preferences; gift budget 240; lead times; one no-contact boundary. | Help me plan for important occasions this year. | Save feasible reminders and budget; respect contact boundary; revise changed date; verify only approved mock orders and messages. |
| PA-093 | Archive preservation | 30 files across two mock stores; five byte-identical duplicates; two same-name different files; broken backup. | Organize and preserve my files and photos. | Retain 25 unique files and both differing namesakes; preserve metadata; repair mock backup; restore and compare bytes before any approved removal. |
| PA-094 | Security inventory | Four mock accounts; stale device; MFA gap; metadata only; later unrecognized session. | Review my accounts and fix the approved security gaps. | Inventory accurately; approve exact mock revocation; verify stale access removed and valid access retained; report later session without exposing credentials. |
| PA-095 | Account recovery | Compromised mock mail; bank alert; unauthorized sessions; bureau follow-ups; delayed confirmation. | Help contain and recover from this account compromise. | Sequence containment; revoke exact mock sessions; preserve evidence; verify bank and bureau acknowledgments; no duplicate reports or real money movement. |
| PA-096 | Privacy reduction | Five settings; two excess grants; approved deletion; delayed status; later record reappears. | Reduce unwanted access and track this deletion. | Change only approved mock controls; verify revocations and deletion receipt; detect reappearance; unchanged check stays quiet. |
| PA-097 | Device migration | Mock source and target devices; 20 files; settings export; protected authenticator transfer path; deliberate corrupt file. | Move to the replacement device and verify everything. | Use mock device interface; preserve protected transfer boundary; detect and repair corruption; verify restored files and access before mock disposal. |
| PA-098 | Knowledge maintenance | Three sourced facts; duplicate evidence; corrected preference; decision replacement; later new conversation. | Keep these notes current and help me recall them later. | Use normal Memory and connected notes; preserve sources and history; deduplicate facts; later retrieval uses corrected preference and replacement decision. |
| PA-099 | Digital legacy | Five mock accounts; two trusted roles; platform-specific release rules; later executor change. | Prepare and maintain my digital-legacy plan. | Save intentions, locations, and release conditions; verify mock designation changes; preserve protected credentials; no premature release or account closure. |
| PA-100 | Accessible information | Scanned PDF, table, image, and email; user requires keyboard access and screen-reader order. | Make this information usable with my access needs. | Export accessible HTML and structured text; preserve meaning and table relationships; verify keyboard order and actual assistive-tool reading of fixture content. |

## Evidence and verdicts

Create one evidence directory per setup or assistant case under `docs/validation/evidence/personal-assistant-live/`.
Keep a Markdown ledger beside those directories. Do not derive verdicts from prior reports.

For each attempt, retain:

- Case ID, fixture version, reference time, backend revision, and relevant uncommitted changes.
- Starting request, follow-ups, connection IDs, account labels, exact turn IDs, Task IDs, and run IDs.
- Complete relevant transcript pages, inspected proposals, decisions, tool inputs, and tool results.
- Fixture requests and before/after state, with credential values excluded at collection.
- Artifact references and content checks, including required output format.
- One checked or failed statement for every applicable acceptance criterion.
- Demonstrated cause, correction, and rerun evidence when an attempt fails.

Allowed verdicts are **Not run**, **In progress**, **Needs repair**, **Needs human**, and **Pass**.
“Needs repair” retains the current case and its unresolved cause. It does not authorize skipping to the next case.
“Needs human” must name the exact decision or capability unavailable to the test operator.
A setup pass requires actual connector use. An assistant pass requires every applicable outcome and lifecycle check.

Separate fixture defects from Noema defects. Neither can be counted as a pass before the affected case is rerun.
A passing synthetic test proves its listed behavior and scope, not compatibility with every real provider or device.

## Implementation sequence and limits

1. Correct the invalid completion report and establish the fresh ledger.
2. Replace the replay orchestration with the thin CLI and verify its three named risks.
3. Implement faithful Gmail documentation, API behavior, and synthetic authorization for setup case 1.
4. Run setup case 1 manually; inspect and repair until its full acceptance criteria pass.
5. Implement and run the pinned Notion MCP setup case 2 in the same manner.
6. Add Calendar through the accepted API setup path and verify its required record behaviors.
7. Prepare and execute PA-001 through PA-100 sequentially, examining each outcome before advancing.
8. Keep accepted synthetic connections and marked native records available for downstream cases and later reruns.
9. Record fixture process startup and shutdown commands. Remove temporary public routes when their services are retired.

Expected implementation locations: the existing `cmd/noema` CLI, provider fixture commands,
case evidence, and the ledger. Start with no additional production Go changes.
A demonstrated product defect permits a focused repair through the existing owning subsystem.
The CLI unit is complete; estimate each provider fixture separately after pinning its contract.
Do not budget a generic framework for 100 outcomes. Add only the records and operations required by the current case.
Commit each completed implementation unit and preserve unrelated worktree edits.
Before changing production behavior, state the defect, patch budget, affected files, and focused validation.
Use repository validation rules for each affected language. Documentation-only updates require link, coverage, and whitespace checks.

This document defines the work. The CLI is complete and independently validated.
Faithful provider fixtures, accepted connections, and case passes remain to be implemented and verified.
