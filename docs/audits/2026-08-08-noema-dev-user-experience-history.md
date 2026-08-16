# `.noema-dev` user-experience incident audit

- **Status:** Dated evidence snapshot; closed at the stated cutoff
- **Mode:** Explore and report only
- **History window:** 2026-08-01 20:08 UTC through 2026-08-08 17:04 UTC
- **SQLite snapshot:** `/tmp/noema-history-audit-20260808.sqlite3`
- **Error-log snapshot:** `/tmp/noema-history-audit-20260808.errors.jsonl`
- **Scope:** Every distinct failure, misleading result, blocked workflow, repeated
  friction point, and latent state defect evidenced by `.noema-dev`

This file preserves historical evidence.
It does not describe current repository status.

This is an incident inventory, not a remediation plan or a second product
authority. Counts are occurrences in the point-in-time snapshot, not estimates.
One root cause may appear in several evidence sources; the issue register
deduplicates those manifestations where the evidence supports doing so.

The report intentionally omits credential stores and unnecessary personal
content. It did not inspect `adapters/cursor-secrets`, provider token files,
connection credential generations, MCP `secrets.json`, Web Push private keys,
subscription authentication material, authorization URLs, or protected
capability arguments. Personal email, calendar, and Notion contents are described
only at the level needed to establish the failure.

## Executive assessment

The history is not a collection of occasional third-party errors. It shows a
system that repeatedly declared capabilities ready before they worked, repaired
ordinary connector behavior live in the conversation, lost task and approval
continuations, performed at least one incorrect external mutation, and required
the human to diagnose or route around internal state failures.

The register below contains 78 distinct issues: 13 chat/reasoning issues, 21
Work/scheduling/governance issues, 17 Calendar issues, 11 Gmail issues, 8
Notion/MCP/browser issues, and 8 memory/state/observability issues.

The highest-impact findings are:

1. A calendar event other than the one requested was modified after incomplete
   search and unsafe candidate selection.
2. Calendar writes succeeded remotely while Noema reported an uncertain outcome,
   forcing read-after-write checks to avoid duplicates.
3. The Work scheduler executed malformed SQL thousands of times, so promised
   recurring behavior could not be considered reliable.
4. Work runs crossed task/review fences, repeatedly loaded foreign reviews, and
   exhausted automatic retries on deterministic faults.
5. Governed-action continuations lost their approval origin thousands of times;
   approvals were repeatedly superseded or became invalid before execution.
6. Gmail and Calendar connectors were repeatedly marked connected or ready before
   their main read operations could decode provider responses.
7. Gmail analyses operated on severely incomplete coverage because pagination,
   message lookup, MIME decoding, and OAuth refresh behavior each failed.
8. A large MCP tool catalog exhausted the conversation context after only 26
   turns, with no completed history left that could be compacted.
9. Transcript and Work records retain hundreds of permanently `running` items
   after their turns or runs are terminal, creating stale status truth.
10. Automatic memory updates failed on invalid provenance and citation structure,
    silently weakening continuity.

### Snapshot scale

| Source | Records | Relevant result |
| --- | ---: | --- |
| Conversation turns | 251 | 246 completed; 5 failed |
| Conversation items | 2,119 | 61 failed tool results; 5 failed error notices |
| Agent runs | 108 | 16 failed, 3 cancelled, 1 interrupted; 2 active and 1 awaiting approval at cutoff |
| Agent-run items | 1,897 | 40 failed tool results; hundreds retained as `running` |
| Runtime debug spans | 2,176 | Tool and provider failures across chat and Work |
| Tasks | 25 | 6 cancelled; 8 reviews requested changes |
| Work events | 1,159 | 16 `run.failed` events and 16 opened gates |
| Governed actions | 78 | 10 failed, 3 outcome-uncertain, 4 superseded, 1 cancelled, 1 declined |
| Error log | 28,919 | 18.7 MB in seven days |
| Adapter filesystem | 24 active definitions, 2 active connections | 45 quarantined definitions and 11 quarantined connections |

## Method and completeness

The audit covered:

- the live SQLite database including its WAL through the snapshot cutoff;
- the pre-ACP SQLite snapshot from 2026-08-03;
- all conversation turns and user/assistant/error/tool-result items;
- agent runs, agent-run items, runtime debug spans, task contracts, gates,
  messages, submissions, reviews, Work events, and notification state;
