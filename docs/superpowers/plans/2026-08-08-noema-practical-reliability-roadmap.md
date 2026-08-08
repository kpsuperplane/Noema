# Noema Plain-Code and Reliability Roadmap

- **Status:** Proposal
- **Mode:** Plan only
- **Date:** 2026-08-08
- **Primary evidence:** [Current-build UX audit](../../audits/2026-08-08-current-build-ux-survivors.md)
- **Live validation:** [Personal agent 50-case ledger](../../validation/personal-agent-50-case-ledger.md)
- **Safety rules:** [Current action checks and approvals](../../harness/action-governance.md)

## 1. Purpose

Noema already has a task system, an action gateway, and several tool sources.
This proposal does not replace those systems.

This proposal first gives the existing systems clear names. It then repairs
their current failure paths. It keeps the current immediate and reviewed tool
routes. It does not add a general permission or outgoing-data runtime.

The proposal has ten change packages:

0. Set the plain-language contract for code that this roadmap changes.
1. Store the exact chat approval source.
2. Recover tasks from the current task version.
3. Find and fix the worker-claim expiry cause.
4. Make conversation tool-call state correct.
5. Make task run items and debug spans final.
6. Give the task reviewer stored execution evidence.
7. Use the exact source review without a fallback.
8. Complete the reviewed tool-definition replacement path.
9. Make schema enforcement consistent for every tool source.

## 2. Current product baseline

The current system is not an early prototype. Most common personal-agent work
already completes.

The current evidence shows:

- 49 of 50 live validation cases pass.
- The final case waits for one operation in a reviewed replacement definition.
- Nine recent action requests succeeded.
- Recent external writes return provider receipts and support read-back.
- Current tool sources support cross-system tasks.
- Action requests preserve exact arguments and one-time approvals.

The current defects occur near interruption, recovery, evidence, and stored
status. These defects limit safe autonomy more than missing connector breadth.

```mermaid
flowchart LR
    H["Human request"] --> T["Background task or chat turn"]
    T --> R["Agent run"]
    R --> P["Connection policy and tool risk facts"]
    P --> D{"Current execution decision"}
    D -->|Execute immediately| C["Current tool source"]
    D -->|Review| G["Exact action request"]
    G --> C
    C --> O["External outcome"]
    O --> X["Recovery and evidence gaps"]
    X --> U["User result"]
```

## 3. Terms

### 3.1 Chat turn

A chat turn is a normal chat interaction. The user waits for the reply inside
the conversation.

### 3.2 Background task

A background task can continue after the user closes the app or Noema
restarts.

### 3.3 Action request

An action request is one exact tool call that current policy sends to human or
model review. It stores the tool, arguments, review result, approval, execution
state, and final output.

### 3.4 Current-run check

A current-run check identifies one run, worker claim, task version, and set of
task requirements. It stops old work from changing a newer task version.

### 3.5 Worker claim

A worker claim gives one worker temporary control of one task run. The worker
renews the claim while the run remains active.

### 3.6 Stored state

Stored state is the source data in SQLite. A client can make a view from this
data, but the view cannot correct false stored data.

### 3.7 Restart recovery

Restart recovery reads stored task records after an interruption. It selects
the next valid step, such as queue, complete, recover, or remain idle.

### 3.8 Source schema

A source schema is the complete input rule supplied by a tool. Provider schema
conversion can make a smaller copy for a model service.

## 4. Current end-to-end mechanism

```mermaid
flowchart TD
    H["Human request"]
    A["Agent proposes a tool call"]
    P["Connection policy and tool risk facts"]
    D{"Current execution decision"}
    I["Immediate dispatch"]
    S["Store exact action request"]
    R["Model or human review"]
    E["Dispatch reviewed arguments"]
    T["Current tool source"]
    O["Store tool result"]
    F["Chat or task follow-up"]

    H --> A --> P --> D
    D -->|Execute immediately| I --> T
    D -->|Model or human review| S --> R --> E --> T
    T --> O --> F
```

This is the current code shape. Reliability changes must repair one of these
paths without replacing the policy model.

## 5. Delivery order

```mermaid
flowchart LR
    L["Plain-language contract<br/>Change 0"]
    A["Exact action source<br/>Change 1"]
    Q["Current recovery query<br/>Change 2"]
    X["Claim-expiry investigation<br/>Change 3"]
    S["Final stored state<br/>Changes 4 and 5"]
    R["Exact review evidence<br/>Changes 6 and 7"]
    C["Definition adoption<br/>Change 8"]
    V["Shared schema check<br/>Change 9"]

    L --> A
    L --> V
    L --> Q
    L --> X
    L --> S --> R
    L --> C
```

Each branch has an independent release boundary. The small language contract
in Change 0 must finish first. A full repository rename is separate work.

---

## Change 0: Set the plain-language contract for changed code

### Current problem

Noema uses several private platform terms for mechanisms that already have
simple names. These terms appear in the product, code, APIs, logs, and docs.

A new contributor must first learn Noema vocabulary. Only then can the
contributor understand the mechanism.

A current scoped scan found:

