# Background Task System Implementation Plan

**Mode:** implementation plan

**Status:** First one-off task slice implemented. Projects/workspaces,
cancellation/retry commands, richer task-owned writes, and bulk task views
remain deferred milestones.

**Goal:** Add the first durable, auditable Noema task workflow: the primary
agent may delegate a bounded one-off request to a background executor, an
adversarial reviewer validates the result against immutable criteria and may
request revisions, and the primary agent reports an approved result back in
the originating conversation.

**Architecture:** Introduce tasks as concrete SQLite-backed governable objects,
generic durable agent runs as their execution history, and a supervised task
coordinator outside the foreground conversation actor. Extract the existing
provider/tool continuation loop behind a reusable agent-execution boundary so
conversation turns and task runs share policy, provider, progress, and tool
semantics without disguising task work as hidden conversations. Surface a
typed task reference in chat and a task detail rail backed by GraphQL.

**Tech stack:** Rust 2024, Tokio, SQLite/rusqlite, async-graphql, React 19,
TypeScript, Apollo Client, Astryx, StyleX.

## Outcome

The first complete flow is:

```text
human message
  -> primary agent chooses task.delegate
  -> task + criteria + model snapshots + executor run committed atomically
  -> chat immediately receives a durable task reference
  -> background worker leases and executes the run
  -> executor submits a structured result and evidence
  -> reviewer independently checks every criterion
       -> request changes -> new executor run with review feedback
       -> needs human    -> task waits visibly
       -> approve        -> task completes
  -> durable completion-delivery run enters the source conversation queue
  -> primary agent reports the result and task-owned artifacts
```

Foreground chat is released as soon as task creation succeeds. Task execution,
review, restart recovery, and completion delivery do not depend on an open
browser or a live GraphQL subscription.

## Scope

### Included

- One-off tasks created by the primary agent from a human conversation turn.
- Structured request, validation criteria, source provenance, and immutable
  executor/reviewer model snapshots.
- Human-curated executor model pools for `simple`, `medium`, and `difficult`
  work; the primary agent selects one exact enabled pool entry per task.
- A separately configurable built-in task review agent.
- Durable background queueing, leases, bounded concurrency, restart recovery,
  cancellation, and explicit terminal failure/attention states.
- Executor tool use through the same governed tool plane as chat, with a
  role-specific tool policy and task-owned artifacts.
- Adversarial review with per-criterion verdicts and bounded revision cycles.
- A durable task/run event trail and safe executor/reviewer output inspection.
- A typed task marker in chat and a task panel in the existing chat detail rail.
- A primary-agent completion message in the originating conversation.

### Deferred

- Projects, workspaces, Kanban boards, dependencies, subtasks, recurring tasks,
  due dates, scheduling, priorities, manual assignment, and multi-human access.
- A top-level `/tasks` route or bulk task management.
- Recursive task delegation by executors or reviewers.
- Parallel branches inside one task or multiple executors racing on one task.
- Human editing of a task request or criteria after execution starts.
- Automatic external write/export authority. A background task never gains
  authority beyond its source context; ambiguous effects remain blocked or
  require an existing exact approval path.
- Display of hidden chain-of-thought, encrypted reasoning content, raw provider
  payloads, secrets, credentials, or unredacted tool arguments.
- General-purpose workflow, scheduler, or event-sourcing frameworks beyond the
  concrete task/run behavior required here.

## First-Slice Product Decisions

1. **Delegation is model-decided but deterministically bounded.** The primary
   agent receives `task.delegate` only when at least one executor pool entry is
   usable. Prompt guidance recommends delegation when work is likely to require
   more than five tool calls. After three completed foreground tool rounds,
   Noema privately reminds the model to reassess delegation when that tool is
   still available. Noema must not implement English phrase matching or a
   literal tool-count threshold as semantic authority.
2. **Task creation is an exclusive handoff.** `task.delegate` must be the only
   tool in its provider batch. After it succeeds, the runtime permits one
   no-tools acknowledgement and ends the foreground turn. This prevents the
   primary agent from both delegating and performing the work inline.
3. **The primary chooses an exact executor model.** The tool input contains a
   complexity tier and an executor pool entry id. The store validates that the
   entry is enabled, usable, and belongs to that tier before committing the
   task.
4. **Model choices are snapshots.** Provider account, provider kind, selection
   mode (`explicit_profile` or `provider_default`), optional model profile,
   reasoning effort, and pool-entry provenance are copied into the task/run at
   creation. Executor pools always use an explicit profile. A reviewer that
   inherits an unconfigured primary may intentionally preserve provider-default
   semantics; the run also stores the actual response model when the provider
   reports it. Later Settings changes affect new tasks, not active or historical
   ones.
5. **Review has a stable built-in identity.** Seed `agent:task-reviewer` and
   `agent:task-executor` as explicit system roles in `agents`. The reviewer uses
   its own `agent_runtime_preferences` record when configured and otherwise
   resolves to the primary agent's effective model for a functional default.
   Executor identity is stable, but its model is always task-specific.