- governed actions, assessments, approvals, authentication requests, and events;
- provider, MCP, adapter, and Web Push status projections;
- `errors.log`, adapter quarantine history, task/conversation artifacts, and
  native-memory status;
- the complete human-message chronology, with personal details omitted here.

The pre-ACP comparison found no conversation item, turn, task, or run that is
missing from the current database. It did find 8 old adapter definitions, 2 old
adapter connections, and 1 old MCP server absent from the current disposable
projection. Their filesystem quarantine/provenance remains, but a live-database-
only audit would miss that integration history.

Severity means:

- **Critical:** known wrong external effect, material disclosure, or cross-task
  state corruption.
- **High:** blocks a core workflow, makes an answer materially unreliable, or
  invalidates promised automation.
- **Medium:** repeated avoidable friction, incomplete behavior, misleading state,
  or degraded recovery.
- **Low:** mostly latent status/observability degradation with plausible UX cost.

## Issue register

### Chat, reasoning, and user communication

#### CHAT-01 — Whole-turn provider outages

- **Severity:** High
- **Evidence:** Two turns failed with provider HTTP 503/connectivity errors, on
  August 2 and August 6.
- **User impact:** The human received a raw provider failure instead of an answer
  and had to retry.

#### CHAT-02 — Context exhaustion without a viable compaction path

- **Severity:** High
- **Evidence:** The 26th turn required approximately 123,251 input tokens against
  a 119,872-token model limit; the runtime said no completed history remained to
  compact.
- **User impact:** A simple Calendar question failed after a short conversation.
  The large MCP/tool-schema context, not human prose, dominated the window.

#### CHAT-03 — Slow foreground turns

- **Severity:** Medium
- **Evidence:** Completed turns averaged 24.1 seconds; 65 took at least 30 seconds,
  26 took at least 60 seconds, and the slowest took 373 seconds.
- **User impact:** Ordinary chat and connector actions often felt task-like rather
  than interactive.

#### CHAT-04 — Capabilities declared ready before they were usable

- **Severity:** High
- **Evidence:** Calendar, Gmail, and Notion were each announced as connected or
  ready immediately before core operations failed through invalid arguments,
  response-transform errors, cursor errors, or skipped authentication.
- **User impact:** The human repeatedly learned that “ready” meant only that setup
  state existed, not that the promised job worked.

#### CHAT-05 — The human became the failure router

- **Severity:** Medium
- **Evidence:** The chronology repeatedly contains “try again,” “why,” “wrong,”
  “fix,” “reconnect,” and suggestions from the human to start a new task or use a
  different calendar ID.
- **User impact:** Recovery and diagnosis were pushed back onto the user instead of
  being contained by the product.

#### CHAT-06 — Available web lookup was denied or avoided

- **Severity:** High
- **Evidence:** For a flight request, the assistant repeatedly asked for facts it
  could search, claimed a flight lookup was unavailable, and only searched after
  several objections.
- **User impact:** Repetitive clarification, loss of confidence, and delayed task
  completion.

#### CHAT-07 — Unverified all-day flight placeholders were created

- **Severity:** Critical
- **Evidence:** A flight was added as an all-day event before its schedule was
  researched; the assistant later acknowledged this was incorrect. A second
  flight request followed the same all-day-first pattern before a scheduled event
  was added.
- **User impact:** The calendar could contain misleading or duplicate travel
  commitments.

#### CHAT-08 — Flight status was overstated and internally inconsistent

- **Severity:** High
- **Evidence:** The assistant progressed from “no live status,” to “scheduled with
  no delay,” to “not available; final status cannot be verified” without a stable
  authoritative result.
- **User impact:** Time-sensitive travel information was presented with more
  certainty than the evidence supported.

#### CHAT-09 — Connector safety was described inaccurately

- **Severity:** High
- **Evidence:** The assistant said custom adapters contained zero arbitrary code
  and explicitly denied Luau, then reversed that statement after inspecting the
  manifest and finding constrained Luau transforms.
- **User impact:** A trust-critical explanation was false until challenged; an
  already-created safety artifact then needed correction.

#### CHAT-10 — A suspicious form received inferred private answers before a clear reply