- 726 exact uses of the product word `Work`.
- 396 uses of the identifier form `governed_action`.
- 85 uses of the type `WorkRunFence`.
- 63 uses of the field `triggering_review_id`.
- 19 uses of `strict_schema` or `lower_strict_schema`.
- 331 uses of the word `canonical`.

These counts exclude generated files and third-party license text. Some uses
are valid technical terms. Most listed product terms are not necessary.

### What STE compliance means here

Apply ASD-STE100 directly to product text, current docs, code comments, prompts,
logs, and human-readable errors.

Code identifiers do not contain sentences. Apply the same approved vocabulary
and one-meaning rule to identifiers.

Keep a necessary technical noun only when no approved common word preserves
the meaning. Define that noun once in the terms source.

Do not claim formal ASD-STE100 certification from an automated word search. The
reader-path review must also confirm that the code explains its behavior.

Primary code areas:

- Task domain code under `crates/noema-tasks`.
- Task storage under `crates/noema-store/src/work_*`.
- Task runtime code under `crates/noema-runtime/src/daemon/task_*`.
- [`action_gateway.rs`](../../../crates/noema-runtime/src/daemon/runtime/action_gateway.rs)
- [`governed_actions.rs`](../../../crates/noema-store/src/governed_actions.rs)
- [`strict_schema.rs`](../../../crates/noema-providers/src/response_support/strict_schema.rs)
- GraphQL task and action-request fields under `crates/noema-api`.
- Web task code under `apps/web/src/components/work`.
- iOS task and action-request operations under `apps/ios/Noema`.

### Before

```mermaid
flowchart TD
    H["Human sees Work and governed action"]
    D["Docs explain fence, lineage, and canonical state"]
    A["API exposes WorkEvent and GovernedAction"]
    C["Code uses WorkRunFence and strict lowering"]
    S["SQLite uses work_events and governed_actions"]
    Q["Reader translates every layer"]

    H --> Q
    D --> Q
    A --> Q
    C --> Q
    S --> Q
```

The terms move across system boundaries. They also appear in generated clients
and error messages. A documentation-only rename will not solve the problem.

### Required term changes

Use this language contract in every file that this roadmap changes:

| Current term | Plain term | Example code name | Direct meaning |
| --- | --- | --- | --- |
| `Work` | Tasks | `TaskCommandService`, `TaskEvent` | The task product and its stored activity |
| Canonical state | Stored state | `StoredTaskState` when a type needs the distinction | The source data in SQLite |
| Task fence | Current-run check | `TaskRunCheck` | The values that prove a write belongs to the current run |
| Governed action | Action request | `ActionRequest`, `action_requests` | One saved tool action that Noema must check |
| Review lineage | Source review | `source_review_id` | The review that requested the next run |
| Schema lowering | Provider schema conversion | `convert_provider_schema` | Noema adapts a tool schema for a model service |
| Strict lowering | Exact schema conversion | `convert_schema_exactly` | The converted schema accepts the same inputs |
| Canonical runtime validation | Final source-schema check | `validate_tool_input` | Noema checks tool input before invocation |

Other words remain unchanged until a reader-path test proves a problem. Use the
exact external term when Noema implements an external protocol.

### Context-dependent words

Do not make a global text replacement for `canonical`, `strict`, or `work`.
Select the word that describes the specific mechanism.

| Current phrase | Replacement |
| --- | --- |
| Canonical database state | Stored state or source data |
| Canonical tool schema | Source schema |
| Canonical arguments | Saved exact arguments |
| Canonical file | Source file |
| Canonical JSON encoding | Normalized JSON encoding |
| Strict provider schema | Exact provider schema |
| Strict JSON parser | Keep this term when the parser follows a named strict mode |
| Workflow | Keep this word because it has a different established meaning |
| Ordinary English work | Keep this word when it is not the Tasks product name |

### Terms that are not mandatory renames

Do not rename a standard technical term only because a new reader might not
know it. Terms such as `payload`, `principal`, `idempotency`, and `adapter` can
be precise in the correct context.

Keep such a term when an external protocol owns it or when a common replacement
would remove a necessary distinction. Define it near its first use.

Package and directory names are also candidates, not approved changes. First
trace one real reader path. Rename a package only when the path proves that the
name hides its current responsibility. Do not combine that rename with a
reliability repair.

The deferred plain-code program can inspect the rest of the repository in
small, buildable slices. Each slice needs its own reader evidence, budget,
tests, and product decision.

### Code readability rules

Apply these rules to the files that an implementation slice changes:

1. One mechanism has one noun across Rust, GraphQL, TypeScript, Swift, SQL, and docs.
2. A type name states what data the type contains.
3. A function name states the action and its result.
4. A Boolean name reads as a true-or-false statement.
5. A state name describes an observable state.
6. An error states what failed, why it failed, and what remains unchanged.
7. A log identifies the operation, object, result, and safe reason.
8. A module name matches the product or mechanism that it owns.
9. A comment explains a required rule or a non-obvious reason.
10. A comment must not translate a confusing identifier into plain language.
11. An acronym appears only when an external protocol owns it.
12. The first module comment expands each required external acronym.
13. Generic words such as `manager`, `handler`, `helper`, and `service` need a specific owned action.
14. New aliases cannot preserve a replaced internal name.
15. Generated code must come from a source schema that uses the approved terms.

