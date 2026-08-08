# Noema Practical Reliability Roadmap

- **Status:** Proposal
- **Mode:** Plan only
- **Date:** 2026-08-08
- **Primary evidence:** [Current-build UX audit](../../audits/2026-08-08-current-build-ux-survivors.md)
- **Live validation:** [Personal agent 50-case ledger](../../validation/personal-agent-50-case-ledger.md)
- **Security authority:** [Governed actions and approvals](../../harness/action-governance.md)

## 1. Purpose

Noema already has a task system, an action gateway, a web browser, MCP support,
and native API adapters. This proposal does not replace those systems.

This proposal connects the existing systems and repairs their current failure
paths. The final change adds reusable authority to the shared action gateway.

The proposal has ten change packages:

1. Store the exact foreground approval origin.
2. Reconcile Work from the current task fence.
3. Find and fix the Work lease-expiry cause.
4. Make conversation tool-call state correct.
5. Make Work items and debug spans terminal.
6. Give the task reviewer durable execution evidence.
7. Use exact review lineage without a fallback.
8. Finish the current Calendar and connector-definition path.
9. Make schema enforcement consistent for every tool source.
10. Add reusable grants for every governed action source.

## 2. Current product baseline

The current system is not an early prototype. Most common personal-agent work
already completes.

The current evidence shows:

- 49 of 50 live validation cases pass.
- The final case waits for a true all-day Calendar operation.
- Nine recent governed actions succeeded.
- Calendar writes return provider receipts and support read-back.
- Gmail, Calendar, and Notion support cross-system tasks.
- Governed actions preserve exact payloads and one-shot approvals.

The current defects occur near interruption, recovery, evidence, and stored
status. These defects limit safe autonomy more than missing connector breadth.

```mermaid
flowchart LR
    H["Human request"] --> T["Task or foreground turn"]
    T --> R["Agent run"]
    R --> G["Governed action"]
    G --> C["Browser, MCP, or API connector"]
    C --> O["External outcome"]
    O --> X["Recovery and evidence gaps"]
    X --> U["User result"]
```

## 3. Terms

### 3.1 Foreground turn

A foreground turn is a normal chat interaction. The user waits for the reply
inside the conversation.

### 3.2 Work task

A Work task is durable background work. It can continue after the user closes
the browser or Noema restarts.

### 3.3 Governed action

A governed action is one exact proposed external effect. It stores the tool,
arguments, policy result, approval, execution state, and terminal output.

### 3.4 Task fence

A task fence identifies one task generation and one execution contract. It
prevents old runs, gates, or approvals from changing new task work.

### 3.5 Run lease

A run lease gives one worker temporary authority over one Work run. The worker
renews the lease while it remains active.

### 3.6 Canonical state

Canonical state is the authoritative state in SQLite. A client can derive a
view, but that view must not replace incorrect server state.

### 3.7 Reconciliation

Reconciliation reads durable Work records after an interruption. It selects the
next valid action, such as queue, complete, recover, or remain idle.

## 4. Current end-to-end mechanism

```mermaid
sequenceDiagram
    participant Human
    participant Agent
    participant Gateway as Action gateway
    participant Store
    participant Tool as Browser or connector
    participant Work

    Human->>Agent: Request an outcome
    Agent->>Gateway: Propose an exact tool call
    Gateway->>Store: Store governed action
    Gateway-->>Human: Request approval when necessary
    Human->>Gateway: Approve exact revision
    Gateway->>Tool: Execute saved arguments
    Tool-->>Gateway: Return result
    Gateway->>Store: Store terminal action state
    Store->>Work: Resume foreground turn or Work run
    Work-->>Human: Deliver result
```

The design is correct. The current problems exist in specific transitions.

## 5. Delivery order

```mermaid
flowchart TD
    P1["Phase 1: Action continuity<br/>Changes 1 through 3"]
    P2["Phase 2: State truth<br/>Changes 4 and 5"]
    P3["Phase 3: Review evidence<br/>Changes 6 and 7"]
    P4["Phase 4: Connector baseline<br/>Changes 8 and 9"]
    P5["Phase 5: Reusable authority<br/>Change 10"]

    P1 --> P2 --> P3 --> P4 --> P5
```

Each phase has an independent release boundary. Do not start Phase 5 before all
earlier completion criteria pass.

---

## Change 1: Store the exact foreground approval origin

### Current problem

The action record stores the conversation ID and turn ID. It does not store the
exact approval-request item that contains the original provider call details.

After approval, `publish_foreground_action_outcome` loads visible conversation
items. It searches those items for an approval request with the action ID.

The search failed 148 times in the current audit. The runtime logged
`governed action continuation origin is unavailable` and returned.

The external action can finish before this failure. The user can then miss the
result and the agent continuation.

Primary code:

- [`action_resolution.rs`](../../../crates/noema-runtime/src/daemon/runtime/action_resolution.rs)
- [`action_items.rs`](../../../crates/noema-runtime/src/daemon/runtime/transcript_persistence/action_items.rs)
- [`governed_actions.rs`](../../../crates/noema-store/src/governed_actions.rs)
- [`schema.rs`](../../../crates/noema-store/src/schema.rs)

Audit issues: `WORK-16`, `WORK-10`, and `INT-08`.