- **Severity:** Critical
- **Evidence:** A delegated newsletter task asked for an email address and next
  calendar event, received no intervening human answer, and later reported that it
  had filled both fields. A later foreground attempt did receive an explicit
  “submit,” but that does not retroactively authorize the earlier inference.
- **User impact:** Private information was prepared for external disclosure
  without a clear answer to the focused questions.

#### CHAT-11 — Asynchronous task messages lost conversational referents

- **Severity:** Medium
- **Evidence:** After the human said to cancel a failed task and create a new one,
  a new executor asked what existing subscription should be cancelled, confusing
  task cancellation with an external newsletter cancellation.
- **User impact:** The user had to restate intent that was clear in the parent
  conversation.

#### CHAT-12 — Incomplete analyses could look more complete than they were

- **Severity:** High
- **Evidence:** Gmail answers summarized a small accessible subset while searches
  estimated roughly 201 messages; another run inspected 8 of 24 messages and
  failed to decode 16. Uncertainty was eventually disclosed, but headline answers
  still preceded or outweighed the coverage limitation.
- **User impact:** Empty or short result sets could be mistaken for a complete
  mailbox conclusion.

#### CHAT-13 — Recurring-news content initially violated the intended freshness job

- **Severity:** Medium
- **Evidence:** The user later had to change the daily positive-news task so every
  story was published that same day.
- **User impact:** A daily briefing could contain older stories while appearing to
  be a current-day briefing.

### Work, scheduling, review, and governance

#### WORK-01 — Scheduler deadline SQL was syntactically invalid

- **Severity:** High
- **Evidence:** `work_schedule_deadline_failed` occurred 7,002 times in about 41
  minutes. The recorded query omitted spaces between `tasks`/`WHERE`,
  `NULL`/`AND`, `ALL`/`SELECT`, and other clauses.
- **User impact:** The runtime could not reliably determine the next scheduled
  deadline.

#### WORK-02 — Due-schedule processing SQL was also invalid

- **Severity:** High
- **Evidence:** `work_schedule_processing_failed` occurred 5 times; the query
  omitted spaces around `task JOIN`, `stage WHERE`, and related clauses.
- **User impact:** Even when a schedule was due, the worker could not select it.

#### WORK-03 — Recurrence readiness was not demonstrated

- **Severity:** Medium
- **Evidence:** The same history later announced daily positive-news delivery as
  active, but its first review lacked the recurring configuration needed to prove
  the schedule. The earlier scheduler failures show why that evidence mattered;
  subsequent chat history does contain delivered briefings and therefore does not
  prove a continuing missed-delivery defect.
- **User impact:** “Set up and running” communicated more confidence than the
  accepted task evidence supported.

#### WORK-04 — Same-day story requirements conflicted with a 7:00 AM schedule

- **Severity:** Medium
- **Evidence:** A later gate asked what to do when fewer than three same-day
  stories exist by 7:00 AM.
- **User impact:** A predictable contract ambiguity was discovered only when the
  recurrence ran, rather than at configuration time.

#### WORK-05 — Reconciliation crossed task-generation fences

- **Severity:** Critical
- **Evidence:** `work_reconciliation_snapshot_failed` occurred 12 times with
  “resolved gate crosses the current task fence.”
- **User impact:** Durable task/gate state could not be reconciled safely after
  retries or reopen operations.

#### WORK-06 — Runs loaded foreign reviews

- **Severity:** Critical
- **Evidence:** Eight executor runs across Calendar and Gmail setup failed because
  each loaded a review belonging to a different run/task generation.
- **User impact:** Cross-run state contamination exhausted retries and forced task
  cancellation/recreation.

#### WORK-07 — Executor gate variants disagreed across protocol boundaries

- **Severity:** High
- **Evidence:** Four ACP executor attempts emitted
  `missing_required_executor` or `tool_unavailable`, but the daemon accepted only
  `clarification`, `approval`, or `recovery`.
- **User impact:** A correctly delegated filesystem task failed before doing work.

#### WORK-08 — The selected ACP executor tried to rediscover or redelegate itself

- **Severity:** High
- **Evidence:** The first ACP validation task failed with missing-executor/tool-
  unavailable gates; the successful replacement had to explicitly say the ACP
  session was already the executor and must work directly.
- **User impact:** The user had to understand and correct internal executor
  semantics.

#### WORK-09 — Relative/empty effective working directories failed repeatedly