### Reader-path test

Each implementation slice must trace the real path that it changes. Its before
and after diagrams must name the actual modules, types, functions, database
records, and API fields. A reviewer with basic Rust and TypeScript knowledge
must be able to explain that path with the terms document.

The deferred plain-code program must use the same test for each candidate
rename. A search count alone is not evidence that a rename helps a reader.

### Proposed mechanism

Create one terms source at `docs/development/terms.md`. It must contain
the approved term and its definition. It must also state which old forms new
prose and changed identifiers must not add.

Apply the contract in two small steps.

1. Add the terms document and link it from `AGENTS.md`.
2. Use the approved names in this roadmap and in each file that a later change
   edits.

Do not rename an untouched package, route, API field, SQL table, or generated
client in this change. A later change can rename a touched identifier when the
rename fits its budget and does not increase release risk.

### Build sequence

```mermaid
flowchart LR
    A["Approve eight private-term replacements"]
    B["Add one terms document"]
    C["Use the terms in each changed reliability path"]
    D["Run a separate plain-code program later"]

    A --> B --> C
    B --> D
```

The path from A through C is the only reliability prerequisite. The separate
plain-code program does not block Changes 1 through 9.

### After

```mermaid
flowchart TD
    T["Terms document defines the eight private terms"]
    R["Roadmap uses the approved terms"]
    C["Changed code uses the same terms"]
    F["Untouched code can change in later bounded slices"]

    T --> R --> C
    T --> F
```

### Database changes

None. Do not rename a database object only to complete this language contract.
If a later reliability change needs a schema change, it must use a forward-only
migration.

### API and generated-client changes

None. Do not rename a public API or generated client only to complete this
language contract.

### Tests and validation

Review this roadmap and the terms document. Confirm that each approved term has
one definition. Confirm that no replacement removes a necessary distinction.
This change adds no production code and no tests.

### Completion criteria

- The terms document defines the eight confirmed private terms.
- This roadmap uses those terms.
- Each later change uses those terms in the files that it edits.
- No repository-wide rename blocks a reliability change.

### Stop conditions

- Stop when a proposed name changes behavior or removes a necessary distinction.
- Stop when a rename would require a migration or client regeneration that the
  reliability change does not otherwise need.
- Stop when one term needs two meanings inside the same subsystem.

### Non-goals

- Do not redesign Tasks, action checks, recovery, or schema enforcement.
- Do not rename packages, routes, public APIs, database objects, or generated
  clients as a general cleanup.
- Do not rename terms owned by an external protocol.
- Do not add a repository-wide banned-word check.

### Names used by later changes

Changes 1 through 9 use the plain terms in prose. Current file names can remain
until a bounded implementation slice changes them safely. Current audit labels
can also remain because they identify fixed historical findings.

---

## Change 1: Store the exact chat approval source

### Current problem

The action-request record stores the conversation ID and chat-turn ID. It does
not store the exact approval item that contains the original model tool-call ID.

After approval, `publish_foreground_action_outcome` loads visible conversation
items. It searches those items for an approval request with the action ID.

The search failed 148 times in the current audit. The agent runner logged a
missing action source and stopped.

The external action can finish before this failure. The user can then miss the
result and the agent follow-up.

Current code owners:

- Agent follow-up code under `noema-runtime`.
- Conversation-item code under `noema-runtime`.
- Action-request database code under `noema-store`.
- Database migrations under `noema-store`.

Audit issues: `WORK-16`, `WORK-10`, and `INT-08`.

### Before

```mermaid
flowchart TD
    A["Action request reaches final state"]
    B["Load visible conversation items"]
    C{"Matching approval item exists?"}
    D["Read model tool-call details"]
    E["Publish tool result and continue"]
    F["Log missing source and stop"]

    A --> B --> C
    C -->|Yes| D --> E
    C -->|No| F
```

### Proposed mechanism

Store an exact source reference when Noema saves the approval item. The
reference must include the item ID and the model tool-call ID.

Use this order:

1. Create and assess the action request.
2. Save the approval-request conversation item.
3. Link that item to the exact action version.
4. Resolve the action after the human decision.
5. Load the linked source directly.
6. Append one repeat-safe result item.
7. Start one repeat-safe follow-up run.

The link operation must use an expected action version. It must reject an old
or different action.

### After

```mermaid
flowchart TD
    A["Save approval-request item"]
    B["Link item ID to action version"]
    C["Human resolves action"]
    D["Load exact linked source"]
    E["Append repeat-safe tool result"]
    F["Start one repeat-safe follow-up run"]
    G["Repeated resolution finds existing result"]

    A --> B --> C --> D --> E --> F
    C --> G
```

### Data change

Append one forward-only migration. Store the existing approval conversation
item ID on the current action-request row. Do not add a source table.

Load the model tool-call ID from that conversation item. Add the model tool-call
ID to the same action row only if the linked item cannot provide it. Do not copy
the complete conversation item into the action row.

### Tests

1. Load the exact source after it leaves the visible view, and reject a link to
   another action version.
2. Restart after action completion but before publication, then process the
   action twice and create one result item and one follow-up run.
