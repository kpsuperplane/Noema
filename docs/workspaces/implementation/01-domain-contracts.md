# Implementation packet 01: domain contracts

## Brief

| Field | Contract |
| --- | --- |
| Mode | Implement. |
| Goal | Create the workspace/project and unified work-domain contracts that replace task-level `TaskStatus` with one workflow-stage reference while preserving run-local lease state and immutable task evidence. |
| Prerequisites | Read [the domain model](../02-domain-model.md), [storage and events](../03-storage-and-events.md), and [commands and reconciliation](../04-commands-and-reconciliation.md) in full. The integrator must reserve root workspace-manifest edits and generated files. |
| Expected commit | `feat(tasks): define unified work domain contracts` |
| Commit owner | The packet owner hands off an unstaged patch; the integrator creates the focused commit after accepting validation. |

## Exclusive ownership

This agent owns only these areas:

```text
crates/noema-workspaces/Cargo.toml
crates/noema-workspaces/src/**
crates/noema-tasks/Cargo.toml
crates/noema-tasks/src/**
```

It may add focused unit tests under those directories and update Rust module declarations within them. It must keep source files below the project’s size threshold by splitting records, IDs, commands, and pure planners into separate modules.

### Forbidden files

It must not edit:

```text
Cargo.toml
Cargo.lock
crates/noema-store/**
crates/noema-runtime/**
crates/noema-api/**
apps/web/**
generated GraphQL or route files
docs/context/current.md
```

If adding `noema-workspaces` needs a root workspace member or any sibling crate dependency wiring, report that exact diff as an integration request. Do not make a temporary forwarding type, duplicate ID enum, compatibility shim, or handwritten mirror to compile around the boundary.

## Interfaces consumed

- `noema-providers::ProviderSelectionSnapshot` and provider-selection validation types remain the source of immutable model snapshots.
- `noema-artifacts` remains the source of artifact and artifact-version records referenced by immutable submissions.
- Existing `HumanId`, agent IDs, conversation IDs, turn IDs, and transcript item IDs retain their current semantic owners. Use validated task-domain wrappers at the new work boundary; do not create a global ID module.
- The store agent will consume the exact public exports from `noema-workspaces` and `noema-tasks`; no store trait is introduced here.

## Interfaces produced

### `noema-workspaces`

Export these validated types and records from `lib.rs`:

```rust
WorkspaceId, ProjectId, WorkspaceRole,
WorkspaceRecord, WorkspaceMembership, ProjectRecord,
WorkspaceInputError
```

`WorkspaceId` and `ProjectId` validate the semantic prefix, nonblank value, maximum byte length, and no control characters. `WorkspaceRecord` and `ProjectRecord` are data contracts only; they do not contain agent, memory, capability, task-stage, or storage logic.

Suggested module split:

```text
src/
  error.rs
  ids.rs
  project.rs
  workspace.rs
  lib.rs
```

### `noema-tasks`

Export the complete contracts described in [the domain model](../02-domain-model.md):

```rust
WorkflowId, WorkflowStageId, TaskId, TaskContractId, TaskGateId, TaskMessageId, WorkEventId,
WorkflowDefinition, WorkflowStage, WorkflowStageBehavior,
TaskRecord, TaskProvenance, TaskSourceKind,
TaskExecutionContract, ContractOrigin, WorkspaceContextSnapshot, ProjectContextSnapshot,
TaskGateKind, TaskGateState, TaskRecoveryReason, ApprovalDecision, TaskGateAnswer, TaskGateRecord,
TaskMessageKind, TaskMessageRecord,
TaskComplexity, TaskExecutionPolicy,
RunKind, RunStatus, AgentRunRecord,
TaskValidationCriterion, TaskSubmissionRecord, TaskReviewRecord,
CommandMeta, TaskPrecondition, ProjectPrecondition,
CaptureTask, UpdateInboxTask, QueueTask, AnswerTask, RetryTask, AcceptTask,
RequestTaskChanges, CancelTask, ReopenTask,
CreateProject, UpdateProject, ArchiveProject, ReopenProject,
WorkCommand, WorkCommandResult, WorkEventKind, WorkEventRecord,
WorkDomainError,
WorkReconciliationSnapshot, WorkReconciliationAction,
plan_work_transition, plan_reconciliation_action
```

The exact module names may differ, but the public names and field semantics must not. New types must derive the serialization/equality traits required by current provider/tool/API boundaries and parse closed persisted vocabularies fail-closed.

`RunKind` gains `Planner`; `RunStatus` remains run-local. `TaskStatus` is removed from public exports and storage vocabulary. Do not replace it with a renamed task phase enum. Task current state is the resolved `WorkflowStage` referenced by `TaskRecord.stage_id`.

Suggested `noema-tasks` split:

```text
src/
  command.rs          # command inputs, common metadata, results
  contract.rs         # immutable execution contract and snapshots
  error.rs
  event.rs            # WorkEventKind and safe event record
  gate.rs
  ids.rs
  planning.rs         # pure transition/reconciliation planners
  run.rs
  state.rs            # TaskComplexity only; no task status
  task.rs
  workflow.rs
  ...existing criteria/review/submission/transcript modules...
```