- **Severity:** High
- **Evidence:** Four planner attempts for one Calendar capability task failed with
  `contract.effective_cwd: working directory must be absolute`.
- **User impact:** A routine connector revision exhausted infrastructure retries
  before planning began.

#### WORK-10 — Automatic retries repeated deterministic defects

- **Severity:** Medium
- **Evidence:** Invalid gate enums, invalid working directories, and foreign
  reviews each repeated through four attempts without changing the failing input
  or state.
- **User impact:** Retry policy added delay and noise without recovery.

#### WORK-11 — `task.report_blocked` could not satisfy its own terminal contract

- **Severity:** High
- **Evidence:** Four agent tool results failed with
  `invalid_terminal_contract: terminal payload does not match the required
  contract`.
- **User impact:** Executors could identify a blocker but could not reliably report
  it through the task API.

#### WORK-12 — Task retry calls lacked usable gate identity

- **Severity:** Medium
- **Evidence:** One retry passed an identifier that did not start with `gate:`;
  the next failed because the current open gate was required but unavailable.
- **User impact:** An explicit human “yes” could not resume the Calendar task.

#### WORK-13 — Task cancellation used stale revisions

- **Severity:** Medium
- **Evidence:** Two `task.cancel` calls failed with `work revision is stale`.
- **User impact:** The assistant reported successful cancellation/recreation only
  after routing around stale state.

#### WORK-14 — Recovery policy sometimes offered only cancellation

- **Severity:** Medium
- **Evidence:** A subscription task told the user to choose retry or cancel, then
  said its actual gate allowed only cancellation.
- **User impact:** The first prompt advertised an action the product could not
  perform.

#### WORK-15 — Executor leases expired mid-task

- **Severity:** High
- **Evidence:** A browser-based subscription run was interrupted by
  `lease_expired`; its governed action was cancelled because the origin run
  expired.
- **User impact:** Browser progress and approval context were lost.

#### WORK-16 — Governed-action approval origins disappeared

- **Severity:** High
- **Evidence:** `runtime_invariant_violation` occurred 3,292 times from August 3
  through August 8 with “governed action approval request is unavailable.”
- **User impact:** Pending continuations repeatedly could not find the approval
  request needed to proceed.

#### WORK-17 — Approvals and actions became stale before use

- **Severity:** High
- **Evidence:** Four approvals were superseded; chat showed two “Action is no
  longer valid” results. Three actions were superseded because a capability was
  removed, one because a browser session was unavailable, and one was cancelled
  after its origin run expired.
- **User impact:** The user approved work and still had to retry because the action
  identity or capability vanished.

#### WORK-18 — Outcome-uncertain state was not limited to chat

- **Severity:** High
- **Evidence:** Three governed actions ended `outcome_uncertain`; one capability-
  authentication completion also carried `outcome_uncertain`.
- **User impact:** Governance could not tell whether an external effect happened,
  defeating confident retry and audit behavior.

#### WORK-19 — Reviewer evidence contracts caused repeated change cycles

- **Severity:** Medium
- **Evidence:** Eight of 26 reviews requested changes. Most did not dispute the
  claimed implementation; they rejected submissions because canonical manifests,
  proposal receipts, recurring configuration, per-thread evidence, or tool-result
  evidence were not attached to the current submission.
- **User impact:** Work could be performed but not accepted, producing avoidable
  reruns and delayed completion.

#### WORK-20 — Replacement tasks could target stale proposal generations

- **Severity:** Medium
- **Evidence:** A Calendar revision was blocked because the requested reviewed
  digest already had a pending successor; the task contract and adapter tool
  disagreed about the legal replacement target.
- **User impact:** A valid correction became another clarification/cancellation
  cycle.

#### WORK-21 — ACP authentication failed without actionable detail

- **Severity:** Medium
- **Evidence:** One of five ACP authentication attempts failed with only the safe
  message “ACP authentication failed.”
- **User impact:** The failure gave no recovery guidance.

### Google Calendar connector

#### CAL-01 — Initial connector setup required repeated validator repair

- **Severity:** High
- **Evidence:** Several proposals were rejected before review; one failure cited an
  internal `response_size` field absent from the supplied manifest template.
- **User impact:** Calendar setup required repeated retries and developer-like
  instructions from the human.

#### CAL-02 — Standard Calendar jobs arrived as serial adapter revisions