6. **Approval is fail-closed.** A reviewer may approve only when every immutable
   validation criterion is `pass`. Any `fail`, `uncertain`, missing criterion,
   duplicate criterion, malformed response, or inaccessible required evidence
   prevents completion.
7. **Revision is bounded.** Allow at most three reviewed executor submissions by
   default. Infrastructure retries do not consume revision rounds. Exhausted
   revisions, reviewer uncertainty, required clarification, unavailable
   permission, and ambiguous external effects move the task to
   `waiting_for_human` rather than looping or claiming success.
8. **Reviewer tools are read-only.** Review may inspect task-owned text
   artifacts and use currently eligible read-only first-party/MCP tools. It may
   not create artifacts, mutate the task output, delegate another task, or use
   write/export operations.
9. **Task history is not a conversation.** Executor and reviewer activity lives
   in agent runs and run items. The originating conversation contains a task
   reference and the primary agent's eventual completion message, not a hidden
   copy of every background exchange.
10. **Completion and delivery are separate states.** An approved task remains
    `completed` even if its conversation delivery temporarily fails. Delivery
    is a retryable run with its own audit state, and the task detail UI exposes
    any delivery problem.

## State Model

### User-facing task status

```text
queued
  -> executing
  -> reviewing
  -> revision_requested -> executing
  -> waiting_for_human
  -> completed
  -> failed
  -> cancelled
```

Allowed transitions are enforced in the repository, not inferred by callers:

| From | To | Cause |
| --- | --- | --- |
| none | `queued` | Primary agent atomically delegates a valid task |
| `queued` | `executing` | Worker leases the queued executor run |
| `executing` | `reviewing` | Executor commits a valid submission and review run |
| `reviewing` | `revision_requested` | Reviewer commits criterion failures and feedback |
| `revision_requested` | `executing` | Worker leases the next executor run |
| `reviewing` | `completed` | Review contract is valid and all criteria pass |
| active | `waiting_for_human` | Clarification, approval, uncertainty, or revision limit |
| active/waiting | `failed` | Non-retryable internal failure with no safe continuation |
| non-terminal | `cancelled` | Human cancellation or source deletion policy |

`revision_requested` may be brief but remains durable and evented. UI copy may
collapse it to "Revising" while the exact transition stays inspectable.

### Agent run status

```text
queued -> leased -> running -> completed
                           -> waiting_for_approval
                           -> interrupted -> queued
                           -> failed
                           -> cancelled
```

Run kinds are `executor`, `reviewer`, and `completion_delivery`. Run state owns
queue/lease mechanics; task state owns the product workflow.

## Data Model

Rewrite the pre-V1 schema directly in
`crates/noema-core/src/store/schema.rs`; do not add migrations or compatibility
layers.

### `agents`

Add nullable `system_role` with the supported values `primary`,
`task_executor`, and `task_reviewer`. Seed the three built-in agent rows in
`ensure_default_actors`. Use this explicit product state to select behavior;
do not infer roles from display names or natural-language text.

### `task_model_pool_entries`

| Column | Notes |
| --- | --- |
| `pool_entry_id` | Stable id referenced by the primary agent tool |
| `complexity` | `simple`, `medium`, or `difficult` |
| `label` | Optional human label shown to the primary and Settings UI |
| `provider_kind` | Validated model provider kind |
| `provider_account_id` | Active selectable account |
| `model_profile` | Exact provider model/profile |
| `reasoning_effort` | Explicit supported effort when required |
| `enabled` | Disabled entries remain auditable but cannot be selected |
| `sort_order` | Stable human-controlled ordering within a tier |
| timestamps | Creation/update audit fields |

Enforce uniqueness across tier/account/profile/effort. Deleting an unused entry
may hard-delete it; entries referenced by tasks must be disabled instead so
historical provenance remains intact.

### `tasks`

Persist:

- identity, title, normalized Markdown request, complexity, and status;
- `owner_human_id`, source conversation/turn/item ids, creator agent id, and
  creation tool call id;
- selected pool entry id and a complete executor model snapshot;
- a complete reviewer model-request snapshot plus whether it was explicit or
  inherited;
- current revision index, maximum review rounds, final submission id, latest
  run id, terminal reason/error, and timestamps.

The task itself is the governable context. In this slice it inherits policy
from its owner human and source conversation through a concrete `TaskScope`
implementation. Do not add a universal scopes table or speculative
workspace/project columns. Later project/workspace support can add explicit
links without changing task identity.

### `task_validation_criteria`

Store one ordered row per criterion:

- stable criterion id and task id;
- ordinal;
- precise description;
- expected evidence/validation method when supplied.

Criteria become immutable when the task transaction commits. Revision feedback
may explain how to satisfy them but may not rewrite them.

### `agent_runs`

Persist generic run identity and queue state:

