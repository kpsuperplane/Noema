# Packet 03: Runtime and Chat Tools

**Mode:** implement
**Goal:** replace the current one-off executor/reviewer task loop with the
Work planner/executor/reviewer runtime, primary-agent command tools, and
event-driven chat delivery described in
[runtime and chat](../05-runtime-and-chat.md).

This packet is deliberately limited to runtime code. It consumes frozen domain
and store contracts; it must not add alternate enums, task writers, store
transactions, GraphQL shapes, or frontend compatibility paths to make isolated
work compile.

## Prerequisites

Before starting, confirm all of the following have landed or been frozen:

- Packet 01 has exported workflow stage, contract, gate, message, command,
  Planner run-kind, and generation-fence domain contracts.
- Packet 02 has frozen the semantic command, run-claim, reconciliation-read,
  event-outbox, and notification-outbox signatures. Runtime implementation may
  proceed alongside Store implementation once those signatures are accepted;
  final validation waits for the Store handoff.
- The integration owner has recorded the exact source/target names in the
  contract-freeze table. If an interface is missing, file a contract-change
  request rather than editing Packet 01 or 02 files.
- Existing provider, capability, artifact, transcript, cancellation, and
  foreground delivery facilities remain available through their current
  runtime-owned interfaces.

## Exclusive ownership

This agent owns Work-specific changes in:

- crates/noema-runtime/src/daemon/task_runtime.rs and its Work-specific
  submodules;
- crates/noema-runtime/src/daemon/task_tool.rs and
  crates/noema-runtime/src/daemon/task_tool/;
- crates/noema-runtime/src/daemon/task_run_context.rs;
- crates/noema-runtime/src/daemon/task_delivery.rs;
- crates/noema-runtime/src/daemon/runtime/background_task/ and
  crates/noema-runtime/src/daemon/runtime/task_* where the code is task-run
  orchestration, continuation, completion, or transcript integration;
- the corresponding noema-runtime unit-test modules;
- task-only prompt and local tool catalogue additions in
  crates/noema-runtime/src/daemon.

This agent may split those files into focused runtime-owned modules when that
keeps them below the source-size threshold. New code belongs under a
Work-specific runtime module rather than another catch-all daemon file.

### Forbidden files

This agent must not edit:

- crates/noema-workspaces, crates/noema-tasks, or crates/noema-store;
- crates/noema-api or any GraphQL SDL/export file;
- apps/web, generated frontend GraphQL types, generated routes, root manifests,
  Cargo.lock, or docs/context/current.md;
- another packet's tests, schema markers, migrations, or generated output.

## Interfaces consumed and produced

### Consumes from the frozen domain/store boundary

The runtime consumes the documented command service and read projections for:

- claiming one runnable Work run in strict FIFO order by queued-at then
  run id, with lease token and current task generation;
- loading a run execution context containing exact task, contract, workspace
  snapshot, optional project snapshot, resolved messages/gates, lineage,
  submission/review evidence, model snapshot, and policy snapshot;
- committing planner plans, executor submissions, reviewer verdicts, blocked
  gates, lease heartbeats, failures, and cancellation-safe terminal outcomes;
- reconciling a task or durable Work queue after a command/restart;
- claiming and acknowledging notification-outbox rows idempotently;
- publishing/replaying committed Work-event invalidations through the existing
  runtime event registry.

The runtime treats these as transactional operations. It may not compose
ad-hoc reads and writes to reproduce a command transaction.

### Produces for downstream packets

This packet delivers:

- Planner, Executor, and Reviewer run dispatch using the new RunKind;
- a bounded role-specific context builder and prompt set;
- a role-gated typed tool catalogue and terminal payload validation;
- primary-agent task/project tool registration and handlers;
- a startup/command-triggered reconciliation coordinator;
- notification-outbox delivery into conversation cards;
- runtime event publication compatible with the GraphQL packet's single
  work-event subscription projection.

The API packet receives only durable store/event data. It must not call runtime
internals to derive task stage, attention, or action availability.

## Implementation checklist

### 1. Replace the old task-status orchestration assumptions

- [ ] Remove runtime branching on the old task-level execution status enum.
      Read the single task stage plus the role/status of the claimed run.
