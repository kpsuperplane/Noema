# Current-build disposition of `.noema-dev` UX incidents

- **Status:** Dated evidence snapshot; closed at the stated cutoff
- **Mode:** Adversarial review and report only
- **Historical baseline:** [`.noema-dev` user-experience incident audit](2026-08-08-noema-dev-user-experience-history.md)
- **Source revision:** `be388c1e223800bb9925e49302eb2a1acb3c7d9f`
- **Runtime cutoff:** 2026-08-08 19:24:29 UTC
- **SQLite snapshot:** `/tmp/noema-current-build-audit-20260808-v2.sqlite3`
- **Error-log snapshot:** `/tmp/noema-current-build-audit-20260808-v2.errors.jsonl`

“Current build” means only the stated source revision and runtime cutoff.
This file does not describe current repository status.

This report rechecks all 78 historical issue records against the current source,
the fresh `.noema-dev` state, post-audit runtime activity, focused unit tests, and
the [50-case live validation ledger](../validation/personal-agent-50-case-ledger.md).
It is a filter, not a remediation plan.

The source revision includes an iOS-only merge after the runtime's latest backend
change. The server-side source is unchanged from `71270f3a`; the iOS transcript
and task projections added by the merge were inspected statically. No browser or
device UI session was run. Credential material, protected arguments, and
unnecessary private content were not inspected.

## Verdict

The current build does **not** still exhibit most of the original register.
Fourteen issue records are confirmed current problems and eight more are
partially mitigated but retain a current defect or limitation. Forty are resolved
with current code and live evidence. The remaining sixteen are either not
reproduced, scenario-specific and unverified, or an intentional safety/authority
boundary rather than a current defect.

| Disposition | Count | Meaning |
| --- | ---: | --- |
| **Confirmed current** | 14 | Fresh runtime evidence or an exercised current limitation proves the problem remains. |
| **Partial / retained** | 8 | The original symptom is mitigated, but a material residue or unchanged failure path remains. |
| **Resolved** | 40 | Current source plus focused or live validation disproves the historical failure. |
| **Not reproduced** | 4 | The current workload exercised the area without reproducing the issue, but there is no narrow fix to claim. |
| **Unverified scenario** | 8 | Rechecking would require the same external page, flight, browser session, or conversational situation. |
| **Not a current defect** | 4 | The record describes an intentional safety/authority boundary or diagnostic-history limitation. |

The 22 surviving issue IDs collapse to a smaller set of root problems:

1. Work reconciliation and governed-action continuation still operate on stale
   or missing origins and repeat the same failures.
2. Work leases and review-evidence cycles still create avoidable reruns.
3. Connector authority remains revision-heavy, and Calendar still lacks an
   adopted true all-day create operation.
4. Notion schemas still fall back to best-effort provider enforcement and flood
   the error log.
5. Automatic memory still emits invalid provenance and footnote manifests.
6. Canonical transcript/run-item statuses and runtime spans remain open after
   their logical work is terminal, although current clients mask much of it.
7. Foreground latency and provider reliability still have a visible long tail.

## Confirmed current issues

### Work correctness and recovery

| ID | Current evidence | Current impact |
| --- | --- | --- |
| **WORK-04** | The active 07:00 recurrence still requires exactly three stories published that same calendar day. Its August 8 run returned zero rather than violate freshness. | The configured quantity and schedule remain mutually unreliable; a correct run may deliver nothing. |
| **WORK-05** | Five post-baseline reconciliation snapshots failed because a resolved gate crossed the current task fence. The invariant remains in [`work_reconciliation_snapshot.rs`](../../crates/noema-store/src/work_reconciliation_snapshot.rs). | Reconciliation cannot reliably recover affected Work state. |
| **WORK-10** | The runtime repeated 148 identical missing-origin errors and five identical fence failures in short bursts. | Deterministic defects still consume retries/log volume instead of being quarantined once. |
| **WORK-15** | One new executor run was interrupted with `lease_expired`. | A valid long-running task can still be aborted mid-work and retried. |
| **WORK-16** | `governed action continuation origin is unavailable` occurred 148 times after the historical cutoff. The current path logs and returns without publishing the outcome when it cannot find the request in the visible transcript ([`action_resolution.rs`](../../crates/noema-runtime/src/daemon/runtime/action_resolution.rs)). | Approved work can finish without reconnecting the result to the user-visible origin. |
| **WORK-19** | Eleven of 44 new reviews requested changes. Six of 35 reviewed tasks required multiple review cycles; three reached three reviews. Feedback repeatedly requested evidence already asserted but not attached or reproduced. | Correct work is delayed by evidence packaging and reviewer visibility gaps. |

### Connector and integration behavior