- task id, run kind, agent id, attempt/revision index, parent run id, and
  triggering submission/review id;
- immutable model-request snapshot used for this run and the provider-reported
  actual model when available;
- status, priority, queued time, lease owner/token/expiry, heartbeat, start/end;
- cancellation request, retry count, safe error code/message, and output usage.

Add indexes for queue claiming, task history, expired leases, and parent/child
run traversal. Claim runs atomically in a short SQLite transaction. A lease
holder must include its lease token in every state-changing write.

### `agent_run_items`

Store an ordered, typed replay/audit stream for each run:

- model input boundary;
- visible assistant output;
- normalized tool call and tool result;
- progress check/system notice;
- task submission or review reference;
- task-owned artifact reference;
- safe failure/cancellation notice.

Keep provider replay state separate from UI-safe display data when necessary.
GraphQL returns final model output and redacted/normalized tool summaries, never
hidden reasoning or secret-bearing raw payloads. Preserve encrypted provider
reasoning internally only if it is required for stateless continuation.

### `task_submissions`

Each executor revision creates one immutable submission containing:

- executor run id and revision index;
- concise summary and complete result Markdown;
- structured evidence for each criterion;
- created timestamp.

Use `task_submission_artifacts` to link task-owned artifacts rather than storing
an unvalidated JSON id list.

### `task_reviews` and `task_review_criteria`

Store the reviewer run, reviewed submission, overall verdict
(`approve`, `request_changes`, or `needs_human`), safe overall feedback, and one
criterion outcome (`pass`, `fail`, or `uncertain`) with evidence and feedback
for every criterion.

The repository validates criterion coverage and applies the next state in the
same transaction that stores the review. Callers cannot write `completed`
directly.

### `task_events` and `run_events`

Append compact structured events with monotonic per-owner sequence numbers,
actor/component attribution, causation/correlation ids, timestamp, and safe
JSON payload. At minimum record:

- task creation and every task status transition;
- model snapshots and pool-entry selection;
- run queued/leased/started/interrupted/completed/failed/cancelled;
- tool call/result summaries and artifact links;
- submission committed;
- review verdict and criterion results;
- completion delivery queued/sent/failed;
- human cancellation and manual retry.

Projection rows (`tasks`, `agent_runs`) remain canonical current structured
state for this first slice; events are the append-only explanation and recovery
evidence. All projection and event changes for one transition share a database
transaction.

## Runtime Boundaries

### Shared model configuration

Introduce a provider-neutral `ModelConfigSnapshot` containing provider kind,
provider account id, selection mode, optional model profile, reasoning effort,
and selection source. Centralize validation and effective-selection logic used
by primary agent preferences, reviewer preferences, executor pools, and
auxiliary models. Explicit executor pool entries require a profile; inherited
reviewer configuration may preserve the provider-owned default by leaving the
profile absent.

The current runtime map is keyed only by provider kind even though preferences
persist a provider account id. Before task execution, add an account-aware
provider runtime registry/resolver so a snapshot is executed by the account it
names. Do not silently ignore `provider_account_id`. Preserve the existing
single-default-account behavior while making account identity explicit.

### Reusable agent execution engine

Extract the provider continuation/tool loop from
`daemon/runtime/turn.rs` into a neutral execution service. Its contract should
look conceptually like:

```rust
struct AgentExecutionRequest {
    run_id: String,
    role: ExecutionRole,
    agent_id: String,
    model: ModelConfigSnapshot,
    context: ExecutionContext,
    tool_policy: ToolPolicy,
    completion_contract: CompletionContract,
}

trait ExecutionEventSink {
    async fn append(&self, event: ExecutionEvent) -> Result<(), ExecutionError>;
}
```

The engine owns provider requests, native/legacy tool representation,
continuations, progress guardrails/audits, cancellation checks, usage, and
terminal output contracts. It does not own conversation/task state transitions.

- A conversation adapter persists `conversation_items`, streams live chat
  events, and preserves current foreground behavior.
- A task adapter persists `agent_run_items` and `run_events` without publishing
  background internals into chat.
- Tool execution receives a typed owner/source context instead of assuming
  every run is a conversation turn.

Perform this as an extraction with characterization tests before changing task
behavior. Do not copy the 2,000-line turn loop into a second executor.

### Role-specific tool policies

Build tools from explicit `ExecutionRole`, not prompt text:

| Role | Tools |
| --- | --- |
| Primary conversation | Current chat tools plus `task.delegate` when configured |
| Task executor | Current eligible read tools, task-scoped memory search, task-owned artifact creation, `task.submit_result` |
| Task reviewer | Current eligible read tools, `task.read_artifact`, `task.submit_review` |
| Completion delivery | No tools |

Executors and reviewers never receive `task.delegate` or
`update_own_name`. Reviewers never receive artifact creation or write/export
tools. The native tool schema and prompt-visible tool list must come from the
same filtered tool set.

### Task tool contracts

