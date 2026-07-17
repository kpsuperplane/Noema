# Implementation packet 02: store and event ledger

## Brief

| Field | Contract |
| --- | --- |
| Mode | Implement. |
| Goal | Rewrite the pre-V1 SQLite task slice around Personal workspace/project rows, one task stage reference, immutable contracts/gates/messages, one global work-event ledger, semantic command transactions, and generation-fenced FIFO runs. |
| Prerequisites | Packet 01 has landed and its public exports are frozen. Read [storage and events](../03-storage-and-events.md) and [commands and reconciliation](../04-commands-and-reconciliation.md) in full. The integrator has added `noema-workspaces` to the root workspace and made its crate dependency available to `noema-store`. |
| Expected commit | `feat(store): persist unified work commands and event ledger` |
| Commit owner | The packet owner hands off an unstaged patch; the integrator creates the focused commit after accepting validation. |

## Exclusive ownership

This agent owns the task/workspace/project portions of `noema-store`, including the schema rewrite and their unit tests. Expected owned paths are:

```text
crates/noema-store/src/schema.rs
crates/noema-store/src/lib.rs                         # only work-related module declarations/exports
crates/noema-store/src/tasks/**
crates/noema-store/src/agent_runs/**
crates/noema-store/src/task_controls.rs
crates/noema-store/src/task_events.rs
crates/noema-store/src/task_reads.rs
crates/noema-store/src/work_commands/**               # new, preferred home
crates/noema-store/src/work_reads/**                  # new, preferred home
crates/noema-store/src/work_events/**                 # new, preferred home
crates/noema-store/src/tests/task_*.rs
crates/noema-store/src/tests/work_*.rs
crates/noema-store/src/tests/schema*.rs               # only work-schema assertions
```

The agent may consolidate or remove obsolete task-only modules instead of preserving a parallel legacy path. It owns no root manifest or lockfile change, even if the new domain crate requires one.

### Forbidden files

It must not edit:

```text
Cargo.toml
Cargo.lock
crates/noema-workspaces/**
crates/noema-tasks/**
crates/noema-runtime/**
crates/noema-api/**
apps/web/**
generated GraphQL/routes
docs/context/current.md
```

The repository is expected to be dirty from unrelated work. Preserve every unrelated change in shared files; if a concurrent edit overlaps the owned work region, stop and send the integrator a narrow conflict report rather than overwriting it.

## Interfaces consumed

From Packet 01, consume the frozen `noema-workspaces` and `noema-tasks` records, IDs, commands, planner input/output, event vocabulary, and safe error types. The store may map them to row adapters, but it must not recreate public enum/string contracts or add a `TaskStatus` compatibility field.

Continue consuming:

- provider selection/model-pool resolution and readiness proof from the existing store/provider boundary;
- artifact metadata/version lookup for immutable submission artifact snapshots;
- existing human, agent, conversation, transcript, provider-account, and runtime-preference tables;
- the existing SQLite retry/transaction helpers and safe `StoreError` boundary.

## Interfaces produced

Expose one coherent work persistence surface. Exact internal type placement is flexible, but runtime/API callers need these operations with typed inputs/outputs:

```rust
impl NoemaStore {
    async fn execute_work_command(
        &self,
        command: WorkCommand,
    ) -> Result<WorkCommandResult, StoreError>;

    async fn get_work_task(&self, task_id: &TaskId) -> Result<Option<WorkTaskDetail>, StoreError>;
    async fn list_work_tasks(&self, query: WorkTaskQuery) -> Result<WorkTaskConnection, StoreError>;
    async fn list_work_projects(&self, workspace_id: &WorkspaceId, query: ProjectQuery)
        -> Result<Vec<ProjectRecord>, StoreError>;

    async fn load_work_reconciliation_snapshot(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<WorkReconciliationSnapshot>, StoreError>;
    async fn apply_work_reconciliation_action(
        &self,
        action: WorkReconciliationAction,
    ) -> Result<WorkCommandResult, StoreError>;

    async fn claim_next_work_run(
        &self,
        worker_id: &str,
        lease_seconds: i64,
    ) -> Result<Option<ClaimedWorkRun>, StoreError>;
    async fn record_work_run_terminal(/* typed planner/executor/reviewer terminal */)
        -> Result<WorkCommandResult, StoreError>;

    async fn list_work_events_after(
        &self,
        query: WorkEventQuery,
    ) -> Result<WorkEventConnection, StoreError>;

    async fn claim_work_notifications(/* worker lease */)
        -> Result<Vec<ClaimedWorkNotification>, StoreError>;
    async fn complete_work_notification(/* notification lease */)
        -> Result<(), StoreError>;
    async fn fail_work_notification(/* notification lease and safe error */)
        -> Result<(), StoreError>;
}
```