| ID | Current evidence | Current impact |
| --- | --- | --- |
| **CAL-02** | The live store contains 14 compiled Calendar definitions. Case 12 is still waiting for adoption of another reviewed definition solely to add true all-day creation. | An ordinary Calendar requirement still becomes a connector-revision and approval workflow. |
| **INT-02** | 12,342 Notion schema-fallback errors were recorded after the historical cutoff. Another 242 occurred after the compile-once change, including at 19:23 UTC. All 22 Notion tools still lower schemas containing root `additionalProperties` to best-effort enforcement. | Model arguments remain less strictly constrained, and every task-runtime compilation produces non-actionable error noise. |

### Memory, state, and observability

| ID | Current evidence | Current impact |
| --- | --- | --- |
| **STATE-01** | A new memory update cited an item that was neither existing provenance nor a human message in the admitted chunk. | Automatic memory silently loses otherwise useful updates. |
| **STATE-02** | Two new memory updates failed because the source manifest did not exactly match footnote targets. | Continuity degrades even when the underlying conversation completes successfully. |
| **STATE-05** | Two spans are still `running` more than an hour after start: an August 2 provider continuation and an August 7 `web.browse.open`. | Runtime diagnostics continue to present permanently active work. |
| **STATE-06** | The post-baseline error log contains 12,503 records: 12,342 schema fallbacks, 148 repeated origin failures, five Work worker failures, five reconciliation failures, and three memory failures. | Actionable failures are buried under mechanically repeated diagnostics. |
| **STATE-08** | There are 26 stored compiled connector definitions for two active connections, plus 45 quarantined definitions and 11 quarantined connection directories. | Ordinary capability evolution remains operationally expensive and hard to reason about. |

### Responsiveness

| ID | Current evidence | Current impact |
| --- | --- | --- |
| **CHAT-03** | All 69 post-baseline foreground turns completed, but they averaged 18.7 seconds; 16 took at least 30 seconds, four took at least 60 seconds, and the maximum was 77 seconds. | Reliability improved, but ordinary chat still has a conspicuous long tail. |

## Partially mitigated issues that still matter

| ID | What improved | What remains |
| --- | --- | --- |
| **CHAT-01** | All 69 new foreground turns completed. | Four new executor runs still failed on provider HTTP 503 or transport errors, so the outage experience moved out of foreground chat rather than disappearing system-wide. |
| **CHAT-02** | No context overflow recurred in the 69-turn window, and connector tools are now compiled once per foreground runtime. | The current admission path still returns the same hard overflow when active context alone exceeds the model budget and there is no completed history to compact ([`context_window.rs`](../../crates/noema-runtime/src/daemon/runtime/context_window.rs)). |
| **WORK-06** | No foreign-review failure recurred after the historical cutoff. | The current loader still falls back from a run's triggering review to the task's latest review before applying the foreign-review invariant ([`work_run_context.rs`](../../crates/noema-store/src/work_run_context.rs)); the historical failure path was not removed. |
| **GMAIL-10** | Gmail is now stable across the live cases, and every returned message in Cases 2 and 3 was assessed. | Stability still depends on an 18-message bounded page and snippet/header projection rather than restored full MIME depth. Analyses can be complete only within that reduced projection. |
| **INT-06** | No fresh browser task was executed. | The old `web.browse.open` span remains open, and a new non-browser executor lease expired. Session continuity under long browser work therefore remains unproven and shares a still-active lease failure. |
| **INT-08** | Nine new governed actions succeeded, and no new browser submission was attempted. | The shared approval-origin defect is currently reproducing; whether the browser-specific continuation now survives approval was not re-exercised. |
| **STATE-03** | Web and current iOS clients pair a result with its call and render the group terminal even when the call row says `running`. | All 429 conversation tool calls with a result still retain `running`; 32 were created after the historical cutoff. The canonical state remains false and other consumers can expose it. |
| **STATE-04** | Current task UIs group calls with results and generally avoid a visible spinner. | Terminal runs retain 567 `running` assistant outputs, 20 `running` tool calls, and two `running` tool results. One new terminal run also retained a running call. |

## What is resolved

The strongest evidence at the cutoff is the live validation ledger: 49 of 50 cases are
`PASS`; Case 12 is `RUNNING` only because of the separately retained all-day
Calendar gap. The suite exercised Gmail, Calendar, Notion, cross-system reads,
confirmed writes, exact update targeting, pagination, readback, OAuth recovery,
recurrence, and reviewer approval.

The following historical records are resolved in the current active build:

- **Chat:** CHAT-04, CHAT-09, CHAT-12, CHAT-13.
- **Work:** WORK-01, WORK-02, WORK-03, WORK-07, WORK-09, WORK-11,
  WORK-12, WORK-17, WORK-20.