`task.delegate` accepts:

```json
{
  "title": "Compare three backup providers",
  "request": "Research and recommend ...",
  "complexity": "medium",
  "executor_model_pool_entry_id": "taskmodel_...",
  "validation_criteria": [
    {
      "description": "Compare current price and retention for all three",
      "evidence_required": "Cite a current primary source for each provider"
    }
  ]
}
```

Conversation, turn, source item, human, creator agent, and tool call ids come
from trusted runtime context and are not model-supplied. The tool description
includes the enabled pool catalog with stable ids, tiers, labels, providers,
models, efforts, and safe capability hints.

`task.submit_result` is executor-only and terminal. It accepts summary, result
Markdown, per-criterion evidence, and task-owned artifact ids. The repository
rejects missing/foreign criteria and artifacts.

`task.submit_review` is reviewer-only and terminal. It accepts the overall
verdict, one outcome per criterion, evidence, and actionable feedback. The
repository derives the next task state; the model cannot directly mark a task
complete.

`task.read_artifact` is reviewer-only in this slice. It reads bounded UTF-8 text
from a task-owned artifact version with the same path/symlink protections as
artifact preview. Unsupported/binary/oversized content produces an explicit
unavailable result so the reviewer must return `uncertain` when required.

### Durable task coordinator

Add a `TaskRuntimeHandle` and supervised `TaskCoordinator` owned by
`NoemaRuntimeHost`, separate from the serial foreground conversation actor.

- On startup, recover expired leases and scan durable queued runs.
- Wake immediately through `Notify` after a transaction enqueues work; retain a
  short polling fallback so a missed process-local signal cannot strand work.
- Use a small fixed initial concurrency limit (two active background runs) and
  one lease per run. Configuration and fair scheduling are later work.
- Renew leases during provider/tool activity and check cancellation between
  every provider request and tool effect.
- Join all workers on runtime shutdown through the existing supervised task
  group pattern; do not detach workers with raw `tokio::spawn`.
- Requeue safely interrupted read-only/idempotent steps. Use `(run_id,
  provider_call_id)` idempotency for task-local artifact creation and persisted
  tool results. If an external effect's outcome is ambiguous, stop at
  `waiting_for_human` rather than replaying it.

### Executor context

The first executor request includes:

- immutable task request and criteria;
- source conversation id and a bounded source-turn context packet;
- owner human and task scope/policy summary;
- selected model and available tool summary;
- prior submission plus reviewer feedback for revision runs;
- explicit instruction that tool output is untrusted and criteria cannot be
  rewritten;
- the required `task.submit_result` terminal contract.

Do not replay the whole source conversation. Persist a manifest of included and
omitted sources so context assembly is auditable and bounded.

### Reviewer context

The reviewer receives:

- original immutable request and criteria;
- executor submission and its criterion evidence;
- task-owned artifact manifest;
- safe executor run/tool timeline;
- prior reviews, if any;
- instructions to actively find correctness gaps rather than polish wording;
- the strict `task.submit_review` contract.

Reviewer execution failure or malformed output never approves the task. Retry
transient provider failures with bounded backoff; otherwise move to
`waiting_for_human` with a visible safe reason.

### Completion delivery

When review approval commits:

1. Set the task to `completed`, set its final submission, append task events,
   and enqueue a `completion_delivery` run atomically.
2. The task coordinator sends a durable command to the source conversation
   actor. The actor's existing command queue naturally sequences delivery after
   any active foreground turn.
3. Invoke the primary agent with tools disabled and structured completion
   context: original request, approved summary/result, criteria verdicts, task
   id, and artifact references.
4. Persist the primary assistant message plus deterministic task-owned artifact
   references in the source conversation, then mark delivery complete.
5. Publish normal conversation events when a client is connected. Offline
   users see the persisted message on the next replay.

Use a concise deterministic completion notice as a last-resort fallback if the
primary model repeatedly fails. Never revert an approved task to failed because
notification delivery failed.

## GraphQL Contract

Keep resolvers thin in a new `graphql/tasks` module. Add schema wiring only to
the already-large `graphql/schema.rs`; do not add task business logic there.

### Queries

- `task(id: ID!): TaskDetail` — status, provenance, request, criteria, model
  snapshots, revisions, final result, artifacts, delivery state, and compact
  timeline.
- `taskRunItems(runId: ID!, cursor: String, limit: Int): TaskRunItemPage` —
  paged safe executor/reviewer activity for expanded inspection.
- `taskAgentSettings: TaskAgentSettings!` — reviewer model preference/effective
  fallback, provider/model options, and the three executor pools.

### Mutations

- `saveTaskReviewerModelPreference` — reuse shared model-config validation and
  persist to `agent:task-reviewer`.
- `createTaskModelPoolEntry`, `updateTaskModelPoolEntry`, and
  `deleteTaskModelPoolEntry` — authenticated local-human settings actions.