### Before

```mermaid
flowchart TD
    A["Governed action reaches terminal state"]
    B["Load visible conversation items"]
    C{"Matching approval item exists?"}
    D["Read provider call details"]
    E["Publish tool result and continue"]
    F["Log missing origin and stop"]

    A --> B --> C
    C -->|Yes| D --> E
    C -->|No| F
```

### Proposed mechanism

Store an exact durable origin reference when Noema persists the approval item.
The reference must include the item ID and the provider call identity.

Use this order:

1. Create and assess the governed action.
2. Persist the approval-request conversation item.
3. Bind that item to the exact action revision.
4. Resolve the action after the human decision.
5. Load the bound origin directly.
6. Append one deterministic result item.
7. Start one deterministic continuation.

The bind operation must use an expected action revision. It must reject a stale
or different action.

### After

```mermaid
flowchart TD
    A["Persist approval-request item"]
    B["Bind item ID to action revision"]
    C["Human resolves action"]
    D["Load exact bound origin"]
    E["Append deterministic tool result"]
    F["Start deterministic continuation"]
    G["Repeated resolution finds existing result"]

    A --> B --> C --> D --> E --> F
    C --> G
```

### Data change

Append one forward-only migration. Add one exact origin reference to the
governed-action authority or to a one-to-one origin table.

Do not copy the complete conversation item into the action row. Store only the
exact identifiers and provider-call fields required for continuation.

### Tests

1. Resolve an action after the approval item leaves visible replay.
2. Resolve the same action two times and create one continuation.
3. Restart after action completion but before result publication.
4. Reject an origin link for a different action revision.
5. Preserve foreground action output and ordinary provider identifiers.

### Completion criteria

- No continuation depends on a visible-transcript search.
- Every terminal foreground action has one terminal result item.
- Every eligible terminal result starts at most one continuation.
- Restart recovery produces the same result as live resolution.

### Non-goals

- Do not change Work action resumption.
- Do not add a new conversation model.
- Do not retain full private payloads in duplicate rows.

---

## Change 2: Reconcile Work from the current task fence

### Current problem

Work reconciliation can select the latest resolved gate before it applies the
current task fence. A later validation step rejects the stale gate.

The current audit found five failures with this condition. Repeated
reconciliation then produced repeated errors.

Primary code:

- [`work_reconciliation_snapshot.rs`](../../../crates/noema-store/src/work_reconciliation_snapshot.rs)
- [`work_reconciliation.rs`](../../../crates/noema-store/src/work_reconciliation.rs)
- [`task_controls.rs`](../../../crates/noema-store/src/task_controls.rs)

Audit issues: `WORK-05` and `WORK-10`.

### Before

```mermaid
flowchart TD
    A["Load current task"]
    B["Select latest resolved gate"]
    C{"Gate matches current generation and contract?"}
    D["Plan recovery action"]
    E["Return fence error"]
    F["Later reconciliation repeats the query"]

    A --> B --> C
    C -->|Yes| D
    C -->|No| E --> F --> B
```

### Proposed mechanism

Apply the current fence inside the gate query. A stale gate must not become a
candidate for the current snapshot.

Use this order:

1. Load the current task generation and contract.
2. Query only gates for that generation and contract.
3. Validate the selected row as defense in depth.
4. Plan one reconciliation action.
5. Record a recovery gate for a true invariant fault.
6. Do not retry the same deterministic fault without a state change.

### After

```mermaid
flowchart TD
    A["Load current task fence"]
    B["Query gates inside that fence"]
    C{"Current gate exists?"}
    D["Plan from current gate"]
    E["Plan without a gate"]
    F["Validate selected row"]
    G["Apply one action"]

    A --> B --> C
    C -->|Yes| D --> F --> G
    C -->|No| E --> G
```

### Data change

No schema change should be necessary. Use the current task, gate, generation,
and contract columns.

### Tests

1. Ignore a resolved gate from an old generation.
2. Ignore a resolved gate from a superseded contract.
3. Use the current resolved gate when its fence matches.
4. Produce the same action after repeated reconciliation.
5. Stop repeated error publication after one unchanged invariant fault.

### Completion criteria

- Reconciliation never selects a stale gate as current evidence.
- The five recorded fence scenarios produce valid actions.
- A stale gate never queues a run.
- An unchanged deterministic fault produces one actionable record.

### Non-goals

- Do not weaken generation or contract checks.
- Do not change the Work state machine.
- Do not delete old gate history.

---

## Change 3: Find and fix the Work lease-expiry cause

### Current problem

A worker gets a 120-second lease. The runtime renews that lease every 30
seconds.

The current audit found one valid executor run with `lease_expired`. The current
evidence does not identify the cause.

Primary code:

- [`task_runtime.rs`](../../../crates/noema-runtime/src/daemon/task_runtime.rs)
- [`task_controls.rs`](../../../crates/noema-store/src/task_controls.rs)

Audit issues: `WORK-15` and `INT-06`.

### Before

```mermaid
flowchart TD
    A["Worker claims run for 120 seconds"]
    B["Execution and heartbeat share runtime"]
    C["Heartbeat should run every 30 seconds"]
    D{"Renewal arrives before expiry?"}
    E["Continue run"]
    F["Mark run interrupted"]
    G["Recovery can queue another attempt"]

    A --> B --> C --> D
    D -->|Yes| E --> C
    D -->|No| F --> G
```