`WorkCommandService` may be a dedicated façade held by `NoemaStore` rather than public methods with this exact spelling, but it must remain the sole writer for semantic work changes. `WorkTaskDetail` and list projections must resolve stage, current run, active gate, latest review, and valid actions separately; they must not synthesize a second lifecycle/execution-phase field.

The runtime owns the scheduling loop. The store owns durable facts and atomic application of a pure `WorkReconciliationAction`; it must not start workers, read prompts, inspect model prose, or depend on runtime event broadcasts.

## Implementation checklist

### Schema and bootstrap

- [ ] Replace the current task/run schema segment with the exact V3 tables, checks, indexes, and seeds in [storage and events](../03-storage-and-events.md).
- [ ] Set the schema marker/version to `sqlite_store_v3` / `3`; remove V2 task tables and marker assumptions from bootstrap tests.
- [ ] Enable and test SQLite foreign-key enforcement on every store connection.
- [ ] Add Personal workspace/membership/default workflow/seven stage seeds after default human seeding. Seed no project and expose no workspace picker semantics here.
- [ ] Preserve existing generic provider/account/agent/artifact/conversation tables and references. Rewrite only the pre-V1 work slice; no migration or compatibility read path.
- [ ] Remove `tasks.status`, task-level model/complexity fields, `task_events`, `run_events`, and run priority persistence. Every work run has equal scheduling weight and FIFO claim order is `queued_at`, then `run_id`.

### Row adapters and reads

- [ ] Add row adapters for workspace, project, workflow, stage, task, immutable contract/criteria, gate, message, run, submission, review, work event, notification, and command receipt.
- [ ] Validate all persisted closed vocabularies through Packet 01 types. An unknown stage behavior, event kind, gate state, run kind/status, or verdict becomes a typed safe store invariant/error rather than a fallback string.
- [ ] Implement bounded board/list/detail reads with joins or batch loaders. A board card query must not issue one run/review/gate query per task.
- [ ] Implement exact `WorkEventCursor` encoding/decoding, exclusive `after` semantics, default 50/max 100 page size, workspace/task/project/run filtering, and ascending global event order.
- [ ] Keep task detail history immutable and cursor-backed. Existing task transcript/artifact reads should resolve through the new contract/run relationships, not old status event tables.

### Semantic command transactions

- [ ] Implement receipt-first idempotency lookup using actor, command name, key, and canonical SHA-256 request fingerprint. Persist receipt/result/event cursor in the same command transaction.
- [ ] Implement `CaptureTask`, `UpdateInboxTask`, `QueueTask`, `AnswerTask`, `RetryTask`, `AcceptTask`, `RequestTaskChanges`, `CancelTask`, and `ReopenTask` exactly as specified in [commands and reconciliation](../04-commands-and-reconciliation.md).
- [ ] Implement `CreateProject`, `UpdateProject`, `ArchiveProject`, and `ReopenProject` with revision fences and no task/run side effects.
- [ ] Implement tool-only `DelegateTask` as one transaction creating Queue task, events, receipt, and creation notification. With no complete intent, queue Planner and create no contract; with complete intent, create the immutable contract/criteria and queue Executor. Enforce source conversation/tool-call uniqueness.
- [ ] Have every successful command update projection(s), append global event(s), enqueue any required notification, and save its receipt atomically. No public generic stage setter is permitted.
- [ ] Resolve fresh model selections/policy snapshots only when creating a contract/run. Verify readiness in the transaction using the existing provider selection machinery; do not let later settings mutations rewrite historical snapshots.
- [ ] Enforce Inbox-only capture-field edits. Use task messages and new immutable contracts for all later human input.

### Runs, terminal evidence, and fences

- [ ] Add `Planner` run kind, `task_generation`, optional `contract_id`, review rounds, and all existing immutable model/policy/usage/lease fields to `agent_runs`.
- [ ] Enforce Planner-without-contract and Executor/Reviewer-with-contract at insert/read boundaries. Planner uses the executor identity but has a separately snapshotted model request.
- [ ] Implement one runnable run per task with the partial unique index. Claim only Queue first-dispatch runs or Doing child runs; move Queue to Doing atomically, leave Doing unchanged for child work.
- [ ] Remove priority input/ordering from work runs. Claim FIFO by `queued_at ASC, run_id ASC` across the global supervisor limit.
- [ ] Fence all terminal, heartbeat, submission, review, and cancellation writes by lease token, run status, cancellation flag, run task generation, and current task generation.
- [ ] Rework submissions/reviews to link the contract and contract criteria. Preserve exact criterion coverage, immutable artifact-version links, and idempotent same-payload terminal replay.
- [ ] Implement Planner complete/blocking terminals, Executor submission, Reviewer approve/request-changes/needs-human terminals, retryable failure, exhausted failure, lease expiry, and cancellation according to the command spec.

### Events, notifications, and reconciliation persistence