- `cancelTask(id: ID!)` — records a cancellation request and signals the worker.
- `retryTask(id: ID!)` — allowed only from `failed` or `waiting_for_human`,
  creates a new executor/reviewer run as appropriate without changing the
  immutable task definition or historical runs.

There is intentionally no GraphQL task-creation mutation in this slice; normal
creation is the audited primary-agent `task.delegate` tool path.

### Subscriptions

- `taskUpdated(taskId: ID!): TaskUpdate!` — compact status/progress/delivery
  updates for a marker and open detail panel.

Back subscriptions with a runtime event hub shared by the task coordinator and
GraphQL. Durable SQLite reads remain authoritative; broadcasts are only a live
delivery optimization.

### Transcript integration

Add `task_reference` to `conversation_items` and the typed GraphQL transcript
union. Its durable payload contains task id and creation-time display snapshot;
GraphQL enriches replay with current task status from the canonical task row in
a batched lookup. Live task events update the same frontend entry without
rewriting historical conversation content.

## Frontend Experience

### Chat marker

Add `TaskReferenceCard` beside the existing artifact reference card. It shows:

- task title;
- compact status (`Queued`, `Working`, `Reviewing`, `Revising`, `Needs you`,
  `Complete`, `Failed`, or `Cancelled`);
- a short progress line such as `Executor · revision 2`;
- an affordance to open the existing chat detail rail.

The card is a durable transcript item, not an ephemeral activity row. It must
remain clickable after restart and must not make the chat composer pending
while work continues.

### Task detail rail

Extend `ChatDetailTarget` to the discriminated union:

```ts
type ChatDetailTarget =
  | { type: "artifact"; version: string }
  | { type: "task"; taskId: string };
```

Add `TaskDetailPanel` with a single scroll surface and restrained sections:

1. Status, creation provenance, complexity, current stage, and cancel/retry
   action when valid.
2. Original request and ordered validation criteria.
3. Executor/reviewer model snapshots, including reasoning effort and inherited
   reviewer fallback state.
4. Revision timeline. Each cycle groups executor output, safe tool activity,
   submission/evidence, reviewer verdict, and criterion feedback.
5. Approved result and task-owned artifacts.
6. Delivery state and safe failure/recovery information.

Load compact detail first and page run items only when a cycle is expanded.
Render Markdown with the existing safe artifact Markdown path. Do not render
raw HTML, raw provider payloads, hidden reasoning, secrets, or unrestricted
tool arguments.

On narrow viewports, retain the rail's existing full-width overlay behavior.
On desktop, retain its resizable side-by-side behavior. No new `/tasks` route is
required.

### Settings

Keep configuration under `/settings/agents` for this slice:

- Primary agent card: current behavior.
- Task review agent card: the shared `ModelPreferenceSelect`, showing "Uses
  primary model" until an explicit preference is saved.
- Task executor pools: three clearly labeled tier sections. Each entry selects
  provider account, model/profile, required reasoning effort, optional label,
  enabled state, and ordering. Explain that the primary selects one entry from
  the chosen tier per delegated task.

If no enabled executor entry exists, show that background delegation is off and
do not advertise `task.delegate` to the primary model. Partial pool
configuration is valid; only configured tiers are available.

## Milestones

Each milestone is one coherent implementation unit and should end with a
focused commit. Before each commit, inspect `git status --short --branch`, run
`git diff --check`, and inspect staged stat/name-status. Preserve unrelated
worktree changes.

### Milestone 1 — Domain, schema, repository, and model configuration

**Goal:** Establish the concrete task/run vocabulary and human-controlled model
configuration without starting background work.

**Primary files**

- Create `crates/noema-core/src/task.rs` and focused modules under
  `crates/noema-core/src/task/` for ids, states, model snapshots, criteria, and
  transition validation.
- Modify `crates/noema-core/src/objects.rs`, `ids.rs`, and `lib.rs` for concrete
  `Task` and `AgentRun` references.
- Modify `crates/noema-core/src/store/schema.rs`.
- Create `crates/noema-core/src/store/tasks.rs`, `task_model_pools.rs`,
  `agent_runs.rs`, and `task_events.rs`; expose them narrowly from `store.rs`.
- Modify `crates/noema-core/src/store/agents.rs` and
  `agent_runtime_preferences.rs` for explicit built-in roles.
- Create a shared provider-account-aware model config resolver under
  `crates/noema-core/src/provider/` and route existing primary preference
  resolution through it.

**Work**

- Add schema tables, constraints, indexes, and transaction helpers described
  above.
- Seed executor/reviewer identities without giving the executor a fixed model.
- Add store APIs for pools, atomic task creation, run claims/leases, transitions,
  submissions, reviews, and event append.
- Centralize model preference validation now duplicated by agent/auxiliary
  settings and ensure runtime resolution honors provider account id.
- Add repository unit tests for invalid transitions, immutable criteria,
  snapshot stability, pool membership, all-pass review gating, monotonic event
  sequencing, atomic task creation, lease contention, and expired lease
  recovery.