### Proposed mechanism

First add bounded timing evidence. Do not increase the lease before the evidence
identifies the delay.

Record these ordinary diagnostics:

- Planned heartbeat time.
- Actual heartbeat start time.
- SQLite renewal duration.
- Time remaining before lease expiry.
- Runtime shutdown or cancellation state.
- Provider call or tool phase at the delay.

Then fix the demonstrated cause. Possible fixes can include a blocking-call
boundary, a database-contention fix, or a supervisor scheduling fix.

### After

```mermaid
flowchart TD
    A["Worker claims run"]
    B["Heartbeat supervisor records delay"]
    C{"Renewal is healthy?"}
    D["Renew lease"]
    E["Cancel run before unsafe expiry"]
    F["Record exact delay cause"]
    G["Use normal recovery"]

    A --> B --> C
    C -->|Yes| D --> B
    C -->|No| E --> F --> G
```

### Data change

Prefer the current debug-span or system-error authority. Do not add lease timing
columns unless runtime evidence cannot survive the required diagnosis.

### Tests

1. Keep a run active through a long provider call.
2. Delay one heartbeat without crossing the lease boundary.
3. Cross the boundary and interrupt the run once.
4. Reject renewal after task cancellation.
5. Reject renewal after a generation change.

### Completion criteria

- The recorded expiry has a reproduced cause or a disproved current path.
- A healthy long run renews its lease.
- A stale worker cannot renew a lease.
- Recovery never overlaps two active workers for one run.

### Stop condition

Stop this change if no current path can reproduce the failure. Retain the new
diagnostic evidence and do not change lease policy without proof.

---

## Change 4: Make conversation tool-call state correct

### Current problem

The runtime stores a tool-call item with `running` status. It later stores a
separate terminal tool-result item.

The original tool-call item can remain `running`. The web and iOS clients group
the call with its result and show a completed view.

The audit found 429 conversation tool calls with results and `running` status.
The stored authority is false even when the screen looks correct.

Primary code:

- [`tool_lifecycle.rs`](../../../crates/noema-runtime/src/daemon/runtime/transcript_persistence/tool_lifecycle.rs)
- [`action_items.rs`](../../../crates/noema-runtime/src/daemon/runtime/transcript_persistence/action_items.rs)
- Conversation-item store update authority under `crates/noema-store`.

Audit issue: `STATE-03`.

### Before

```mermaid
flowchart LR
    A["Tool starts"] --> B["Store call: running"]
    B --> C["Tool finishes"]
    C --> D["Store result: completed"]
    D --> E["Client groups both items"]
    E --> F["Screen says complete"]
    D --> G["Database call still says running"]
```

### Proposed mechanism

Use the existing provider-call identity to settle the original tool-call item.
The terminal result and call update must use one store transaction where
possible.

Map terminal results as follows:

| Result | Tool-call status |
| --- | --- |
| Successful result | `completed` |
| Tool failure | `failed` |
| Human decline | `cancelled` or the current skipped representation |
| Runtime interruption | `interrupted` |
| Unknown external result | `failed` with governed uncertainty detail |

Do not put external uncertainty into a new conversation status. The governed
action remains the detailed uncertainty authority.

### After

```mermaid
flowchart LR
    A["Tool starts"] --> B["Store call: running"]
    B --> C["Tool finishes"]
    C --> D["One transaction"]
    D --> E["Update call: terminal"]
    D --> F["Insert result: terminal"]
    E --> G["Database and client agree"]
    F --> G
```

### Data change

No schema change should be necessary. Add or reuse a fenced conversation-item
update that identifies the exact call item.

### Tests

1. Complete one successful tool call.
2. Fail one tool call.
3. Resolve one declined governed action.
4. Resolve one uncertain governed action.
5. Replay the same terminal result without a second update conflict.

### Completion criteria

- A terminal result always has a terminal call.
- Conversation replay does not contain a completed result under a running call.
- Clients do not need sibling inference for correctness.

### Non-goals

- Do not combine call and result into one item.
- Do not redesign transcript presentation.
- Do not remove provider-call identifiers.

---

## Change 5: Make Work items and debug spans terminal

### Current problem

Terminal Work runs can retain running assistant outputs, tool calls, and tool
results. Debug spans can also remain open after the logical operation ends.

The audit found 567 running assistant outputs inside terminal runs. It also
found 20 running calls, two running results, and two old running spans.

Primary code:

- Work run-item persistence under `crates/noema-store/src/agent_runs`.
- [`task_transcript.rs`](../../../crates/noema-runtime/src/daemon/runtime/task_transcript.rs)
- [`runtime_debug.rs`](../../../crates/noema-runtime/src/daemon/runtime/runtime_debug.rs)
- Work terminal command handlers under `crates/noema-store/src/work_run_terminal_plan.rs`.

Audit issues: `STATE-04` and `STATE-05`.

### Before

```mermaid
flowchart TD
    A["Run writes active items"]
    B["Run reaches terminal command"]
    C["Run row becomes completed or failed"]
    D["Some child items remain running"]
    E["Some debug spans remain running"]
    F["UI hides part of the mismatch"]

    A --> B --> C
    C --> D --> F
    C --> E --> F
```

