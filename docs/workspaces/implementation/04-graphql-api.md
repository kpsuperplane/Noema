# Packet 04: Work GraphQL API

**Mode:** implement
**Goal:** expose the Work read model, semantic command mutations, safe errors,
and cursor-backed work-event subscriptions defined in
[the GraphQL contract](../06-graphql-contract.md).

This packet turns frozen domain/store data into a bounded client contract. It
does not own task transition logic, schema storage, runtime scheduling, or
frontend generated files.

## Prerequisites

Before starting, verify:

- Packet 01 has frozen all exported Work domain types, stage behaviors,
  command variants, event kinds, gate/message records, and valid-action
  derivation inputs.
- Packet 02 has frozen its Work command, bounded query, cursor, and event
  interfaces. GraphQL type/schema work may proceed alongside Store; resolver
  completion and validation wait for the Store handoff.
- Runtime event invalidation has a frozen subscription-registry interface, but
  GraphQL can always backfill from durable store events after a wakeup.
- The integrator owns generated schema/operation files and will regenerate
  them only after this packet's Rust API is accepted.

## Exclusive ownership

This agent owns Work-specific changes in:

- crates/noema-api/src/graphql/tasks.rs;
- crates/noema-api/src/graphql/tasks/;
- the Work sections of crates/noema-api/src/graphql/schema/query.rs,
  mutation.rs, and subscription.rs;
- Work-specific resolver, loader, error-mapping, and unit-test modules under
  crates/noema-api/src/graphql/;
- focused Work additions under crates/noema-api/src/graphql/schema_tests/.

Keep queries, mutations, subscriptions, connection helpers, and projection
mappers in focused modules rather than expanding one file beyond the project
source-size threshold.

### Forbidden files

This packet must not edit:

- crates/noema-workspaces, crates/noema-tasks, crates/noema-store, or
  crates/noema-runtime;
- apps/web, its GraphQL documents, generated schema, generated types, or route
  tree;
- root manifests, Cargo.lock, schema marker/version values, migrations, or
  docs/context/current.md;
- any unrelated GraphQL surface such as provider, memory, artifact, or MCP
  behavior.

## Interfaces consumed and produced

### Consumes

Use only the frozen store/domain interfaces for:

- owner/membership-authorized workspace, project, workflow, task, contract,
  gate, message, run, submission, review, artifact, and work-event reads;
- bounded batch projections for board/list/detail pages;
- semantic WorkCommand execution with expected revision, generation,
  client-mutation idempotency, actor, and correlation context;
- event-cursor validation, global workspace replay, and task-filtered event
  replay;
- current subscription invalidation receivers.

The GraphQL layer must not inspect raw SQLite rows, reimplement command
validation, or call a runtime worker to obtain task state.

### Produces

This packet exports the GraphQL types and operations named in
[the GraphQL contract](../06-graphql-contract.md):

- Workspace, Project, Workflow, WorkflowStage;
- TaskSummary, TaskDetail, TaskExecutionContract, TaskGate, TaskMessage,
  CurrentRunSummary, TaskSubmission, TaskReview, TaskAttention, ValidTaskAction,
  TaskRun, TaskRunItem, WorkEvent, and all documented connections;
- workOverview, projects, workTasks, task, needsYou, workActivity,
  completedTasks, and taskRunItems queries;
- semantic project/task mutations with command payloads;
- workEvents plus filtered taskEvents subscriptions and stable safe errors.

## Implementation checklist

### 1. Replace old task API assumptions

- [ ] Remove the existing GraphQL task-status/execution-phase projection and
      old status-specific controls from Work-owned API files.
- [ ] Expose exactly one task stage object. Current run, gate, review, and
      attention are separate nullable/derived fields.
- [ ] Add Planner to the run-kind presentation while preserving run-local
      run status. Do not add a task-level planner/executor/reviewer field.
- [ ] Remove legacy arbitrary task mutation paths rather than keeping aliases
      or compatibility facades in the pre-V1 schema.

### 2. Build bounded read projections

- [ ] Implement Workspace, Project, Workflow, WorkflowStage, TaskSummary, and
      TaskDetail mappers from frozen domain records.
- [ ] Return the Personal workspace normally while still authorizing the
      requested workspace id before all reads.
- [ ] Make project linkage nullable and preserve archived project/history
      visibility where the query asks for it.
- [ ] Add CurrentRunSummary, active gate, latest review, derived attention,
      and server-computed valid actions to summary/detail projections.
- [ ] Implement bounded connections for contracts, gates, messages, runs,
      submissions, reviews, activity, and run transcript items.
- [ ] Use store batch queries/loaders for board/list pages. Verify that
      summaries do not produce per-card run, gate, review, or project reads.

### 3. Implement query roots and filtering

- [ ] Add workOverview with workflow-driven active columns, counts, and a
      bounded board bootstrap projection.
- [ ] Add projects with archived filtering and stable project cursors.
- [ ] Add workTasks with workspace, optional project, stage-id/stage-behavior,
      text, and active/terminal/all scope validation.
- [ ] Add task with unavailable-on-missing-or-unauthorized behavior.
- [ ] Add needsYou derived exclusively from unresolved gates and current
      approved review readiness.