- **Calendar:** CAL-01 and CAL-03 through CAL-17.
- **Gmail:** GMAIL-02 through GMAIL-09, and GMAIL-11.
- **Integrations:** INT-01 and INT-03.

In particular:

- the August 8 recurrence materialized on schedule and completed, and the focused
  due-processing unit test passes;
- strict gate schemas now agree on `clarification` and `approval`, task retry
  requires a gate ID, and ACP execution explicitly tells the selected executor
  to perform the contract directly;
- Calendar listing, recurrence expansion, chronological ordering, pagination,
  search, exact event targeting, writes, timezone/reminder evidence, attendees,
  OAuth, and catalog visibility all have current live proof;
- Gmail pagination, message lookup, bounded base64url decoding, OAuth refresh,
  typed arrays, and per-thread evidence all have current live proof;
- Notion optional `cursor: null` omission has both a passing focused unit test
  and live private/shared-page proof.

## Full 78-issue disposition matrix

### Chat, reasoning, and user communication

| ID | Disposition | Short basis |
| --- | --- | --- |
| CHAT-01 | Partial / retained | Foreground recovered; four new Work provider failures remain. |
| CHAT-02 | Partial / retained | Not reproduced; unchanged active-context hard overflow remains. |
| CHAT-03 | **Confirmed current** | 16/69 turns took at least 30 seconds. |
| CHAT-04 | Resolved | Current active connectors completed 49 live cases and one safe gate. |
| CHAT-05 | Not reproduced | Current validation did not require the human to route internal failures. |
| CHAT-06 | Unverified scenario | The historical flight conversation was not replayed. |
| CHAT-07 | Unverified scenario | No equivalent flight mutation was authorized for audit. |
| CHAT-08 | Unverified scenario | No live flight-status scenario was replayed. |
| CHAT-09 | Resolved | Current sandbox and connector contracts match the safety description. |
| CHAT-10 | Unverified scenario | The suspicious external form was not resubmitted. |
| CHAT-11 | Unverified scenario | Product-level asynchronous referent retention was not replayed. |
| CHAT-12 | Resolved | Current Gmail/cross-system reviews prove bounded coverage and limitations. |
| CHAT-13 | Resolved | The latest recurrence refused older stories and returned zero transparently. |

### Work, scheduling, governance, and ACP

| ID | Disposition | Short basis |
| --- | --- | --- |
| WORK-01 | Resolved | Current schedule processing SQL and focused unit test pass. |
| WORK-02 | Resolved | The live August 8 due occurrence materialized and completed. |
| WORK-03 | Resolved | Four daily occurrences exist; the latest is terminal success. |
| WORK-04 | **Confirmed current** | Exactly three same-day stories at 07:00 remains infeasible. |
| WORK-05 | **Confirmed current** | Five new reconciliation fence failures. |
| WORK-06 | Partial / retained | No recurrence, but the current review fallback remains. |
| WORK-07 | Resolved | Current gate schema exposes only the two canonical variants. |
| WORK-08 | Not reproduced | Current ACP prompt forbids rediscovery/redelegation; no new failure. |
| WORK-09 | Resolved | Current schemas reject empty overrides; no new cwd failure. |
| WORK-10 | **Confirmed current** | Identical deterministic failures still repeat in bursts. |
| WORK-11 | Resolved | Current terminal tool schema is satisfiable; no new contract failure. |
| WORK-12 | Resolved | `task.retry` now requires exact gate identity. |
| WORK-13 | Not a current defect | Revision rejection is the intended optimistic-concurrency fence. |
| WORK-14 | Unverified scenario | No equivalent exhausted-recovery gate was created. |
| WORK-15 | **Confirmed current** | One new executor lease expired. |
| WORK-16 | **Confirmed current** | 148 new missing approval-origin errors. |
| WORK-17 | Resolved | Nine new governed actions succeeded; none newly failed or went uncertain. |
| WORK-18 | Not a current defect | `outcome_uncertain` remains an intentional safety state; no new occurrence. |
| WORK-19 | **Confirmed current** | Six tasks needed repeated review cycles, up to three reviews. |
| WORK-20 | Resolved | Complete transitive adapter replacement lineage is now followed. |
| WORK-21 | Not reproduced | Four later ACP authentication attempts completed; no new failure. |

### Calendar