- **Severity:** Medium
- **Evidence:** Read, write, RSVP, location edit, time edit, text search, attendees,
  timezone, and reminder behavior were added in separate review cycles.
- **User impact:** Ordinary calendar management repeatedly stopped to build and
  approve another connector version.

#### CAL-03 — Date-range listing rejected valid-looking calls

- **Severity:** High
- **Evidence:** Three `list_events` tool results failed with invalid arguments
  during next-event and event-selection requests.
- **User impact:** A connected calendar could not answer basic “what is next?”
  questions.

#### CAL-04 — Recurring occurrences and chronological order were not pinned

- **Severity:** High
- **Evidence:** The original definition omitted provider-required
  `singleEvents=true` and `orderBy=startTime` invariants.
- **User impact:** Results contained out-of-range recurring series and unsorted
  pages.

#### CAL-05 — Pagination stopped early and returned the wrong next event

- **Severity:** High
- **Evidence:** The assistant stopped after the first unsorted page and missed a
  current-day event, then admitted it had not exhausted or sorted results.
- **User impact:** The first next-event answer was wrong.

#### CAL-06 — Event search silently ignored the query

- **Severity:** High
- **Evidence:** The adapter mapped the model input as `query`, while Google
  expected `q`; searches returned the same unrelated old events.
- **User impact:** Search looked successful but was unfiltered until the human
  challenged it.

#### CAL-07 — The wrong event was modified

- **Severity:** Critical
- **Evidence:** With duplicate connections and incomplete search, the assistant
  selected a similarly named event and changed its location. The human then said
  it was the wrong one.
- **User impact:** Confirmed incorrect external mutation.

#### CAL-08 — Duplicate connections and the wrong Calendar resource caused 404s

- **Severity:** High
- **Evidence:** One update and three follow-up searches returned not-found/resource-
  not-found until the human suggested the wrong calendar ID was being used.
- **User impact:** Multiple failed retries and an incorrect reconnection diagnosis.

#### CAL-09 — Successful creates were reported as uncertain

- **Severity:** High
- **Evidence:** Three `create_event` calls ended `outcome_uncertain`; read-only
  verification showed the events had in fact been created.
- **User impact:** The user had to check the external calendar before retrying to
  avoid duplicates.

#### CAL-10 — Calendar authentication was skipped mid-use

- **Severity:** High
- **Evidence:** One Calendar read was cancelled as `authentication_skipped` after
  earlier successful use.
- **User impact:** A connected calendar became intermittently unreadable.

#### CAL-11 — Write response transforms were incomplete

- **Severity:** High
- **Evidence:** A later revision had to add bounded result transforms for create,
  update, RSVP, delete, and identity operations.
- **User impact:** Noema could perform writes but could not reliably interpret or
  confirm their provider responses.

#### CAL-12 — Empty provider arrays were serialized incorrectly

- **Severity:** High
- **Evidence:** A demonstrated correction changed recurrence, attendees, calendar
  lists, event lists, and search results to initialize typed `json.array()`
  values. Before that correction, `list_events` produced five
  `response_transform_failed` results in one validation case.
- **User impact:** Legitimate empty collections could break an entire Calendar
  read.

#### CAL-13 — `get_event` omitted timezone evidence

- **Severity:** Medium
- **Evidence:** A live-validation repair added required start and end timezone
  fields while preserving the rest of the operation.
- **User impact:** Cross-timezone scheduling could not be verified from connector
  output.

#### CAL-14 — `get_event` omitted reminder evidence

- **Severity:** Medium
- **Evidence:** A later repair added `remindersUseDefault` and bounded reminder
  overrides.
- **User impact:** The assistant could create milestone events but could not verify
  whether reminder preferences were honored.

#### CAL-15 — Attendee replacement incorrectly required exactly four addresses

- **Severity:** Medium
- **Evidence:** The operation description and schema were revised so only the
  first address was required and addresses two through four were optional.
- **User impact:** Common one-to-three-attendee edits could not satisfy the
  connector contract.

#### CAL-16 — Canonical definitions were intermittently unavailable to repair tasks

- **Severity:** Medium
- **Evidence:** A correction task twice reported that it could not retrieve the
  exact current definition, despite later succeeding and attaching a replacement.