3. Preserve chat output and ordinary model-service identifiers at the new
   storage boundary.

### Completion criteria

- No follow-up run depends on a visible-conversation search.
- Every final chat action has one final result item.
- Every eligible final result starts at most one follow-up run.
- Restart recovery produces the same result as live resolution.

### Non-goals

- Do not change background-task action resumption.
- Do not add a new conversation model.
- Do not retain full private content in duplicate rows.

---

## Change 2: Recover tasks from the current task version

### Current problem

Task restart recovery can select the latest resolved pause before it checks the
current task version. A later check rejects the old pause.

The current audit found five failures with this condition. Repeated
recovery then produced repeated errors.

Current code owners:

- Task recovery input under `noema-store`.
- Task recovery rules under `noema-store`.
- Current-run checks under `noema-store`.

Audit issues: `WORK-05` and `WORK-10`.

### Before

```mermaid
flowchart TD
    A["Load current task"]
    B["Select latest resolved task pause"]
    C{"Pause matches current task version and requirements?"}
    D["Plan recovery action"]
    E["Return current-run error"]
    F["Later recovery repeats the query"]

    A --> B --> C
    C -->|Yes| D
    C -->|No| E --> F --> B
```

### Proposed mechanism

Apply the current task version inside the pause query. An old pause must not
become a candidate for the current recovery view.

Use this order:

1. Load the current task version and requirements.
2. Query only pauses for that task version and requirements.
3. Validate the selected row as defense in depth.
4. Plan one recovery step.

### After

```mermaid
flowchart TD
    A["Load current task version"]
    B["Query pauses for that version"]
    C{"Current pause exists?"}
    D["Plan from current pause"]
    E["Plan without a pause"]
    F["Validate selected row"]
    G["Apply one action"]

    A --> B --> C
    C -->|Yes| D --> F --> G
    C -->|No| E --> G
```

### Data change

No schema change should be necessary. Use the current task, pause, version, and
requirements columns.

### Test

Add one table-driven regression test. It must include an old task version,
replaced requirements, and the current matching pause. The query must return
only the current matching pause.

### Completion criteria

- Restart recovery never selects an old pause as current evidence.
- The five recorded version scenarios produce valid steps.
- An old pause never queues a run.

### Non-goals

- Do not weaken task version or requirements checks.
- Do not change the task state machine.
- Do not delete old pause history.
- Do not add a new fault-recording or retry mechanism.

---

## Change 3: Find and fix the worker-claim expiry cause

### Current problem

A worker gets a 120-second claim. The task runner renews that claim every 30
seconds.

The current audit found one valid task-worker run whose claim expired. The
current evidence does not identify the cause.

Current code owners:

- Task-worker supervision under `noema-runtime`.
- Worker-claim database code under `noema-store`.

Audit issues: `WORK-15` and `INT-06`.

### Before

```mermaid
flowchart TD
    A["Worker claims run for 120 seconds"]
    B["Execution and claim renewal share one process"]
    C["Claim renewal should run every 30 seconds"]
    D{"Renewal arrives before expiry?"}
    E["Continue run"]
    F["Mark run as interrupted"]
    G["Recovery can queue another attempt"]

    A --> B --> C --> D
    D -->|Yes| E --> C
    D -->|No| F --> G
```

### Proposed mechanism

First add limited timing evidence. Do not increase the claim before the evidence
identifies the delay.

Record these ordinary diagnostics:

- Planned claim-renewal time.
- Actual claim-renewal start time.
- SQLite renewal duration.
- Time remaining before claim expiry.
- Agent-runner shutdown or cancellation state.
- Model-service call or tool phase at the delay.

Reproduce the expiry and name its measured cause. Then make the smallest change
that removes that cause. Do not select an architecture before reproduction.

### After

```mermaid
flowchart TD
    A["Worker claims run"]
    B["Current renewal path records timing"]
    C{"Can the expiry be reproduced?"}
    D["Name the measured cause"]
    E["Apply one cause-specific fix"]
    F["Prove the fix with one regression test"]
    G["Keep diagnostics and stop"]

    A --> B --> C
    C -->|Yes| D --> E --> F
    C -->|No| G
```

### Data change

Prefer the current debug-span or system-error record. Do not add claim timing
columns unless the current diagnostic records cannot preserve the evidence.

### Tests

1. Add one regression test that reproduces the measured cause and proves the
   cause-specific fix.
2. Add a current-run safety test only if the fix changes cancellation or task
   version checks.

### Completion criteria

- The recorded expiry has a reproduced cause or a disproved current path.
- A healthy long run renews its worker claim.
- An old worker cannot renew a claim.
- Recovery never overlaps two active workers for one run.

### Stop condition

Stop this change if no current path can reproduce the failure. Retain the new
diagnostic evidence and do not change claim timing without proof.

---

## Change 4: Make conversation tool-call state correct

### Current problem

The agent runner stores a tool-call item with `running` status. It later stores
a separate final tool-result item.

The original tool-call item can remain `running`. The web and iOS clients group
the call with its result and show a completed view.

The audit found 429 conversation tool calls with results and `running` status.
The stored state is false even when the screen looks correct.