### Proposed mechanism

Make terminal run settlement own child-item and debug-span settlement. Apply the
change in the same command transaction when the data shares the store.

Use these rules:

1. Complete items that have a matching successful terminal result.
2. Fail items that have a matching failed terminal result.
3. Interrupt remaining active items when the run is interrupted.
4. Cancel remaining active items when the run is cancelled.
5. Close the run debug span with the same terminal reason.
6. Keep governed-action uncertainty as a separate detailed state.

### After

```mermaid
flowchart TD
    A["Run reaches terminal command"]
    B["Settle run row"]
    C["Settle all active child items"]
    D["Close debug span"]
    E["Commit terminal state"]
    F["All readers see one result"]

    A --> B --> C --> D --> E --> F
```

### Data change

No new state values are necessary. Use the current completed, failed,
cancelled, and interrupted values.

### Tests

1. Complete an executor run with a successful tool result.
2. Fail a run during a tool call.
3. Cancel a run during assistant generation.
4. Interrupt a run after lease loss.
5. Repeat terminal settlement and preserve idempotency.

### Completion criteria

- A terminal run has no active child item.
- A terminal run has no active debug span.
- Store reads and client projections show the same terminal state.

---

## Change 6: Give the task reviewer durable execution evidence

### Current problem

The executor submits result Markdown, criterion evidence Markdown, and artifact
IDs. The reviewer sees the submission and artifacts.

The reviewer does not receive the executor transcript items as authoritative
evidence. Current code explicitly excludes those items.

Six recent tasks required multiple review rounds. Review feedback often asked
for evidence that the executor asserted but did not attach or reproduce.

Primary code:

- [`work_run_context.rs`](../../../crates/noema-store/src/work_run_context.rs)
- [`work_run_context_records.rs`](../../../crates/noema-store/src/work_run_context_records.rs)
- [`task_run_context.rs`](../../../crates/noema-runtime/src/daemon/task_run_context.rs)
- [`submission.rs`](../../../crates/noema-tasks/src/submission.rs)

Audit issue: `WORK-19`.

### Before

```mermaid
flowchart TD
    A["Executor uses tools"]
    B["Store keeps run items and governed actions"]
    C["Executor writes evidence Markdown"]
    D["Reviewer sees submission and artifacts"]
    E["Reviewer cannot inspect cited tool result"]
    F["Reviewer asks for another run"]

    A --> B
    A --> C --> D --> E --> F
    B -.->|Not included| D
```

### Proposed mechanism

Create a bounded evidence projection from records that already exist. Do not
create a second evidence store.

The projection should contain:

- Terminal tool-call name.
- Terminal status.
- Safe result summary.
- External resource identifiers already allowed in persistence.
- Governed-action ID, revision, and terminal state.
- Artifact version links.
- Exact run and submission lineage.

The projection must omit secrets. It must keep authorized private evidence
behind current governed references when an inline copy is unnecessary.

### After

```mermaid
flowchart TD
    A["Executor run"]
    B["Stored terminal run items"]
    C["Stored governed actions"]
    D["Submission and artifacts"]
    E["Bounded evidence projection"]
    F["Reviewer checks each criterion"]
    G["Approve, request exact change, or ask human"]

    A --> B --> E
    A --> C --> E
    A --> D --> E
    E --> F --> G
```

### Data change

Prefer a derived store read. Add no schema field unless a required link does
not exist in current rows.

The model must not provide action IDs as trusted evidence. The store derives
them from task, run, contract, and submission links.

### Tests

1. Include a successful governed write in reviewer evidence.
2. Exclude an action from another run.
3. Exclude a stale action from another task generation.
4. Show an uncertain action without claiming success.
5. Preserve ordinary identifiers and exclude credential material.
6. Review a result with no matching tool evidence and reject its write claim.

### Completion criteria

- The reviewer can inspect stored evidence for each claimed effect.
- Repeated review caused only by missing evidence packaging stops.
- Evidence cannot cross task, run, contract, or generation boundaries.

### Non-goals

- Do not let the reviewer call external research tools.
- Do not make raw transcripts authoritative.
- Do not duplicate full private payloads.

---

## Change 7: Use exact review lineage without a fallback

### Current problem

`work_run_context.rs` first uses the run's `triggering_review_id`. If that value
is absent, it falls back to the task's latest review.

The later validator can detect a foreign review. However, the loader still
selects a review that did not trigger the run.

The historical failure did not recur after the audit cutoff. The unsafe path
still exists.

Primary code:

- [`work_run_context.rs`](../../../crates/noema-store/src/work_run_context.rs)
- Run queue helpers under `crates/noema-store/src/work_command_helpers.rs`.

Audit issue: `WORK-06`.

### Before

```mermaid
flowchart TD
    A["Load run context"]
    B{"Run has triggering review ID?"}
    C["Load triggering review"]
    D["Load task latest review"]
    E["Validate review lineage"]
    F["Possible foreign-review error"]

    A --> B
    B -->|Yes| C --> E
    B -->|No| D --> E
    E --> F
```

### Proposed mechanism

Use only the run's explicit trigger. A run that requires review context must
carry the exact review ID when Noema queues the run.