- [ ] Add workActivity and completedTasks with stable connection ordering and
      bounded cursors.
- [ ] Retain taskRunItems as a separately bounded detail transcript read.

### 4. Bind semantic mutations

- [ ] Add create/update/archive/reopen project mutations.
- [ ] Add capture/update-Inbox/queue/answer/retry/accept/request-changes/
      cancel/reopen task mutations.
- [ ] Require clientMutationId for every mutation and expected revision for
      every existing target. Require expected generation for task commands.
- [ ] Pass inputs to one semantic command service; never mutate a stage or
      task field directly from a resolver.
- [ ] Return the authoritative TaskDetail or Project projection, committed
      event cursor, and echoed clientMutationId from command payloads.
- [ ] Keep QueueTask free of caller-supplied contract, model, and policy
      inputs: Inbox Queue always creates Planner, while only server-owned
      continuations with a complete contract create Executor.
- [ ] Reject ambiguous project clearing and validate explicit gate ids for
      answer/retry. Accept and RequestTaskChanges verify the current eligible
      review server-side rather than accepting a caller-supplied review id.

### 5. Map safe errors and authorization

- [ ] Map unavailable, stale revision, stale generation, invalid transition,
      unresolved gate, missing execution configuration, review-not-approved,
      idempotency conflict, validation, and invalid connection/subscription
      cursors to the stable extensions.code values in the contract.
- [ ] Ensure unavailable object responses never reveal whether another
      principal owns the id.
- [ ] Never place provider details, lease tokens, credentials, hidden task
      content, raw tool payloads, or SQL details in error messages/extensions.
- [ ] Treat stale writes as explicit client conflicts; do not silently retry
      from GraphQL.

### 6. Implement ledger-backed subscriptions

- [ ] Add workEvents(workspaceId, after) over the single durable work-event
      ledger, using an exclusive global cursor.
- [ ] Reimplement taskEvents(taskId, after) as an authorized filtered
      projection of workEvents; remove any separate task-event stream/ledger
      assumption.
- [ ] Subscribe to invalidation before taking the durable head, replay batches
      of at most 256 events, and re-read on broadcast lag.
- [ ] Advance a subscription cursor only after an event projects successfully.
- [ ] Keep event payloads bounded/safe and return the stable closed dotted
      `WorkEventKind` strings.
- [ ] Ensure absent after starts after the current head, while history comes
      from workActivity or an explicit prior cursor.

### 7. Prepare integration output

- [ ] Export the schema through the existing API export path without manually
      editing generated SDL.
- [ ] Give the integrator the exact operations/type changes required for web
      code generation.
- [ ] Identify all deleted old task API operation names so web integration can
      remove them rather than preserve two contracts.

## Focused unit tests

Add GraphQL/API unit tests for:

- TaskSummary and TaskDetail expose one stage while Planner/Executor/Reviewer
  appear only as current/run history projections.
- Board/List filters, workflow-driven column order, project filtering, text
  pagination, and completed/cancelled history have stable cursors.
- Needs You derives clarification, approval, recovery, and review-ready items
  without a persisted attention or execution-phase field.
- Every semantic mutation requires expected revision and clientMutationId, and
  task mutations require expected generation.
- Queue, answer, retry, accept, request changes, cancel, and reopen return
  authoritative payloads with committed event cursors.
- No arbitrary stage setter is present in schema introspection.
- Safe errors have the documented codes and unauthorized/missing task/project
  access is indistinguishable.
- Page summaries use bounded/batched reads rather than an N-plus-one pattern.
- workEvents replays a durable cursor, handles wakeup/lag without loss, and
  taskEvents returns the same filtered ledger events.
- Contract/gate/message/run transcript connection limits reject invalid page
  sizes and preserve ordering.
- Review history exposes `reviewAttemptIndex` and `supersedesReviewId`, so a
  needs-human decision and the post-answer final review remain independently
  queryable for the same submission.

Use unit tests only. Do not add browser, smoke, fixture, or external-provider
tests.

## Validation

Run focused API tests while developing, then before handoff run:

1. cargo fmt --all --check
2. cargo check --workspace
3. cargo clippy --workspace --all-targets -- -D warnings
4. cargo test --workspace --no-fail-fast

The integrator performs generated schema and frontend operation checks after
accepting this packet. Do not alter the compiler wrapper or build-cache
configuration.

## Expected commit

The integrator will serialize commits. Prepare this packet as:

**feat(api): expose semantic Work GraphQL contract**

Do not commit, stage, regenerate frontend files, or modify the durable context
file unless the integrator explicitly delegates that work.

## Handoff checklist

- [ ] List changed API files, deleted old task API paths, and added test files.
- [ ] Confirm every resolver uses frozen store/domain contracts and no direct
      state mutation was introduced.
- [ ] Report all exported operation/type names and generated-schema impact for
      the UI packet.
- [ ] Confirm subscription replay is sourced from work_events and taskEvents
      is filtered from it.
- [ ] Report focused and full validation results, plus any unrelated
      pre-existing failure.
- [ ] State every implementation assumption, or explicitly report `none`.
- [ ] Report unresolved contract-change requests by exact interface; do not
      patch another packet's files.