Current code owners:

- Tool-call state under `noema-runtime`.
- Action-request conversation items under `noema-runtime`.
- Conversation-item updates under `noema-store`.

Audit issue: `STATE-03`.

### Before

```mermaid
flowchart LR
    A["Tool starts"] --> B["Save call: running"]
    B --> C["Tool finishes"]
    C --> D["Save result: completed"]
    D --> E["Client groups both items"]
    E --> F["Screen says complete"]
    D --> G["Database call still says running"]
```

### Proposed mechanism

Use the existing model tool-call ID to finish the original tool-call item.
The final result and call update must use one database transaction where
possible.

Map final results as follows:

| Result | Tool-call status |
| --- | --- |
| Successful result | `completed` |
| Tool failure | `failed` |
| Human decline | `cancelled` or the current skipped representation |
| Agent-runner interruption | `interrupted` |
| Unknown external result | `failed` with action-request uncertainty detail |

Do not put external uncertainty into a new conversation status. The action
request remains the detailed uncertainty record.

### After

```mermaid
flowchart LR
    A["Tool starts"] --> B["Save call: running"]
    B --> C["Tool finishes"]
    C --> D["One transaction"]
    D --> E["Update call: final"]
    D --> F["Insert result: final"]
    E --> G["Database and client agree"]
    F --> G
```

### Data change

No schema change should be necessary. Add or reuse a conditional
conversation-item update that identifies the exact call item.

### Tests

1. Use one table-driven test for successful, failed, declined, and uncertain
   outcomes.
2. Process the same final result again without a second update conflict.

### Completion criteria

- A final result always has a final call.
- Conversation history does not contain a completed result under a running call.
- Clients do not need sibling inference for correctness.

### Non-goals

- Do not combine call and result into one item.
- Do not redesign transcript presentation.
- Do not remove model tool-call identifiers.

---

## Change 5: Make task run items and debug spans final

### Current problem

Completed task runs can retain running assistant outputs, tool calls, and tool
results. Debug spans can also remain open after the operation ends.

The audit found 567 running assistant outputs inside final runs. It also
found 20 running calls, two running results, and two old running spans.

Current code owners:

- Task run-item database code under `noema-store`.
- Task run-item recording under `noema-runtime`.
- Agent-runner diagnostics under `noema-runtime`.
- Final task-run updates under `noema-store`.

Audit issues: `STATE-04` and `STATE-05`.

### Before

```mermaid
flowchart TD
    A["Run writes active items"]
    B["Run reaches final command"]
    C["Run row becomes completed or failed"]
    D["Some child items remain running"]
    E["Some debug spans remain running"]
    F["UI hides part of the mismatch"]

    A --> B --> C
    C --> D --> F
    C --> E --> F
```

### Proposed mechanism

Make the final run update also finish child items and the debug span. Apply the
change in the same database transaction when the data shares the database.

Use these rules:

1. Complete items that have a matching successful final result.
2. Fail items that have a matching failed final result.
3. Interrupt remaining active items when the run is interrupted.
4. Cancel remaining active items when the run is cancelled.
5. Close the run debug span with the same final reason.
6. Keep action-request uncertainty as a separate detailed state.

### After

```mermaid
flowchart TD
    A["Run reaches final command"]
    B["Finish run row"]
    C["Finish all active child items"]
    D["Close debug span"]
    E["Commit final state"]
    F["All readers see one result"]

    A --> B --> C --> D --> E --> F
```

### Data change

No new state values are necessary. Use the current completed, failed,
cancelled, and interrupted values.

### Tests

1. Use one table-driven test for completed, failed, cancelled, and interrupted
   runs with active child items.
2. Close the matching debug span with the same final reason.
3. Repeat the final update and preserve repeat protection.

### Completion criteria

- A final run has no active child item.
- A final run has no active debug span.
- Database queries and client views show the same final state.

---

## Change 6: Give the task reviewer stored execution evidence

### Current problem

The task worker submits result Markdown, criterion evidence Markdown, and saved
output IDs. The reviewer sees the submission and saved outputs.

The reviewer does not receive the task worker's stored run items as source
evidence. Current code explicitly excludes those items.

Six recent tasks required multiple review rounds. Review feedback often asked
for evidence that the task worker asserted but did not attach or reproduce.

Current code owners:

- Task review input under `noema-store`.
- Task review records under `noema-store`.
- Reviewer input assembly under `noema-runtime`.
- Task submissions under `noema-tasks`.

Audit issue: `WORK-19`.

### Before

```mermaid
flowchart TD
    A["Task worker uses tools"]
    B["Database keeps run items and action requests"]
    C["Task worker writes evidence Markdown"]
    D["Reviewer sees submission and saved outputs"]
    E["Reviewer cannot inspect cited tool result"]
    F["Reviewer asks for another run"]

    A --> B
    A --> C --> D --> E --> F
    B -.->|Not included| D
```

### Proposed mechanism

Pass records that Noema already stores to the reviewer. Use the final run items,
the submitted result, and the saved-output references for the exact task run.

Filter these records through their current task, run, requirements, version,
and submission links. Use the existing context boundary to exclude secrets.
Preserve authorized private information and ordinary identifiers.