Use these rules:

1. Planner runs have no review trigger.
2. First executor runs have no review trigger.
3. Revision executor runs require their exact triggering review.
4. Reviewer runs require their exact triggering submission.
5. Missing required lineage becomes one invariant fault.

### After

```mermaid
flowchart TD
    A["Load run context"]
    B{"This run kind requires a review trigger?"}
    C["Load exact triggering review"]
    D["Continue without review context"]
    E["Open invariant recovery path"]
    F["Validate exact lineage"]

    A --> B
    B -->|No| D
    B -->|Yes and present| C --> F
    B -->|Yes and missing| E
```

### Data change

No schema change should be necessary. `agent_runs` already stores
`triggering_review_id` and `triggering_submission_id`.

### Tests

1. Load a first executor run without a review.
2. Load a revision executor from its exact review.
3. Reject a revision executor with no trigger.
4. Reject a trigger from another submission.
5. Do not read the task's latest review as a substitute.

### Completion criteria

- No run context uses the task's latest review as causal evidence.
- Every revision run has one exact review lineage.
- Missing lineage produces one clear recovery state.

---

## Change 8: Finish the Calendar and connector-definition path

### Current problem

The active Calendar connector cannot create a true all-day event. A reviewed
successor definition exists, but the current connection has not adopted it.

Validation Case 12 waits for this operation. The live store also contains 26
compiled definitions and 45 quarantined definitions for two active connections.

This change must finish the active operation without deleting useful history.

Primary code:

- Adapter definition and connection stores under `crates/noema-capabilities/adapters/src`.
- Adapter management API under `crates/noema-api/src/graphql/adapters.rs`.
- Capability settings under `apps/web/src/components/settings`.

Audit issues: `CAL-02` and `STATE-08`.

### Before

```mermaid
flowchart TD
    A["Active Calendar connection"]
    B["Current reviewed definition"]
    C["No true all-day create operation"]
    D["Successor definition proposed"]
    E["Human must inspect definition history"]
    F["Validation case remains waiting"]

    A --> B --> C --> F
    D --> E --> F
```

### Proposed mechanism

Adopt the reviewed successor through the existing managed replacement path.
Keep the complete replacement lineage.

Improve the management view so the active definition and pending successor are
the primary records. Keep older and quarantined records in history disclosure.

Use this order:

1. Review the exact all-day operation and response transform.
2. Approve the successor definition.
3. Replace the active definition through current lineage rules.
4. Preserve the connection credential and grant authority.
5. Run Case 12.
6. Confirm create, read-back, and duplicate prevention.

### After

```mermaid
flowchart TD
    A["Active Calendar connection"]
    B["One active reviewed definition"]
    C["True all-day create operation"]
    D["Provider receipt"]
    E["Read-back confirms all-day event"]
    F["Case 12 passes"]
    G["Older definitions remain in history"]

    A --> B --> C --> D --> E --> F
    B --> G
```

### Data change

The definition files remain canonical. Do not rewrite an existing definition or
migration. Create a new immutable definition only when another correction is
necessary.

### Tests and validation

1. Compile the successor under strict schema version 8.
2. Preserve connection, credential, grant, and policy revisions correctly.
3. Create one all-day event with no invented time.
4. Read the event back as an all-day event.
5. Prevent a duplicate event during retry.
6. Run the complete 50-case ledger.

### Completion criteria

- All 50 validation cases pass.
- The active connection resolves one current definition.
- Historical definitions remain available without dominating the normal view.

### Non-goals

- Do not delete quarantine history during startup.
- Do not build a general connector migration framework.
- Do not change MCP through this work.

---

## Change 9: Make schema enforcement consistent for every tool source

### Current problem

Noema presents tools from native adapters, MCP servers, and the browser to
model providers. Each tool has one canonical input schema.

Provider interfaces accept a smaller schema language. The shared lowering path
must translate each canonical schema into that language.

The current failure is visible through Notion. All 22 active Notion tools use
schema forms that strict lowering rejects. The runtime recorded 12,342 repeated
fallback errors after the audit cutoff.

Notion is the largest current example. It is not the correct abstraction
boundary. Any present or future tool source can use the same schema forms.

Primary code:

- [`strict_schema.rs`](../../../crates/noema-providers/src/response_support/strict_schema.rs)
- [`tool_names.rs`](../../../crates/noema-providers/src/response_support/tool_names.rs)
- [`diagnostics.rs`](../../../crates/noema-providers/src/response_support/diagnostics.rs)
- Native adapter schemas under `crates/noema-capabilities/adapters`.
- MCP tool schemas under `crates/noema-capabilities/mcp`.
- Browser tool schemas under `crates/noema-capabilities/src/web`.

Audit issues: `INT-02` and `STATE-06`.

### Before

```mermaid
flowchart TD
    A["Native, MCP, or browser tool schema"]
    B["Shared strict-schema lowering"]
    C{"Provider supports every schema form?"}
    D["Use strict provider decoding"]
    E["Fall back during each compilation"]
    F["Write the same diagnostic many times"]
    G["Invoke tool with uneven guarantees"]

    A --> B --> C
    C -->|Yes| D --> G
    C -->|No| E --> F --> G
```

### Proposed mechanism