**Acceptance**

- A store test can create a task and initial executor run atomically.
- Two claimers cannot lease the same run.
- No repository caller can mark a task complete without a valid all-pass review.
- Changing/deleting Settings records cannot alter an existing task's snapshots.
- Existing primary-agent model selection still behaves the same while honoring
  its persisted provider account.

**Suggested commit:** `feat(tasks): add durable task and run model`

### Milestone 2 — Shared agent execution boundary

**Goal:** Make the existing model/tool continuation engine reusable without
changing foreground chat behavior.

**Primary files**

- Create `crates/noema-core/src/agent_execution.rs` and focused modules under
  `crates/noema-core/src/agent_execution/`.
- Refactor `crates/noema-core/src/daemon/runtime/turn.rs`, `model_tools.rs`,
  `local_tools.rs`, `tool_lifecycle.rs`, `progress.rs`, and
  `progress_audit.rs` into or behind the shared boundary.
- Modify artifact and memory tool runtime contexts to accept typed execution
  owner/source state.

**Work**

- Add characterization tests around current provider continuation, native tool
  replay, progress audits, cancellation, transcript persistence, and tool
  result correlation.
- Extract provider loop, tool-policy construction, and event-sink contract.
- Implement the conversation adapter and prove current GraphQL chat behavior
  still uses it.
- Generalize task-owned local artifact paths and artifact authorization while
  preserving conversation artifact behavior and symlink/path protections.
- Keep all role-specific policy inputs explicit; do not branch on prompt text.

**Acceptance**

- Existing conversation/runtime unit tests pass unchanged or with mechanical
  adapter updates.
- A synthetic non-conversation execution can run a fake provider through a
  read-only tool continuation and persist events to a test sink.
- Task-owned artifact writes are idempotent by run/provider call id.
- The old turn loop has one implementation path; no copied task executor loop
  exists.

**Suggested commit:** `refactor(runtime): extract reusable agent execution`

### Milestone 3 — Primary delegation and durable executor

**Goal:** Let the primary create a task and let a supervised background worker
produce a structured submission.

**Primary files**

- Create `crates/noema-core/src/task_runtime.rs` and modules for coordinator,
  worker, context, prompts, tools, and recovery.
- Modify `crates/noema-core/src/runtime_host.rs` to own `TaskRuntimeHandle`.
- Add the task delegation/submission tools to the role-aware model tool builder.
- Modify conversation transcript persistence for typed task references.

**Work**

- Build dynamic `task.delegate` schema/prompt metadata from enabled pool rows.
- Enforce exclusive task delegation and terminal no-tools acknowledgement.
- Commit task, criteria, snapshots, task event, executor run, and transcript
  reference in one transaction; signal the coordinator only after commit.
- Implement queue claiming, leases, heartbeat, cancellation, shutdown joining,
  restart recovery, and bounded concurrency.
- Assemble bounded source context and persist its inclusion/omission manifest.
- Run the executor with its role-specific tool policy and terminal
  `task.submit_result` contract.
- Persist submissions, criterion evidence, task-owned artifacts, run items, and
  safe run events; enqueue review but do not implement review execution yet.

**Acceptance**

- A fake primary provider delegates a task and its foreground turn completes
  without waiting for executor work.
- A queued executor runs with the selected exact pool snapshot and commits one
  submission.
- No pool configuration means no `task.delegate` tool exposure.
- A mismatched tier/entry or mixed delegation tool batch creates no task.
- Cancellation and process shutdown leave recoverable durable state.

**Suggested commit:** `feat(tasks): delegate work to durable executor`

### Milestone 4 — Adversarial review and revision loop

**Goal:** Validate submissions independently and close the executor/reviewer
feedback loop.

**Primary files**

- Add reviewer and artifact-read modules under `task_runtime/`.
- Add strict review contracts and transition application under `task/` and
  `store/`.
- Reuse the agent model preference GraphQL/domain validation for the reviewer
  identity; UI arrives later.

**Work**

- Resolve and snapshot the reviewer config during task creation.
- Build bounded reviewer context from the immutable task, submission, artifact
  manifest, and safe run evidence.
- Enforce reviewer read-only tool policy and `task.submit_review` terminal tool.
- Validate exact criterion coverage and all-pass approval deterministically.
- On `request_changes`, create a new executor run whose context includes the
  prior submission and actionable reviewer feedback.
- Separate transient execution retries from product revision rounds.
- Move uncertain/exhausted/approval/clarification cases to
  `waiting_for_human`; never auto-approve on reviewer failure.

**Acceptance**

- A failed criterion schedules a revision with unchanged original criteria.
- A subsequent all-pass review completes the task.
- Missing/duplicate/uncertain criterion results cannot complete the task.
- Reviewer attempts cannot access mutation, artifact creation, delegation, or
  write/export tools.
- The fourth requested submission does not run when the default three-round
  bound is exhausted.