Do not create a summary model, evidence table, or transformed evidence copy. If
the exact records do not fit the reviewer input, stop and get a separate product
decision about input limits.

### After

```mermaid
flowchart TD
    A["Task-worker run"]
    B["Stored final run items"]
    C["Submission"]
    D["Saved-output references"]
    E["Filter by exact current links"]
    F["Reviewer checks each criterion"]
    G["Approve, request exact change, or ask human"]

    A --> B --> E
    A --> C --> E
    A --> D --> E
    E --> F --> G
```

### Data change

None. Reuse the current records and their current links.

The model must not provide action IDs as trusted evidence. The database derives
them from task, run, requirements, and submission links.

### Tests

1. Include the exact final items and saved references from the current run.
2. Exclude records from another run, version, or requirements set.
3. Preserve authorized private and ordinary values, exclude credentials, and
   keep an uncertain result uncertain.

### Completion criteria

- The reviewer can inspect stored evidence for each claimed effect.
- Repeated review caused only by missing evidence packaging stops.
- Evidence cannot cross task, run, requirements, or task-version boundaries.

### Non-goals

- Do not let the reviewer call external research tools.
- Do not make raw chat history the source data.
- Do not add a review-evidence summary or a second evidence authority.

---

## Change 7: Use the exact source review without a substitute

### Current problem

The review-input query first uses the run's `source_review_id`. If that value is
absent, it falls back to the task's latest review.

The later check can detect an unrelated review. However, the query still
selects a review that did not request the run.

The historical failure did not recur after the audit cutoff. The unsafe path
still exists.

Current code owners:

- Task review input under `noema-store`.
- Task-run queue code under `noema-store`.

Audit issue: `WORK-06`.

### Before

```mermaid
flowchart TD
    A["Load review input"]
    B{"Run has source review ID?"}
    C["Load source review"]
    D["Load task latest review"]
    E["Check the source review"]
    F["Possible unrelated-review error"]

    A --> B
    B -->|Yes| C --> E
    B -->|No| D --> E
    E --> F
```

### Proposed mechanism

Use only the run's explicit source review. A run that requires review input
must carry the exact review ID when Noema queues the run.

Use these rules:

1. Task-planner runs have no source review.
2. First task-worker runs have no source review.
3. Correction runs require their exact source review.
4. Reviewer runs require their exact source submission.
5. A missing required source link becomes one stored-state fault.

### After

```mermaid
flowchart TD
    A["Load review input"]
    B{"This run role requires a source review?"}
    C["Load exact source review"]
    D["Continue without review input"]
    E["Open stored-state recovery path"]
    F["Check the exact source review"]

    A --> B
    B -->|No| D
    B -->|Yes and present| C --> F
    B -->|Yes and missing| E
```

### Data change

No schema change should be necessary. `agent_runs` already stores the current
fields `triggering_review_id` and `triggering_submission_id`.

### Tests

Add one table-driven regression test for every run role. It must prove that a
correction uses its exact source review, rejects a missing or unrelated source,
and never reads the task's latest review as a substitute.

### Completion criteria

- No review input uses the task's latest review as source evidence.
- Every correction run has one exact source review.
- A missing source review produces one clear recovery state.

---

## Change 8: Complete the reviewed tool-definition replacement path

### Current problem

One active connection cannot run an operation required by the final live
validation case. A reviewed replacement definition contains the operation, but
the current connection has not adopted that definition.

The database also contains 26 built definitions and 45 rejected definitions
for two active connections.

This change must finish the active operation without deleting useful history.

Current code owners:

- Connector definitions and connections under `noema-capabilities`.

Audit issues: `CAL-02` and `STATE-08`.

### Before

```mermaid
flowchart TD
    A["Active tool connection"]
    B["Current reviewed definition"]
    C["Required operation is absent"]
    D["Replacement definition proposed"]
    E["Current replacement path must adopt it"]
    F["Final validation case remains waiting"]

    A --> B --> C --> F
    D --> E --> F
```

### Proposed mechanism

Adopt the reviewed replacement through the existing managed replacement path.
Keep the complete replacement history.

Use this order:

1. Review the exact operation, source schema, and response conversion.
2. Approve the replacement definition.
3. Replace the active definition through current replacement rules.
4. Preserve the connection credential and permission rules.
5. Run the waiting live case.
6. Confirm the effect, read it back, and prevent a repeated effect.

### After

```mermaid
flowchart TD
    A["Active tool connection"]
    B["One active reviewed definition"]
    C["Required operation"]
    D["Provider receipt"]
    E["Independent read-back confirms the effect"]
    F["Final validation case passes"]
    G["Older definitions remain in history"]

    A --> B --> C --> D --> E --> F
    B --> G
```

### Data change

The definition files remain the source files. Do not rewrite an existing
definition or migration. Create a new definition identity for each correction.

### Tests and validation

1. Build and adopt the replacement while the connection keeps its credentials
   and current rules.
2. Run the waiting live case. Perform one low-risk effect, read it back, and
   confirm that a repeated attempt does not create the effect twice.

### Completion criteria

- All 50 validation cases pass.
- The active connection resolves one current definition.

### Non-goals