Make schema enforcement a property of the compiled tool definition. Do not
make it a property of Notion, MCP, or another integration.

Use one pipeline for every tool source:

1. Load the canonical tool schema.
2. Calculate its stable schema digest.
3. Lower it for the selected provider interface.
4. Mark the compiled tool as `strict` or `best_effort`.
5. Keep the canonical schema as the final runtime authority.
6. Record one diagnostic for each unique lowering result.

A `strict` result means the provider schema preserves the canonical accepted
input set. The provider can reject invalid arguments before generation ends.

A `best_effort` result means the provider cannot express the complete schema.
Noema must still validate the exact generated arguments against the canonical
schema before any tool invocation.

Never add a Notion-specific schema rewrite. Improve the shared lowering path
when a canonical schema has an equivalent provider representation.

Keep best-effort mode when no equivalent exists. The mode must remain visible
through inspection and diagnostics.

### After

```mermaid
flowchart TD
    A["Any canonical tool schema"]
    B["Shared compile step"]
    C{"Exact provider representation exists?"}
    D["Mark compiled tool strict"]
    E["Mark compiled tool best effort"]
    F["Canonical runtime validation"]
    G["Invoke native, MCP, or browser tool"]
    H["One diagnostic per schema and target digest"]

    A --> B --> C
    C -->|Yes| D --> F
    C -->|No| E --> F
    E --> H
    F --> G
```

### Why this mechanism is universal

The mechanism depends on only three existing facts:

- A canonical tool schema.
- A provider schema target.
- The generated arguments.

It does not know about pages, events, messages, files, travel, or any other
domain object.

The same result applies to a native API operation, an MCP operation, and a
browser operation.

### Data change

No database schema change should be necessary. Use the tool identity,
canonical schema digest, provider target, and lowering version as the
diagnostic identity.

If compiled adapter definitions already store enough identity, derive the mode
without a new persisted field.

### Tests

1. Lower all active tool schemas through one table-driven test.
2. Include one native adapter, one MCP server, and one browser tool.
3. Confirm strict enforcement when the provider representation is exact.
4. Confirm canonical runtime validation in best-effort mode.
5. Reject an extra generated argument before any connector invocation.
6. Record one fallback diagnostic for one unchanged schema and target.
7. Record a new diagnostic after a real schema or lowering revision.
8. Preserve ordinary schema names and identifiers in diagnostics.

The current Notion tools remain a required regression set. They do not receive
a separate production path.

### Completion criteria

- Every active tool reports one explicit enforcement mode.
- Every tool uses canonical validation before invocation.
- Strict-capable schemas use strict provider decoding.
- Repeated compilation does not flood the error log.
- No connector contains a private schema-enforcement exception.
- Existing native, MCP, and browser validation cases continue to pass.

### Non-goals

- Do not weaken canonical tool validation.
- Do not change MCP or provider protocols.
- Do not make all canonical schemas artificially closed.
- Do not hide a new or changed fallback.
- Do not add integration-specific schema adapters.

---

## Change 10: Add reusable grants for every governed action source

### Prerequisite

Changes 1 through 9 must meet their completion criteria. Reusable authority
must not hide an interruption, evidence, or stored-state defect.

### Current problem

Noema converts reviewed external effects into governed actions. Foreground
turns and Work tasks use the same action gateway.

The shared runtime entry point is `prepare_reviewed_action`. It receives a
source-neutral `CapabilityBinding` for native, MCP, and browser tools.

The current entry point returns early when connection policy says
`execute_immediately`. Those calls do not create a governed-action record.
Reusable grants therefore cannot govern or explain that automatic path.

An approval currently authorizes one immutable action revision. This is the
correct default, but it cannot express a reusable human decision.

A person can approve one Calendar change, one Notion update, or one browser
submission. The approval cannot safely authorize a later action with bounded
differences.

The action-governance contract already states the missing mechanism. Reusable
authority belongs in a separate grant model. Approval history must not become
an implicit permission system.

Primary authority:

- [Governed actions and approvals](../../harness/action-governance.md)
- [Security and egress policy](../../harness/security.md)
- [`action_gateway.rs`](../../../crates/noema-runtime/src/daemon/runtime/action_gateway.rs)
- [`governed_actions.rs`](../../../crates/noema-store/src/governed_actions.rs)
- [`integration.rs`](../../../crates/noema-capabilities/src/integration.rs)

### Before

```mermaid
flowchart TD
    A["Foreground or Work tool call"]
    B["Resolve connection and tool policy"]
    C{"Policy says execute immediately?"}
    D["Invoke without governed-action record"]
    E["Create exact governed action"]
    F["Run deterministic and semantic review"]
    G{"One-shot approval required?"}
    H["Human approves this revision"]
    I["Execute saved action"]
    J["Later similar action starts again"]

    A --> B --> C
    C -->|Yes| D --> J
    C -->|No| E --> F --> G
    G -->|Yes| H --> I
    G -->|No| I
    I --> J --> A
```

### Proposed mechanism

Add one reusable grant authority beside one-shot approvals. Keep the governed
action as the only external-effect execution authority.

Change `prepare_reviewed_action` so every proposed external effect first gets a
governed-action record. Keep the observed-URL safe-read path separate because
it does not create an external effect.