- **User impact:** Repair work produced unnecessary human gates and contradictory
  progress messages.

#### CAL-17 — Calendar capability disappeared from the chat tool catalog

- **Severity:** High
- **Evidence:** After earlier successful Calendar use, the assistant said the
  reading tool was no longer available and started a full reconnect flow.
- **User impact:** Existing access was treated as absent, creating a long rebuild
  and reauthorization cycle.

### Gmail connector

#### GMAIL-01 — Setup reopened the wrong completed task

- **Severity:** Medium
- **Evidence:** A request for read/search/draft access tried to reopen an older
  completed Gmail task. The assistant later acknowledged a fresh task was the
  correct operation.
- **User impact:** The user had to diagnose task lifecycle behavior.

#### GMAIL-02 — Fresh setup still assumed an existing connector

- **Severity:** Medium
- **Evidence:** The replacement task stopped because it required an existing Gmail
  definition even though the user was asking for a new connector.
- **User impact:** An extra failed task and another round of explanation preceded
  the actual standalone proposal.

#### GMAIL-03 — Initial message reads were incomplete

- **Severity:** High
- **Evidence:** The first read-only connector had a failed `get_message` call and
  returned limited sender/body detail during newsletter analysis.
- **User impact:** The result looked useful but could not fully support its own
  unsubscribe recommendations.

#### GMAIL-04 — Optional MIME payload serialization crashed `get_message`

- **Severity:** High
- **Evidence:** The first read/search/draft connector failed four message reads;
  the repair removed serialization of the optional MIME payload.
- **User impact:** Gmail was marked ready but could not open a random message.

#### GMAIL-05 — Search response transformation and pagination failed

- **Severity:** High
- **Evidence:** `search_messages` produced ten conversation-level
  `response_transform_failed` results across validation cases.
- **User impact:** Searches returned only small partial pages and could not exhaust
  a 201-message estimate.

#### GMAIL-06 — Search results produced message IDs that could not be fetched

- **Severity:** High
- **Evidence:** Ten `get_message` calls returned resource-not-found during thread
  validation.
- **User impact:** The assistant could identify candidates but not inspect the
  messages needed to determine whether questions were answered.

#### GMAIL-07 — Base64url parsing rejected most sampled messages

- **Severity:** High
- **Evidence:** Eighteen message reads failed with response-transform errors in the
  later Gmail connector; one run explicitly reported 16 failures out of 24
  messages.
- **User impact:** Thread reconstruction and body analysis were low-confidence.

#### GMAIL-08 — A connector revision removed or renamed a needed operation

- **Severity:** High
- **Evidence:** Four agent calls to `search_messages` failed with
  `unknown_operation` after message-reading repair work.
- **User impact:** Fixing decode behavior regressed the search surface used by the
  same task.

#### GMAIL-09 — OAuth did not initially request durable offline access

- **Severity:** High
- **Evidence:** A later correction added `access_type=offline`,
  `include_granted_scopes=true`, and `prompt=consent` so Google would issue a
  refresh token. Five Gmail capability requests had already ended
  `authentication_skipped`.
- **User impact:** Gmail access worked briefly, then could not refresh and had to be
  reviewed/authorized again.

#### GMAIL-10 — Reliability was recovered by reducing result depth

- **Severity:** Medium
- **Evidence:** Both Gmail reader repairs ultimately avoided deep MIME decoding and
  returned bounded snippets/body text instead of full nested content.
- **User impact:** Reads became more reliable, but full-body and thread analysis
  remained less capable than the requested workflows implied.

#### GMAIL-11 — The current validation result still lacked per-thread evidence

- **Severity:** Medium
- **Evidence:** The final review at the snapshot cutoff requested changes because
  22 single-message threads were excluded only by aggregate category, without
  thread-specific chronology, sender/recipient context, or rationale.
- **User impact:** An empty unanswered-question queue was not auditable enough to
  trust as complete.

### Notion, MCP, browser, and external interaction

#### INT-01 — MCP discovery failed authentication

- **Severity:** High
- **Evidence:** Two `mcp_tool_call_failure` log events said the server rejected its
  credentials, once during initial setup and once during reconnection.
- **User impact:** Hosted connection discovery could not proceed and forced custom
  public-API fallbacks.

#### INT-02 — Notion tool schemas fell back to best-effort enforcement