- Do not delete rejected-definition history during startup.
- Do not build a general connector migration framework.
- Do not add logic for one service, object type, or operation name.
- Do not change the connector management interface.

---

## Change 9: Make schema enforcement consistent for every tool source

### Current problem

Noema presents tools from several sources to model services. Each tool has one
source input schema.

Model-service interfaces accept a smaller schema language. The shared
conversion must translate each source schema into that language.

All 22 active tools from one current source use schema forms that exact
conversion rejects. The task runner recorded 12,342 repeated partial-mode
errors after the audit cutoff.

The affected source proves the defect. It does not define the system boundary.
Any present or future tool source can use the same schema forms.

Current code owners:

- Model-service schema conversion under the model-service package.
- Tool-schema collection under the model-service package.
- Conversion diagnostics under the model-service package.
- Source tool schemas under `noema-capabilities`.

Audit issues: `INT-02` and `STATE-06`.

### Before

```mermaid
flowchart TD
    A["Tool schema from any source"]
    B["Shared model-service schema conversion"]
    C{"Model service supports every schema form?"}
    D["Use exact model-service input rules"]
    E["Use partial rules during each build"]
    F["Write the same diagnostic many times"]
    G["Invoke tool with uneven guarantees"]

    A --> B --> C
    C -->|Yes| D --> G
    C -->|No| E --> F --> G
```

### Proposed mechanism

Make schema enforcement a property of the built tool definition. Do not make
it a property of one source or integration.

Use one pipeline for every tool source:

1. Load the source tool schema.
2. Calculate its stable schema hash.
3. Convert it for the selected model-service interface.
4. Mark the converted schema as `exact` or `partial`.
5. Keep the source schema for the final input check.
6. Record one diagnostic for each unique conversion result.

An `exact` result means the model-service schema preserves the accepted source
set. The model service can reject invalid arguments before its response ends.

A `partial` result means the model service cannot express the complete schema.
Noema must still check the generated arguments against the source schema before
any tool invocation.

Never add a source-specific schema rewrite. Improve the shared conversion when
a source schema has an equivalent model-service representation.

Keep partial mode when no equivalent exists. The mode must remain visible in
inspection and diagnostics.

### After

```mermaid
flowchart TD
    A["Any source tool schema"]
    B["Shared build step"]
    C{"Exact model-service representation exists?"}
    D["Mark converted schema exact"]
    E["Mark converted schema partial"]
    F["Final source-schema input check"]
    G["Invoke the current tool source"]
    H["One diagnostic per schema and target hash"]

    A --> B --> C
    C -->|Yes| D --> F
    C -->|No| E --> F
    E --> H
    F --> G
```

### Why this mechanism is universal

The mechanism depends on only three existing facts:

- A source tool schema.
- A model-service schema target.
- The generated arguments.

It does not know about any service, tool name, or domain object. The same result
applies to every tool source.

### Data change

No database schema change should be necessary. Use the tool identity, source
schema hash, model-service target, and conversion version as the diagnostic
identity.

If built connector definitions already store enough identity, derive the mode
without a new saved field.

### Tests

1. Convert all active tool schemas through one table-driven test.
2. Include tools from at least two materially different sources in the exact
   and partial conversion cases.
3. Run the final source-schema check in partial mode and reject an extra
   argument before connector invocation.
4. Deduplicate an unchanged partial diagnostic, create one after a real input
   change, and preserve ordinary schema identifiers.

The 22 currently affected tools remain a required test set. They do not receive
a separate production path.

### Completion criteria

- Every active tool reports one explicit enforcement mode.
- Every tool uses the source-schema check before invocation.
- Exact conversions use exact model-service input rules.
- Repeated builds do not flood the error log.
- No connector contains a private schema-enforcement exception.
- Existing validation cases for every active tool source continue to pass.

### Non-goals

- Do not weaken the source-schema input check.
- Do not change connector or model-service protocols.
- Do not make all source schemas artificially closed.
- Do not hide a new or changed partial conversion.
- Do not add connector-specific schema conversions.

---

## 6. Cross-change acceptance scenario

Use one scenario to prove that the repaired current execution paths work
together.

The human starts one background task that uses all three current execution
routes. The task first makes an immediate call. It then makes a call that the
model reviewer can clear. Its final call requires one-time human approval.

```mermaid
sequenceDiagram
    participant Human
    participant Task
    participant Policy as Connection and tool policy
    participant ActionReview as Action reviewer
    participant SourceA as Tool source A
    participant SourceB as Tool source B
    participant SourceC as Tool source C
    participant TaskReview as Task reviewer

    Human->>Task: Request one result across three tool sources
    Task->>Policy: Propose bounded read for source A
    Policy->>SourceA: Dispatch immediately
    SourceA-->>Task: Return confirmed result
    Task->>Policy: Propose reviewed call for source B
    Policy->>ActionReview: Store and review exact action
    ActionReview->>SourceB: Dispatch reviewed arguments
    SourceB-->>Task: Return confirmed result
    Task->>Policy: Propose ambiguous call for source C
    Policy->>ActionReview: Store exact action
    ActionReview-->>Human: Request one-time approval
    Human->>ActionReview: Approve the exact action revision
    ActionReview->>SourceC: Dispatch reviewed arguments
    SourceC-->>Task: Return final result
    Task->>TaskReview: Submit result with stored evidence
    TaskReview-->>Task: Approve criteria
    Task-->>Human: Deliver verified result
```