| ID | Disposition | Short basis |
| --- | --- | --- |
| CAL-01 | Resolved | Current reviewed connection supports the validated operation set. |
| CAL-02 | **Confirmed current** | Fourteen definitions plus a pending all-day successor. |
| CAL-03 | Resolved | Current date-range listing is live-validated. |
| CAL-04 | Resolved | Expanded, chronologically bounded occurrence reads are validated. |
| CAL-05 | Resolved | Multi-page traversal and next-event selection are validated. |
| CAL-06 | Resolved | Query mapping is present and exercised. |
| CAL-07 | Resolved | Multiple current updates targeted only the verified event and were read back. |
| CAL-08 | Resolved | Exactly two intended active connections remain; Calendar resolves correctly. |
| CAL-09 | Resolved | Creates return bounded provider receipts instead of uncertainty. |
| CAL-10 | Resolved | Current Calendar use did not skip authentication. |
| CAL-11 | Resolved | Write response transforms are required and exercised. |
| CAL-12 | Resolved | Typed empty arrays are active and live-validated. |
| CAL-13 | Resolved | `get_event` returns bounded IANA timezone evidence. |
| CAL-14 | Resolved | `get_event` returns default and override reminder evidence. |
| CAL-15 | Resolved | Only one attendee is required; three are optional. |
| CAL-16 | Resolved | Exact patching and transitive replacement lineage preserve repair authority. |
| CAL-17 | Resolved | Calendar remained in the tool catalog throughout live validation. |

### Gmail

| ID | Disposition | Short basis |
| --- | --- | --- |
| GMAIL-01 | Not reproduced | No current setup flow reopened an unrelated completed task. |
| GMAIL-02 | Resolved | Fresh active Gmail capability was used successfully. |
| GMAIL-03 | Resolved | Bounded headers/snippet reads are complete within the active projection. |
| GMAIL-04 | Resolved | Current message reads avoid the invalid optional MIME projection. |
| GMAIL-05 | Resolved | Search transform and pagination are live-validated. |
| GMAIL-06 | Resolved | Every returned message in Cases 2 and 3 was fetchable. |
| GMAIL-07 | Resolved | Bounded base64url decoding passes its focused unit test. |
| GMAIL-08 | Resolved | The active operation set remained stable through validation. |
| GMAIL-09 | Resolved | Offline/consent OAuth revision is active; no auth intervention remains. |
| GMAIL-10 | Partial / retained | Reliability still relies on reduced 18-message snippet/header depth. |
| GMAIL-11 | Resolved | Current approved cases include per-message and per-thread coverage evidence. |

### Notion, MCP, and browser

| ID | Disposition | Short basis |
| --- | --- | --- |
| INT-01 | Resolved | Notion discovery and calls completed throughout live validation. |
| INT-02 | **Confirmed current** | Best-effort fallback still fires for all 22 Notion tools. |
| INT-03 | Resolved | Optional-null cursor handling has focused and live proof. |
| INT-04 | Not a current defect | Notion fetch accepts Notion objects; public webpages belong to web fetch. |
| INT-05 | Unverified scenario | No fresh competing browser-session ownership test was run. |
| INT-06 | Partial / retained | Old browser span remains open; general lease expiry persists. |
| INT-07 | Unverified scenario | The same JavaScript-required external form was not replayed. |
| INT-08 | Partial / retained | Shared origin defect remains; browser-specific approval was not replayed. |

### Memory, state, and observability

| ID | Disposition | Short basis |
| --- | --- | --- |
| STATE-01 | **Confirmed current** | One new invalid-provenance memory update. |
| STATE-02 | **Confirmed current** | Two new footnote/source-manifest failures. |
| STATE-03 | Partial / retained | Clients pair results, but 429 canonical calls remain `running`. |
| STATE-04 | Partial / retained | Clients mask much of it, but terminal runs retain running items. |
| STATE-05 | **Confirmed current** | Two debug spans remain open for more than an hour. |
| STATE-06 | **Confirmed current** | 12,503 post-baseline errors are dominated by repeated noise. |
| STATE-07 | Not a current defect | The live database is a disposable projection; Git/filesystem history is authoritative. |
| STATE-08 | **Confirmed current** | Two connections retain 26 compiled and 45 quarantined definitions. |

## Validation performed

- Fresh SQLite snapshot passed `PRAGMA quick_check`.
- `cargo validate test -p noema-store --lib due_processing_is_idempotent_and_applies_one_time_missed_policy`
- `cargo validate test -p noema-capabilities-mcp --lib --features transport optional_invalid_nulls_are_omitted_at_the_mcp_transport_boundary`
- `cargo validate test -p noema-capability-adapters --lib response_base64url_text_decoding_is_bounded_and_fails_closed`

All three focused tests passed. An initial MCP test invocation without the
`transport` feature selected zero tests; it was rerun with the owning feature and
passed one test. No smoke tests, fixture tests, browser inspection, external
writes, or connector mutations were performed for this audit.