- **Severity:** High
- **Evidence:** 18,585 `provider_schema_fallback` errors covered 19 tools on the
  older server projection and 22 tools on the newer one because schemas mapped
  through root `additionalProperties`.
- **User impact:** Tool arguments had weaker provider-side schema enforcement and
  contributed large repeated catalog/context overhead.

#### INT-03 — Notion pagination cursor handling was broken

- **Severity:** High
- **Evidence:** Recent/private/shared page-list calls rejected `cursor: null` at
  input validation or sent an invalid cursor format. Six failed spans/results are
  present across those listing operations.
- **User impact:** Listing pages required retries or fallback search and made a
  simple random-page request unreliable.

#### INT-04 — Notion fetch rejected ordinary webpage URLs

- **Severity:** Medium
- **Evidence:** Two delegated tasks passed webpage URLs to Notion fetch and received
  `URL type webpage not currently supported`.
- **User impact:** Research/inspection detoured through failing tools before using
  another source.

#### INT-05 — Browser sessions were both “already active” and unavailable

- **Severity:** High
- **Evidence:** Nine agent `web.browse.open` calls failed because a session was
  already active, while two chat snapshots said no active session existed and two
  open calls failed outright.
- **User impact:** Browser ownership was inconsistent across chat, task, and
  approval continuations.

#### INT-06 — Browser work timed out and lost continuity

- **Severity:** High
- **Evidence:** One browser wait timed out, one Work lease expired, and later
  snapshots/opens could not recover the prior session.
- **User impact:** A form had to be refilled and retried repeatedly.

#### INT-07 — The browser could not execute a JavaScript-required form

- **Severity:** High
- **Evidence:** Google Forms repeatedly reported JavaScript disabled; the Submit
  control remained disabled and no confirmation could be verified.
- **User impact:** The requested signup was never confirmed despite several task
  and foreground attempts.

#### INT-08 — Browser submission capability disappeared after approval

- **Severity:** High
- **Evidence:** Governed actions were superseded as `capability_removed` and
  `browser_session_unavailable`; chat reported that the submission capability was
  removed before the final click.
- **User impact:** Human approval did not preserve the action environment long
  enough to execute.

### Memory, state truth, and observability

#### STATE-01 — Automatic memory updates cited invalid provenance

- **Severity:** High
- **Evidence:** Three of five `native_memory_update_failed` events cited a source
  that was neither existing provenance nor an eligible human message in the
  chunk.
- **User impact:** Durable memory silently failed to capture or consolidate
  conversation information.

#### STATE-02 — Automatic memory generated invalid footnotes

- **Severity:** Medium
- **Evidence:** Two memory updates failed on duplicate/invalid footnote names or a
  mismatch between citations and definitions.
- **User impact:** Memory continuity depended on citation formatting generated by
  the model.

#### STATE-03 — Conversation tool calls never reached a terminal item status

- **Severity:** High
- **Evidence:** 397 conversation `tool_call` items remained `running` even though a
  later tool result exists; 393 belong to completed turns and 4 to failed turns.
- **User impact:** Any UI or reconstruction that trusts item status can show stale
  spinners or imply ongoing work after completion.

#### STATE-04 — Terminal Work runs retained `running` child items

- **Severity:** High
- **Evidence:** 587 agent-run items remained `running` under completed, failed, or
  interrupted runs: 567 assistant-output chunks, 18 tool calls, and 2 tool
  results.
- **User impact:** Work drill-ins and progress reconstruction can disagree with the
  terminal run state.

#### STATE-05 — Old runtime spans remained open

- **Severity:** Low
- **Evidence:** Excluding work active at the cutoff, a provider continuation from
  August 2 and a browser-open span from August 7 still had `running` status with
  no end time.
- **User impact:** Debug timing and “currently active” diagnostics are not fully
  trustworthy.

#### STATE-06 — Error logging was dominated by repeated non-actionable noise

- **Severity:** Medium
- **Evidence:** `errors.log` reached 18.7 MB and 28,919 records in seven days.
  Schema fallback alone contributed 18,585 records; malformed schedule polling
  added 7,007; missing approval origins added 3,292.
- **User impact:** Genuine failures are difficult to find, disk/log volume grows
  rapidly, and repeated background error paths may consume runtime resources.

#### STATE-07 — Integration history is not reconstructable from the live projection alone