## Implementation checklist

- [ ] Scaffold `noema-workspaces` with no dependency on store, runtime, API, or web crates. Add only the smallest serde/error dependencies that its owned types require.
- [ ] Implement workspace/project validated opaque IDs and exact error variants. Validate the documented prefixes rather than inferring identity from a display name.
- [ ] Add workspace/project record, membership role, archive/revision contracts, and unit tests for normalization and invalid values.
- [ ] Add task-owned opaque IDs, `WorkflowDefinition`, `WorkflowStage`, and the seven-value `WorkflowStageBehavior`. Resolve behavior from a stage record; do not encode an English label as logic.
- [ ] Replace `TaskStatus` and its `can_transition_to` helper. Rework callers inside `noema-tasks` to accept resolved stage behavior/stable IDs and command-specific planning input.
- [ ] Rework `TaskRecord` to contain workspace/project/workflow/stage, capture fields, generation/revision, evidence pointers, and timestamps only. Remove task-level complexity, model snapshots, execution status, blocked-text fields, and terminal-error fields.
- [ ] Add immutable execution-contract records, complete criterion records, contract amendments, context snapshots, and the policy fields for review rounds/retries. A contract must have no mutable “draft” state.
- [ ] Add gates and durable human message contracts. Gate state may be open/resolved/superseded, but it must not be copied into a second task state field.
- [ ] Add `Planner` to `RunKind`, `task_generation` and optional `contract_id` to run contracts, and preserve existing lease/usage/transcript fields. Enforce Planner-without-contract and Executor/Reviewer-with-contract in pure validation.
- [ ] Update submission/review contracts to link immutable contract IDs and review rounds. Preserve exact criterion coverage and fail-closed reviewer verdict validation.
- [ ] Define every human command, the tool-only `DelegateTask` composition, command metadata, preconditions, stable results, and safe `WorkDomainError` codes. Keep project commands separate from task generation.
- [ ] Define `WorkEventKind` and safe `WorkEventRecord`; no separate task/run event public type survives.
- [ ] Implement pure command-transition and reconciliation planners. They receive durable facts, return one action or intentional idle, and must not inspect SQLite, process state, tool text, Git state, or model prose.
- [ ] Remove obsolete `TaskStatus`, `TaskEventKind`, `NewTaskEvent`, `NewRunEvent`, old lifecycle-only planners, and their tests rather than retaining aliases for compatibility.
- [ ] Update crate-level documentation and exports, then run rustfmt before handoff.

## Focused unit tests

The domain agent owns unit tests in its two crates. At minimum, add or replace tests that prove:

1. Semantic ID parsing rejects wrong prefixes, blanks, controls, oversize values, and unknown closed enum values.
2. The Personal stage definition resolves all seven behaviors and cannot mix a stage with another workflow.
3. `TaskRecord` has no task status/phase field and starts with generation/revision one.
4. Inbox fields can be updated only through the Inbox transition; Queue/Doing/Waiting/Review/terminal updates fail with `invalid_transition`.
5. A complete execution contract normalizes its immutable request, exact criteria, and context snapshots; Planned origin requires an immutable execution plan, while Delegated/HumanRevision may omit it, and no contract field can be amended in place.
6. Planner has no contract, Executor/Reviewer require one, and all run status parsing remains fail-closed.
7. Every command planner accepts only the matrix in the domain spec, including cancel/reopen generation changes and Review-only accept/change requests.
8. Approving review requires exact all-pass coverage; `request_changes` requires a failure; `needs_human` requires uncertainty and a valid human gate kind.
9. Reconciliation returns exactly one deterministic action for Queue-without-contract, Queue-with-contract, approved review, waiting gate, retryable failure, exhausted failure, and terminal stages.
10. Idempotency metadata and version preconditions are represented by every mutable command without relying on textual intent.

Do not add integration, fixture, smoke, SQLite, runtime, GraphQL, or frontend tests in this packet. Report any behavior that cannot be expressed as a pure domain test to the store/runtime owners instead of reaching across files.

## Required validation

Run focused package tests while iterating, then the project standard suite before the commit when the integration branch is ready:

```text
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Never disable or bypass the configured Rust compiler cache/wrapper. If the full workspace is temporarily unavailable solely because the integrator has not added the new package to the root workspace, run the affected package/unit checks that are available and state the exact pending integration command in the handoff.

## Handoff checklist

- [ ] List every changed file and the resulting commit hash.
- [ ] List every exported public type/function and any removed export, especially `TaskStatus`.
- [ ] State the exact `noema-workspaces` package/dependency edits the integrator must apply at the root.
- [ ] Confirm no store/runtime/API/web/root/generated file was edited.
- [ ] Attach focused and full validation output, or identify the one integration prerequisite blocking full validation.
- [ ] State every implementation assumption, or explicitly report `none`.
- [ ] Identify any contract question as a concrete proposed type/signature change; do not work around it with a duplicate type or compatibility alias.