- [ ] Replace per-task/per-run append helpers with one transactional `append_work_event_tx` that receives scope IDs, causal metadata, a validated event kind, and safe JSON payload.
- [ ] Ensure event sequence allocation is global and monotonic from the database, never computed with `MAX(...) + 1`.
- [ ] Implement generalized notification outbox claiming, lease expiry, failure/retry, and deterministic conversation-item delivery identity. Outbox retry may produce delivery events but may not duplicate a chat card.
- [ ] Address V1 notification rows to the Personal workspace's sole owner and
      resolve `humans.primary_conversation_id` at delivery; never use task
      source provenance as the destination.
- [ ] Implement `load_work_reconciliation_snapshot` from durable rows only, and `apply_work_reconciliation_action` using unique/index predicates. Do not put process inspection or English-output parsing in the store.
- [ ] On startup/lease recovery, transition expired leased/running runs to Interrupted, then apply the same pure recovery action as an ordinary failure. Do not reuse old task status recovery paths.
- [ ] Delete obsolete task completion delivery queries once their semantic equivalent is covered by the notification outbox.

## Focused store tests

Add unit tests inside `noema-store` only. They should use the existing in-memory/temporary SQLite test support and prove durable behavior, not runtime worker behavior.

1. **Bootstrap:** creates one Personal workspace, owner membership, default workflow, exactly seven stages, and the V3 marker; a second bootstrap is idempotent.
2. **Schema integrity:** foreign keys, stage/workflow composite reference, active-project validation, contract criteria uniqueness, one open gate, one runnable run, and no `tasks.status`/legacy event tables.
3. **Stage commands:** every allowed command transition commits the expected stage and event rows; arbitrary raw stage mutation and all disallowed starts fail safely.
4. **Optimistic concurrency:** stale task revision/generation and stale project revision leave projections and events unchanged; exact idempotent replay returns the original cursor; divergent replay fails.
5. **Inbox edit boundary:** direct edits work in Inbox and fail after Queue; Answer/Retry/RequestChanges create messages/contracts instead of editing capture fields.
6. **FIFO dispatch:** equal-weight runs claim in `queued_at, run_id` order, including a deterministic tie; no task priority column/API remains.
7. **Contract immutability:** complete delegated/planned/revised contracts snapshot model/policy/context once, Planned requires an execution plan, criteria are exact, and Request Changes creates a new version/generation without altering history.
8. **Gate behavior:** one open gate, Answer and Retry resolve the right kind, Approval gates require a structured approved/declined decision, no gate can be resolved after cancellation/reopen, and waiting attention reads are bounded.
9. **Evidence:** executor/reviewer terminals reject wrong run kind, stale generation, lease loss, missing/duplicate criteria, mismatched contract, foreign artifact, and malformed approval; same terminal replay is safe, and a post-gate Reviewer writes a linked later attempt for the same submission.
10. **Review/recovery:** approval reaches Review but not Completed; request changes queues another Executor while within bounds; exhausted reviews and failed retries open Recovery/Waiting.
11. **Cancellation/reopen:** queued runs cancel, running runs are fenced, old terminal writes cannot revive a task, and reopen preserves history while clearing current pointers and requiring Inbox/Queue again.
12. **Event ledger:** every command/event pair commits atomically; global cursors are ordered, filtered, bounded, and replayable across tasks; no per-task sequence is used as a subscription cursor.
13. **Outbox:** required card events enqueue exactly once; leased/delivered/failed retry handling is idempotent by notification/destination; duplicate delivery cannot duplicate the deterministic conversation item.
14. **Reconciliation:** each documented durable snapshot produces exactly one next action or intentional idle, and applying it twice leaves one run/gate/event outcome.

Do not add runtime, API, browser, smoke, or fixture tests in this packet. Report any missing prompt/tool behavior to the runtime owner.

## Required validation

Run focused store tests while iterating. Before the packet commit, run the project standard suite when all prerequisite commits are present:

```text
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

The agent must not disable, bypass, or reconfigure the shared Rust compiler cache/wrapper. If a full validation failure belongs to a concurrent unowned change, report its command, failure boundary, and unaffected focused test result to the integrator.

## Handoff checklist

- [ ] List changed and removed store files, including obsolete V2 task-event/status paths.
- [ ] State the exact schema marker, table/index changes, seed behavior, and whether `PRAGMA foreign_keys` is covered by tests.
- [ ] List public store methods/types consumed by runtime/API and their ownership boundary.
- [ ] Report the event cursor format, idempotency receipt key, FIFO query, and run-generation fencing predicate implemented.
- [ ] Attach focused tests and full validation output, including any external failure that remains outside this packet.
- [ ] State every implementation assumption, or explicitly report `none`.
- [ ] Confirm no root manifest/lockfile/domain/runtime/API/web/generated/context file was edited.
- [ ] Surface unresolved cross-crate changes as precise signature/schema requests to the integrator; do not leave temporary legacy status/event code behind.