- **Severity:** Low
- **Evidence:** The pre-ACP snapshot contains 8 adapter definitions, 2 connections,
  and 1 MCP server absent from the current disposable SQLite projections. The
  filesystem contains 45 quarantined definitions and 11 quarantined connections.
- **User impact:** Inspecting why an integration changed requires correlating a
  database backup with filesystem quarantine/provenance rather than using one
  current history surface.

#### STATE-08 — Connector revision churn was extreme

- **Severity:** Medium
- **Evidence:** Against only 24 active definitions and 2 active connections, the
  history accumulated 45 quarantined definitions and 11 quarantined connections
  in roughly one week.
- **User impact:** Repeated immutable replacement/review/reconnect cycles became a
  primary interaction pattern instead of an exceptional maintenance path.

## Error-log reconciliation

All 28,919 JSONL records in the snapshot fall into these categories:

| Category | Count | First occurrence | Last occurrence | Root issue |
| --- | ---: | --- | --- | --- |
| `provider_schema_fallback` | 18,585 | Aug 1 20:58 | Aug 8 17:04 | INT-02, STATE-06 |
| `work_schedule_deadline_failed` | 7,002 | Aug 3 07:11 | Aug 3 07:52 | WORK-01 |
| `runtime_invariant_violation` | 3,292 | Aug 3 01:06 | Aug 8 16:49 | WORK-16 |
| `work_runtime_worker_error` | 16 | Aug 3 17:07 | Aug 8 06:26 | WORK-06 through WORK-09 |
| `work_reconciliation_snapshot_failed` | 12 | Aug 4 06:18 | Aug 8 16:54 | WORK-05 |
| `native_memory_update_failed` | 5 | Aug 1 20:11 | Aug 8 07:32 | STATE-01, STATE-02 |
| `work_schedule_processing_failed` | 5 | Aug 3 07:11 | Aug 3 07:51 | WORK-02 |
| `mcp_tool_call_failure` | 2 | Aug 1 20:09 | Aug 8 06:04 | INT-01 |

The counts sum exactly to the 28,919-record snapshot.

## Structured failure reconciliation

The following are manifestations already assigned to issue IDs above, not extra
root-cause findings:

- 5 failed conversation turns: 2 provider 503s, 1 context overflow, and 2
  uncertain Calendar writes.
- 61 failed conversation tool results, dominated by Gmail transform/not-found
  failures, Calendar transform/not-found/uncertain failures, browser-session
  failures, Notion cursor failures, and stale Work commands.
- 40 failed agent-run tool results, dominated by Gmail decoding/search failures,
  active-browser conflicts, invalid terminal reports, and unsupported Notion
  fetch inputs.
- 16 failed agent runs: 4 invalid ACP gate variants, 4 invalid working-directory
  contracts, and 8 foreign-review invariant failures.
- 8 review-change verdicts: 7 historical and 1 at the snapshot cutoff.
- 3 Calendar creates with uncertain outcome, all of which required
  external read-back before a safe retry.
- 6 cancelled capability-auth requests: 1 Calendar and 5 Gmail, all marked
  `authentication_skipped`.

## What did not fail in this snapshot

For balance, several delivery/control surfaces did not show an error state:

- all 56 Work notification-outbox records were delivered once;
- Web Push recorded 491 delivered and 74 suppressed deliveries, with no failed
  delivery state or last-error code;
- all current provider-account projections were authenticated;
- the current MCP server projection was authenticated and healthy at cutoff;
- all 28 current MCP tool policies were `ready`;
- all 24 current adapter definitions compiled, and both current adapter
  connections were active.

Those healthy projections do not negate the historical incidents: several
connectors were “healthy” or “ready” while their operation transforms, OAuth
refresh behavior, pagination, or task continuation still failed.

## Audit limitations

- The snapshot ended with two executor runs active and one waiting for approval.
  Their later outcomes are outside this report.
- External provider state was not re-read for this audit. The report records what
  Noema persisted, including its uncertainty and contradictions.
- The audit identifies every distinct issue evidenced in the available history;
  it cannot identify silent problems that produced no conversation, database,
  filesystem, or log evidence.
- Quarantined adapter manifests were counted and correlated through provenance,
  but credential generations were deliberately not opened.
- Severity describes user-experience impact in this one development history, not
  exploitability or production prevalence.