Reuse `CapabilityConnectionPolicy` and `CapabilityToolPolicy` as lower-bound
policy inputs. Do not copy their fields into the grant model.

A grant describes what one human permits across future governed actions. It
matches fields that the governed-action envelope already contains:

- Authorizing human and acting principal.
- Governable scope, such as one task, project, workspace, or human.
- Capability and exact connection.
- Operation and effect class.
- Resource selector.
- Destination and audience.
- Egress class and information constraints.
- Allowed changed fields and bounded argument values.
- Start, end, and revocation state.
- Grant revision and policy revision.
- Required audit and verification behavior.

The grant must use a small closed constraint vocabulary. Initial constraints
should support exact equality, allowed sets, absence, numeric bounds, maximum
collection size, and an allowed changed-field set.

Constraint paths refer to the canonical tool schema. Unknown paths, unknown
values, and unsupported comparisons do not match.

Do not put event, page, message, flight, or file concepts into the grant
engine. A connector exposes structured arguments and action facts through its
existing tool contract.

### Grant creation

A human must create a grant explicitly. Noema must never infer one from repeat
approvals or model text.

The creation flow is:

1. Show the human one exact governed action.
2. Offer `Approve once` as the default.
3. Offer a separate reusable-grant flow when the operation supports it.
4. Show every proposed constraint and its practical effect.
5. Store the grant only after explicit confirmation.
6. Reassess the original action against the new grant.

The server can prefill exact values from the action and its canonical schema.
Only schema-backed controls can widen a value into an allowed set or bound.
Free-form policy text cannot define executable constraints.

The human can also create or revoke a grant from policy settings. The same
server command must own both creation surfaces.

### Grant evaluation

Use this order for every proposed effect:

1. Build the exact governed action.
2. Resolve current connection policy and exact tool policy.
3. Run hard security, secret, network, and egress checks.
4. Select active grants inside the action's current scope.
5. Match all structured action fields and constraints.
6. Run the current semantic action review when it is required.
7. Require one-shot approval when no grant fully matches.
8. Revalidate all action, grant, connection, and tool revisions.
9. Execute the saved action and record the exact grant revision.

A grant supplies human authority. It does not override a hard deny, active
revocation, stale task fence, changed payload, uncertain result, or failed
semantic review.

### After

```mermaid
flowchart TD
    A["Any foreground or Work effect"]
    B["Exact governed action"]
    C["Current connection and tool policy"]
    D["Hard policy and egress checks"]
    E["Match active reusable grants"]
    F{"One grant matches every constraint?"}
    G["Continue action review with grant authority"]
    H["Require one-shot approval"]
    I["Revalidate every authority revision"]
    J["Execute through current connector"]
    K["Record outcome and grant evidence"]

    A --> B --> C --> D --> E --> F
    F -->|Yes| G --> I
    F -->|No or unknown| H --> I
    I --> J --> K
```

### Why this mechanism is universal

The grant engine evaluates governed-action facts. It does not evaluate a
Calendar event, a Notion page, or a travel booking directly.

These examples use the same mechanism:

| Example action | Grant constraints | Result |
| --- | --- | --- |
| Move an owner-only Calendar block | Exact connection, update operation, no attendees, time fields only, bounded hours | Grant can match |
| Update one Notion database status | Exact connection, database selector, status field only, allowed status set | Grant can match |
| Submit one known browser form | Exact origin, operation, destination, field set, and egress class | Grant can match |
| Send email to a new recipient | Recipient is outside the allowed destination set | Require one-shot approval |
| Purchase an item | Effect class exceeds the grant | Require one-shot approval |

Calendar, Notion, and browser code provide schemas and action facts. They do
not implement separate grant evaluators.

### Authority and storage

Add a separate durable grant authority. Do not add reusable fields to approval
rows.

Each grant must contain exact owner, principal, scope, capability, connection,
operation, resource, destination, constraint, policy, and revision identities.

Store no credentials in a grant. Reference authorized private payloads when
the grant does not need an inline value.

Grant states should include `active`, `revoked`, `superseded`, and `expired`.
Use an expiry only when the human sets one or the source authority requires
one.

Every automatic execution records the exact grant ID and revision. Immediate
revocation prevents new execution admissions.

Append one forward-only migration. Add one grant table with immutable revision
rows and one validated `constraints_json` value.

Add the selected grant ID and revision to the governed-action assessment. The
terminal action event must retain the same reference for audit.

Add one bounded index for active grant selection by owner, scope, capability,
and operation. Do not scan approval history during evaluation.

Expose server commands to create, list, inspect, revoke, and supersede grants.
The approval card and policy settings must call the same commands.

```mermaid
stateDiagram-v2
    [*] --> Active: Human creates grant
    Active --> Revoked: Human revokes grant
    Active --> Superseded: Human replaces constraints
    Active --> Expired: Human-defined expiry passes
    Revoked --> [*]
    Superseded --> [*]
    Expired --> [*]
```

### Tests

1. Match one native API action through the shared grant evaluator.
2. Match one MCP action through the same evaluator.
3. Match one structured browser action through the same evaluator.
4. Reject a different principal, scope, connection, or operation.
5. Reject a different resource, destination, or audience.
6. Reject an extra changed field or a value outside its bound.
7. Reject an unknown schema path or unsupported comparison.
8. Reject a revoked, superseded, expired, or stale-revision grant.
9. Preserve hard denies and secret rules when a grant matches.
10. Record the exact grant revision on one successful action.