- [ ] Add Planner dispatch alongside Executor and Reviewer. Planner uses the
      task-executor identity and the executor model snapshot already frozen on
      the run.
- [ ] Keep a maximum of one active run for a task and eight supervised runs
      globally. Claim runs in queued-at/run-id FIFO order with no priority axis.
- [ ] Preserve lease heartbeats, cancellation-token propagation, progress
      audit, provider continuation, repetition detection, transcript
      redaction, and failure reporting from the existing background runtime.
- [ ] After a worker returns, assert that its run reached a valid terminal or
      waiting state. A return with an active run is a runtime error, never an
      idle lease-recovery shortcut.

### 2. Implement reconciler integration

- [ ] Start the idempotent reconciler at runtime startup before normal worker
      polling, then invoke it after every relevant command, claimed-run
      recovery, and terminal event.
- [ ] Let the store/domain decision table choose Planner versus Executor,
      reviewer handoff, same-contract automated review rounds, gate opening, retry, completion,
      cancellation, and reopen. The runtime only schedules the returned
      durable work.
- [ ] Ensure an interrupted process can rerun reconciliation any number of
      times without duplicate active runs, duplicate gates, duplicate
      notifications, or changed contract versions.
- [ ] Do not inspect assistant text, artifact content, process tables, Git
      state, or source-language phrases to determine next action.

### 3. Build role context and prompts

- [ ] Replace the current task prompt builder with a bounded context assembler
      in the order defined by the runtime specification: role-appropriate
      task/contract input, workspace/project snapshots, linked evidence,
      resolved messages, bounded lineage, and role instructions.
- [ ] Include workspace and project name/description only through the frozen
      contract snapshot. Do not retrieve project memory/documents or change
      agents, pools, capabilities, or authority because of those fields.
- [ ] Planner receives no general action tools and must either submit a
      complete normalized contract with bounded execution intent or open a
      clarification/approval gate.
- [ ] Executor receives exact criterion ids, prior review feedback, resolved
      answers, and artifact rules. It must submit criterion evidence and
      cannot invent artifact ids.
- [ ] Reviewer receives evidence as data, not instructions; it may inspect
      only the submitted artifact manifest and must assess every criterion.
- [ ] Preserve safe continuation boundaries: answers and human change requests
      reach newly queued child runs, never a live provider continuation.

### 4. Register typed tools and enforce visibility

- [ ] Replace the primary-only old delegation schema with capture, list,
      update, queue, delegate, answer, retry, accept, request-changes, cancel,
      and reopen task tools plus project create/list/update/archive/reopen.
- [ ] Derive actor, source conversation, causation, correlation, and
      idempotency metadata from trusted runtime context, not model arguments.
- [ ] Implement structured foreground/capture/delegate policy in the primary
      prompt/tool contract. The roughly fifteen-second rule is model policy;
      do not add English phrase or prefix matching.
- [ ] Make Delegate atomic in both typed forms: absent complete intent queues
      Planner, while a complete intent freezes contract v1 and queues Executor.
      Never implement delegation as Capture followed by Queue.
- [ ] Make project linking explicit. Reject or omit a project reference that
      was not supplied as an explicit structured id.
- [ ] Register task.submit_plan only for Planner, task.submit_result only for
      Executor, and task.submit_review only for Reviewer.
- [ ] Restrict task.report_blocked by role and gate kind. Planner and Executor
      may open clarification/approval gates; Reviewer uses its typed
      needs-human review verdict.
- [ ] Retain existing governed read-only tools only where role policy allows
      them. Reviewer writes no artifacts or external effects; no background
      role controls a task/project or delegates children.
- [ ] Validate all terminal payload cardinality, criterion coverage, artifact
      ownership, verdict invariants, generation, contract id, and lease fence
      before committing through the store command service.

### 5. Route models and handle unavailable selections

- [ ] Consume the frozen executor/reviewer selection snapshots. Do not select
      a live replacement model at worker execution time.
- [ ] Let Queue-time store/domain logic select the Planner route from the
      configured medium tier, then first-enabled fallback, and prove route
      readiness. Let complete Delegate/Planner contract creation resolve the
      executor/reviewer snapshots from its frozen complexity.
- [ ] Map a later exact-route loss to the documented retry/recovery path; do
      not silently change provider account, profile, or reasoning effort.