**Suggested commit:** `feat(tasks): add adversarial review loop`

### Milestone 5 — Completion delivery and GraphQL API

**Goal:** Notify the primary conversation durably and expose complete task
inspection/control APIs.

**Primary files**

- Add a completion-delivery command/adapter to the conversation runtime.
- Create `crates/noema-core/src/graphql/tasks.rs` and focused resolver tests.
- Modify `graphql.rs`, `graphql/runtime_state.rs`, `graphql/subscriptions.rs`,
  and thin root wiring in `graphql/schema.rs`.
- Extend GraphQL replay mapping for `task_reference`.

**Work**

- Enqueue delivery atomically with approval and serialize it through the source
  conversation actor.
- Generate a no-tools primary response, append deterministic artifact
  references, retry safely, and provide a deterministic fallback notice.
- Add task queries, paged run items, Settings query/mutations, cancel/retry, and
  task subscription.
- Batch task-reference hydration during transcript replay to avoid N+1 reads.
- Apply server-derived local-human authorization to task reads/actions and
  task-owned artifact downloads.
- Generate frontend GraphQL schema/types after backend contracts stabilize.

**Acceptance**

- Approved work produces exactly one durable completion delivery despite
  retries/restart.
- Completion waits behind an active source-conversation turn and never
  interleaves transcript sequence indexes.
- Offline completion appears on later transcript replay.
- Unauthorized/wrong-owner task and artifact reads fail without leaking
  existence.
- Task subscriptions can be missed without losing canonical state.

**Suggested commit:** `feat(tasks): deliver reviewed results to chat`

### Milestone 6 — Settings, chat marker, and task detail rail

**Goal:** Make configuration and execution history clear, compact, and
auditable in the current chat-led UI.

**Primary files**

- Modify `web/src/components/settings/AgentsSettingsPane.tsx` and
  `AgentsSettingsPaneContent.tsx`; add focused task-agent pool components.
- Modify `web/src/transcript/events.ts`, shared transcript types, render model,
  and transcript rendering switches for `task_reference`.
- Create `web/src/components/transcript/TaskReferenceCard.tsx`.
- Extend `web/src/components/chatDetail/chatDetailTypes.ts` and
  `ChatDetailRail.tsx`.
- Create focused components under
  `web/src/components/chatDetail/task/` for overview, criteria, run cycle,
  result/artifacts, delivery, and actions.
- Add GraphQL operations and regenerate `generated/schema.graphql` and
  `generated/graphql.ts`.

**Work**

- Add reviewer model and three executor pool settings using shared model
  controls and validation metadata.
- Render durable task cards and subscribe to compact status updates.
- Add task target handling without disturbing artifact version navigation.
- Load compact task detail, lazily page expanded run cycles, and render safe
  Markdown/output/artifact links.
- Keep status semantics, failure reasons, model snapshots, revisions, and
  reviewer criterion feedback visible without exposing unsafe raw internals.
- Add cancel/retry actions only in valid states and reflect optimistic state
  conservatively until the canonical update arrives.

**Acceptance**

- A human can configure one or more models in each desired tier and a separate
  reviewer model.
- The chat marker updates through every workflow stage without holding the
  composer pending.
- Clicking a task marker opens the correct task in the existing detail rail;
  artifact detail still works.
- Executor output, reviewer feedback, criterion evidence, model snapshots,
  artifacts, errors, and delivery state are inspectable after restart.
- Loading a large run does not require embedding its entire item stream in the
  initial transcript or task-detail query.

**Suggested commit:** `feat(web): add auditable background task experience`

### Milestone 7 — Recovery, security, validation, and durable context

**Goal:** Close failure modes, validate the full slice, and document the landed
architecture.

**Work**

- Exercise unit-level crash/restart boundaries: after task commit, after lease,
  after tool-result persistence, after submission, after review, and before/after
  delivery persistence.
- Verify idempotency for task creation tool call id, run claims, tool call ids,
  submissions, reviews, and completion delivery.
- Verify task cancellation at queued, running, reviewing, and delivery stages.
- Audit egress and permissions: inherited source authority, reviewer read-only
  policy, artifact ownership, GraphQL principal checks, safe error payloads, and
  no secret/reasoning exposure.
- Inspect every touched source file over 750 lines and keep new task/runtime/UI
  modules focused rather than expanding existing monoliths.
- Update `docs/project.md`, relevant frontend/harness docs, and
  `docs/context/current.md` with only the architecture that actually landed.
- Run a read-only adversarial review before the final correction commit.

**Suggested commit:** `docs(tasks): record background task architecture`

## Validation Strategy

Follow the project rule to run unit tests only. Do not add frontend/UI tests,
run smoke or fixture tests, or use browser inspection unless separately
requested.

### Focused Rust validation during milestones