Use table-driven cases for shared matching behavior. Add separate tests only
where a connector owns a different canonical-schema boundary.

### Completion criteria

- Foreground turns and Work tasks use the same grant evaluation.
- Native API, MCP, and browser actions use one grant engine.
- No connector contains domain-specific grant logic.
- One-shot approval remains the default when no exact grant matches.
- Every automatic effect becomes a governed action with an audit trail.
- Existing connection and tool policy remains a lower-bound authority.
- Revocation blocks new admissions immediately.
- Unknown or stale state fails closed to one-shot approval or denial.

### Non-goals

- Do not infer grants from prose, titles, approval history, or model output.
- Do not let models create, widen, or renew grants.
- Do not add wildcard capability, connection, resource, or destination grants.
- Do not add a general programming or expression language.
- Do not bypass action review, egress governance, or execution revalidation.
- Do not create domain abstractions for Calendar, Notion, travel, or email.

---

## 6. Cross-change acceptance scenario

Use one scenario to prove that the repaired systems and universal mechanisms
work together.

The human creates two reusable grants. One permits bounded changes through a
native Calendar adapter. The other permits bounded changes through Notion MCP.

The human then starts a Work task that maintains the project schedule and
status. The task contract requires approval before any external message.

```mermaid
sequenceDiagram
    participant Human
    participant Task
    participant Gateway as Action gateway
    participant Calendar as Native Calendar
    participant Notion as Notion MCP
    participant Email
    participant Reviewer

    Human->>Task: Maintain schedule and project status
    Task->>Gateway: Propose exact Calendar update
    Gateway->>Calendar: Execute under Calendar grant
    Calendar-->>Task: Return verified result
    Task->>Gateway: Propose exact Notion update
    Gateway->>Notion: Execute under Notion grant
    Notion-->>Task: Return verified result
    Task->>Gateway: Propose ungranted email
    Gateway-->>Human: Request one-shot approval
    Human->>Gateway: Approve exact email revision
    Gateway->>Email: Execute saved email action
    Email-->>Task: Return terminal result
    Task->>Reviewer: Submit result with stored evidence
    Reviewer-->>Task: Approve criteria
    Task-->>Human: Deliver verified result
```

Run these interruption variants:

1. Restart before the email approval.
2. Restart after approval but before email execution.
3. Restart after email success but before task continuation.
4. Cancel the task before approval.
5. Revoke either reusable grant before its action execution.
6. Change an action field after grant assessment.
7. Make one connector result uncertain.

For each variant, Noema must preserve completed effects. It must not repeat an
uncertain effect.

## 7. Validation plan

### 7.1 Focused unit tests

Each change lists its unique tests. Keep each test at the lowest authoritative
layer.

Do not repeat the same transition across store, runtime, API, and client tests.

### 7.2 Repository validation

Run these commands before each change commit:

```text
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
```

Run focused commands through `cargo validate` during each change.

### 7.3 Live validation

After Change 9, run the complete 50-case ledger and compile every active tool
schema. After Change 10, run the shared grant matrix across native API, MCP,
and browser actions.

Do not use live high-consequence writes for this proposal.

## 8. Release boundaries

| Release | Included changes | User-visible result | Rollback point |
| --- | --- | --- | --- |
| Action continuity | 1 through 3 | Approvals and Work resume reliably | Disable new origin use while preserving migration data |
| State truth | 4 and 5 | Stored status matches completed work | Revert terminal settlement code |
| Review evidence | 6 and 7 | Review uses stored execution evidence | Remove the derived evidence projection |
| Connector baseline | 8 and 9 | All 50 cases pass with useful logs | Restore the prior active connector definition |
| Reusable authority | 10 | Exact reusable grants work across governed action sources | Revoke or disable the grant records |

## 9. Explicitly deferred work

The current audit includes other problems. They are real, but they do not belong
in the same implementation unit.

Deferred work includes:

- Automatic memory provenance and footnote-manifest failures.
- Foreground latency and provider HTTP failures.
- Active-context overflow before any history can compact.
- Gmail full-message depth beyond the current bounded projection.
- The infeasible 07:00 news recurrence contract.
- A general policy programming language.
- General proactive outcome maintenance.
- New connector substrates.
- New domain abstractions.

Create a separate bounded proposal for each selected item. Do not expand this
roadmap during implementation.

## 10. Final system after this proposal

```mermaid
flowchart TD
    H["Human request or task trigger"]
    T["Task contract and current fence"]
    R["Leased agent run"]
    A["Exact governed action"]
    P["Hard policy and egress checks"]
    G["Reusable grant or one-shot approval"]
    C["Current browser or connector"]
    O["Stored terminal outcome"]
    E["Derived reviewer evidence"]
    V["Criterion review"]
    D["Verified delivery or recovery gate"]

    H --> T --> R --> A --> P --> G --> C --> O --> E --> V --> D
    G -.->|Human decision when required| H
    O -.->|Uncertain result| D
```

The result is one universal action boundary. Tasks and connectors keep their
current ownership, while schemas and reusable authority use shared mechanisms.