- [ ] Persist the Planner's resolved executor-pool selection on its run. Freeze
      separate Executor and Reviewer snapshots only when the complete contract
      is created.
- [ ] Resolve Reviewer from the readiness-proven `agent:task-reviewer`
      preference at contract freeze; never select it from project context or a
      primary-agent tool argument.

### 6. Gates, cancellation, and recovery

- [ ] Open typed clarification/approval gates only through the semantic
      command path, then stop the run cleanly.
- [ ] On reviewer needs-human, include the required question and let the store
      create the gate and move the task to Waiting.
- [ ] On retryable infrastructure failure, request the fenced child run from
      reconciliation. On exhausted recovery, surface a Recovery gate rather
      than leaving an un-actionable task.
- [ ] Pass cancellation to active provider and tool work. Reject late
      terminal writes through lease, generation, and contract comparisons.
- [ ] Keep a cancelling in-memory future inside the global supervisor cap and
      defer a new claim for the same task until that future settles; do not
      turn this transient guard into persisted task state.
- [ ] Reopen starts a new Inbox cycle only after a new generation has been
      committed; it never resumes an old worker.

### 7. Deliver decision-relevant chat cards

- [ ] Generalize the current task completion outbox into the Work notification
      outbox consumer.
- [ ] Deliver only task creation, Waiting gate, Review-ready, recovery, and
      accepted-completion cards to the owning human's global primary
      conversation; provenance never selects the destination.
- [ ] Use deterministic event-and-destination conversation-item ids and
      acknowledgement after durable insertion so redelivery is idempotent.
- [ ] Serialize routine delivery behind an active foreground turn and keep
      Planner/Executor/Reviewer progress inside Work.
- [ ] Publish runtime invalidation after durable transitions; never publish a
      speculative state that did not commit.

## Focused unit tests

Add or update noema-runtime unit tests for:

- Planner receives only its two terminal tools, freezes a complete plan, and
  queues Executor without a stage detour.
- Planner clarification, executor clarification/approval, and reviewer
  needs-human produce the correct gated waiting outcome.
- A human answer to reviewer needs-human resumes Reviewer on the same
  submission, supplies the resolved message at the safe boundary, and submits
  the next linked immutable review attempt rather than overwriting the first.
- Executor submission queues Reviewer; automated requested changes queue a
  same-contract Executor review round while task stage remains Doing; reviewer
  approval reaches Review rather than Completed.
- Terminal payload validators reject missing/duplicate criterion evidence,
  foreign artifacts, invalid reviewer verdict combinations, and ordinary-text
  completion.
- Role context includes bounded workspace/project snapshots and safe messages
  without enabling scoped memory/capability authority.
- Primary tool registration has no phrase matcher, keeps project linkage
  explicit, and derives trusted call metadata.
- Worker claim honors queued-at/run-id order and preserves the
  one-active-run task fence.
- Cancellation/reopen and stale terminal completions cannot revive old
  generations or produce a notification.
- Restart/reconciliation and notification redelivery are idempotent.

Use unit tests only. Do not add smoke tests, fixture suites, browser tests, or
provider-network dependencies.

## Validation

Run the focused runtime tests while developing, then before handoff run:

1. cargo fmt --all --check
2. cargo check --workspace
3. cargo clippy --workspace --all-targets -- -D warnings
4. cargo test --workspace --no-fail-fast

Do not disable or alter the configured compiler wrapper or shared build cache.
Record exact commands and any pre-existing unrelated failure in the handoff.

## Expected commit

The integrator will serialize commits. Prepare this packet as:

**feat(runtime): orchestrate Work planner, executor, and reviewer runs**

Do not create the commit yourself unless the integrator explicitly transfers
that responsibility.

## Handoff checklist

- [ ] List every changed runtime file and every removed obsolete task-runtime
      path.
- [ ] State which frozen store/domain interfaces were consumed and whether any
      contract-change request remains open.
- [ ] Confirm the old task execution-status branching is gone from runtime
      ownership.
- [ ] Report focused-test and full-validation results.
- [ ] State every implementation assumption, or explicitly report `none`.
- [ ] Report any intentionally deferred integration compile errors by exact
      symbol, without patching another packet's files.
- [ ] Hand off a concise description of tool schemas, event publication, and
      notification behavior needed by API and UI integration.