```bash
cargo fmt --all --check
cargo check -p noema-core
cargo clippy -p noema-core --all-targets -- -D warnings
cargo test -p noema-core task --no-fail-fast
cargo test -p noema-core agent_execution --no-fail-fast
cargo test -p noema-core graphql --no-fail-fast
```

Use narrower test filters while iterating, but end each backend milestone with
the package-level unit tests relevant to its change. Never alter or bypass the
configured `sccache`/`CARGO_BUILD_RUSTC_WRAPPER` setup.

### Frontend static validation

```bash
cd crates/noema-core/web
bun run gen:types
bun run gen:routes
bun run lint
bun run build
```

### Final gate

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
git diff --check
```

Before each commit also inspect:

```bash
git status --short --branch
git diff --cached --stat
git diff --cached --name-status
```

## Test Matrix

| Area | Required unit evidence |
| --- | --- |
| Model pools | Tier membership, account/profile/effort validation, enable/disable, referenced-entry retention |
| Task creation | Atomic rows/events/run/reference, trusted source ids, call-id idempotency, exclusive tool batch |
| State machine | Every allowed transition and rejection of every invalid/terminal transition |
| Queue | Single lease winner, heartbeat/token enforcement, expired recovery, cancellation, shutdown join |
| Executor | Exact snapshot used, bounded context, role tool policy, structured submission, artifact ownership |
| Reviewer | Read-only tools, exact criterion coverage, fail-closed parsing, revisions, three-round bound |
| Recovery | Restart after each durable boundary, replay of safe/idempotent steps, ambiguous effect waits |
| Delivery | Conversation ordering, exactly-once durable item, offline replay, retry/fallback behavior |
| GraphQL | Local principal ownership, safe task detail, pagination bounds, settings mutations, subscription update |
| Redaction | No secrets, credentials, hidden reasoning, raw provider payloads, or unsafe tool args in GraphQL |
| Regression | Existing chat, tool loop, artifacts, model preferences, transcript replay, and shutdown behavior |

## Operational Invariants

- SQLite is the only source of truth for task/run state; in-memory maps,
  broadcasts, and `Notify` are accelerators only.
- Every meaningful task/run transition has one transaction containing both the
  current-state write and its append-only event.
- A lease token is required to mutate a leased run.
- Task completion requires one immutable submission and one structurally valid
  all-pass review of that exact submission.
- The reviewer cannot modify executor output or validation criteria.
- No task model can recursively delegate.
- Background execution never broadens the source human/conversation authority.
- A Settings change never rewrites an active or historical model snapshot.
- A process restart cannot duplicate task creation, a persisted tool result, a
  submission, a review, or a completion message.
- Missing live subscribers cannot strand work or lose a notification.
- Raw chain-of-thought and secret-bearing provider/tool data are never part of
  the normal task inspection API.

## Risks and Mitigations

| Risk | Mitigation |
| --- | --- |
| Duplicating the conversation tool loop | Extract a shared execution engine first and retain conversation characterization tests |
| Background work blocks chat | Coordinator is outside the serial actor; only final delivery enters the actor queue |
| Reviewer rubber-stamps output | Adversarial prompt, independent model snapshot, per-criterion evidence, and deterministic all-pass gate |
| Infinite executor/reviewer churn | Three reviewed submissions, separate infrastructure retries, then `waiting_for_human` |
| Wrong model/account runs | Account-aware runtime registry plus immutable validated snapshots |
| Restart repeats a side effect | Durable run items/call ids, idempotent local writes, safe retry classification, ambiguous effects stop |
| Audit UI leaks sensitive data | Separate replay/internal payload from safe display projection; GraphQL redaction tests |
| Task marker becomes stale | Canonical task hydration on replay plus live task subscriptions |
| Completion races a human turn | Deliver through the existing source conversation command queue |
| Premature project/workspace abstraction | Concrete source conversation/human links and task scope behavior; add explicit future links later |
| Existing large modules grow further | New focused store/runtime/GraphQL/UI modules and thin wiring only |

## Definition of Done

The slice is done only when all of the following hold:

- The primary agent can delegate a configured, bounded request and immediately
  return control to the human.
- The selected executor model is one exact human-enabled entry from the stated
  complexity pool and is durably auditable.
- The executor can use governed tools, create task-owned artifacts, and submit
  structured criterion evidence in the background.
- A separately configurable reviewer independently approves or requests
  changes, and no failed/uncertain/missing criterion can complete the task.
- Revision, retry, cancellation, shutdown, and restart behavior is bounded and
  durable.
- The primary conversation receives exactly one completion report with the
  approved output and artifacts, even when the browser was closed.
- Chat shows a durable task marker whose detail rail exposes request, criteria,
  status, model snapshots, executor output, review output, tools, artifacts,
  failures, and delivery history safely.
- No global Tasks page, project/workspace system, recursive delegation, or
  unapproved external-write framework was added.
- Focused and full validation passes, the final adversarial review has no open
  critical correctness/security/data-loss finding, and durable project context
  reflects the implementation that actually landed.