Run these interruption variants:

1. Restart before the one-time approval.
2. Restart after approval but before execution.
3. Restart after success but before the task follow-up run.
4. Cancel the task before approval.
5. Change the reviewed arguments after approval.
6. Change the connection policy or tool schema before execution.
7. Make one connector result uncertain.

For each variant, Noema must preserve completed effects. It must not repeat an
uncertain effect.

## 7. Validation plan

### 7.1 Implementation budgets

These are limits, not targets. Production and test values are net changed lines.

| Change | Production budget | Test budget | Maximum new tests |
| --- | ---: | ---: | ---: |
| 0 | 0 | 0 | 0 |
| 1 | 100 to 250 | 30 to 100 | 3 |
| 2 | 10 to 60 | 20 to 60 | 1 |
| 3 | 20 to 100 | 0 to 60 | 2 |
| 4 | 25 to 100 | 30 to 100 | 2 |
| 5 | 25 to 120 | 30 to 120 | 3 |
| 6 | 25 to 100 | 30 to 120 | 3 |
| 7 | 5 to 30 | 20 to 60 | 1 |
| 8 | 0 to 80 | 20 to 80 | 2 |
| 9 | 50 to 180 | 50 to 180 | 4 |

Before each implementation slice, set the exact base revision. Measure the
patch with `bun run scripts/report-rust-size.ts --base <ref>` and the matching
budget flags.

Stop the slice when it exceeds its production or test estimate by 50 percent
or 500 lines, whichever is smaller. Reduce scope before more code is added.
More than ten new Rust tests needs a written risk matrix and a new scope review.

### 7.2 Focused unit tests

Each change lists its unique tests. Keep each test with the code that owns the
behavior.

Do not repeat the same transition across database, agent runner, API, and
client tests.

### 7.3 Repository validation

Run these commands before each change commit:

```text
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
```

Run focused commands through `cargo validate` during each change.

### 7.4 Live validation

After Change 9, run the complete 50-case validation list once and build every
active tool schema.

Do not use live high-risk writes for this proposal.

## 8. Release boundaries

| Release | Included changes | User-visible result | Undo point |
| --- | --- | --- | --- |
| Plain-language contract | 0 | Changed code uses one direct vocabulary | Revert the terms document |
| Action follow-up | 1 and 2 | Approvals and background tasks resume reliably | Disable new source use while preserving migration data |
| Claim diagnosis | 3 | Worker-claim expiry has measured evidence and a proved fix | Remove the added diagnostics and cause-specific fix |
| Stored status | 4 and 5 | Stored status matches completed work | Revert final-state update code |
| Review evidence | 6 and 7 | Review uses exact stored execution records | Remove the reviewer-input query extension |
| Definition adoption | 8 | The final waiting validation case passes | Restore the prior active definition |
| Shared schema check | 9 | All active tool sources use one schema pipeline | Revert the shared conversion change |

## 9. Later work

The current audit includes other problems. They are real, but they do not belong
in the same implementation unit.

Deferred work includes:

- Automatic memory-source and footnote-manifest failures.
- Chat reply delays and model-service HTTP failures.
- Model input that becomes too large before Noema can reduce history.
- Deeper content retrieval beyond the current limited view.
- The infeasible 07:00 news recurrence requirements.
- Repository-wide language cleanup. This includes candidate package, route,
  API, SQL, web, and iOS renames. Each candidate needs reader-path evidence.
- General grants or reusable rules. Reconsider them only after a current
  production task proves that human instructions, task-owned authority, and
  one-time approval are insufficient.
- One universal outgoing-data runtime. Keep current enforcement with its
  existing owners until a demonstrated gap requires a shared boundary.
- General automatic result maintenance.
- New connector types.
- New problem-specific base types.

Create a separate limited proposal for each selected item. Do not expand this
roadmap during implementation.

## 10. Final system after this proposal

```mermaid
flowchart TD
    H["Human request or task start"]
    T["Task requirements and current-run check"]
    R["Claimed task run"]
    P["Connection policy and tool risk facts"]
    X{"Current execution decision"}
    I["Immediate dispatch"]
    A["Exact reviewed action request"]
    M["Model review"]
    G["One-time approval when required"]
    C["Current tool source"]
    O["Stored final outcome"]
    E["Stored final run items and saved references"]
    V["Requirements review"]
    D["Confirmed delivery or recovery pause"]

    H --> T --> R --> P --> X
    X -->|Execute immediately| I --> C
    X -->|Review| A
    A -->|Model route| M
    A -->|Human route| G
    M -->|Cleared| C
    M -->|Ask human| G
    G --> C
    C --> O --> E --> V --> D
    G -.->|Human decision when required| H
    O -.->|Uncertain result| D
```

The result preserves `CapabilityExecutionDecision`, immediate dispatch, and the
reviewed-action path. Tasks and connectors keep their current ownership. The
roadmap repairs these paths without adding another authority.
