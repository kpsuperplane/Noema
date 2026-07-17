# Work Packet 06: Integration

> **Mode:** implementation. The program coordinator owns this packet after the
> domain, store, runtime, API, and frontend packet handoffs are available.

## Goal

Integrate the independently owned Work packets into one coherent build, remove
the superseded task-status paths, regenerate first-party client contracts, run
the complete validation gate, update durable project context, and serialize the
accepted work into focused commits without staging unrelated changes.

This packet does not redesign a subsystem. Contract changes discovered during
integration return to the owning packet through the change-control process in
[the implementation program](README.md).

## Prerequisites

- [ ] Domain packet exports the frozen workspace/project/task/workflow command
      contracts and passes its focused tests.
- [ ] Store packet implements the v3 schema, command transactions, projections,
      events, and query contracts.
- [ ] Runtime packet implements planner/executor/reviewer reconciliation, tools,
      gates, context, and notifications.
- [ ] API packet exports the frozen GraphQL schema and passes resolver tests.
- [ ] Frontend packet is ready to consume regenerated GraphQL and route output.
- [ ] Every packet handoff lists changed files, validation, unresolved issues,
      and requested integration work.
- [ ] The coordinator has re-read `AGENTS.md`, checked `git status`, and recorded
      all pre-existing unrelated changes.

## Exclusive Ownership

The integrator owns:

- the root `Cargo.toml` and `Cargo.lock`;
- cross-crate dependency wiring and public re-exports;
- `apps/web/src/generated/`, `apps/web/src/routeTree.gen.ts`, and other generated
  GraphQL/router output;
- removal of obsolete cross-layer imports after the owning implementation has
  landed;
- narrow cross-crate compile fixes approved by the original owner;
- `docs/context/current.md` and the status table in `docs/workspaces/README.md`;
- final validation, staged-diff inspection, and serialized commits.

The integrator does not take silent ownership of feature logic inside another
packet. Any non-mechanical change to stage transitions, transactions, runtime
policy, GraphQL behavior, or UI interaction is returned to that owner.

## Forbidden Files And Changes

- Do not add a compatibility facade for `TaskStatus`, legacy task events, or old
  GraphQL task status fields.
- Do not retain dual schema, read, write, event, command, or notification paths.
- Do not handwrite generated GraphQL or TanStack Router output.
- Do not loosen role capabilities, approval policy, trust checks, validation,
  lint, Clippy, or test gates to make integration pass.
- Do not modify or stage unrelated dirty-worktree changes.
- Do not push unless the user explicitly asks.

## Interfaces Consumed

- Public exports and transition tests from Work Packet 01.
- Store methods, schema, cursor, and event contracts from Work Packet 02.
- Runtime worker/tool/notification integration from Work Packet 03.
- GraphQL schema and operation contract from Work Packet 04.
- Frontend routes, operations, and components from Work Packet 05.
- The acceptance matrix in [validation and rollout](../08-validation-and-rollout.md).

## Interfaces Produced

- Acyclic Cargo dependency graph containing `noema-workspaces`.
- One compiled command path from tools/GraphQL to store transactions.
- One compiled event path from store to subscriptions and chat delivery.
- Regenerated and clean GraphQL TypeScript and route files.
- No reachable legacy task-status or task/run event API.
- Full validation results and updated durable documentation.
- Focused commits whose staged paths were explicitly inspected.

## Focused Tests

The integrator reruns each packet's named focused unit tests after wiring its
public seams, then runs the eight cross-layer unit-test compositions in step 8.
It may add only narrow cross-crate unit-test composition needed to prove a seam;
subsystem behavior still belongs in the owning packet. Smoke, fixture, browser,
and external-provider tests remain out of scope.

## Expected Commit

The final integration boundary is prepared as
`feat(workspaces): integrate Work management system`. The integrator may retain
the dependency-ordered packet commits listed below, but it must not fold
unrelated worktree changes into this commit.

## Integration Sequence

### 1. Establish the integration baseline

- [ ] Run `git status --short --branch` and save the list of pre-existing
      modified, deleted, and untracked files in the integration notes.
- [ ] Inspect every packet handoff and verify that its files stay inside the
      declared ownership boundary.
- [ ] Run `git diff --check` before integration so pre-existing whitespace
      failures can be distinguished from packet regressions.
- [ ] Confirm no agent has staged files or left an incomplete Git operation.
- [ ] Confirm the v3 schema rewrite is the only supported bootstrap schema.

### 2. Wire workspace/domain dependencies

- [ ] Add `crates/noema-workspaces` to the Cargo workspace and central dependency
      declarations.
- [ ] Add only the directed dependencies established by the architecture:
      `noema-tasks -> noema-workspaces`, then store/runtime/API consumers.
- [ ] Do not make `noema-workspaces` depend on tasks, store, runtime, or API.
- [ ] Regenerate `Cargo.lock` through Cargo rather than editing it manually.
- [ ] Run `cargo tree --workspace -e normal` and reject a cycle or a new generic
      foundation crate.
- [ ] Run focused checks for `noema-workspaces` and `noema-tasks`.

### 3. Integrate the store contract

- [ ] Verify store row types remain private and public records come from their
      semantic owner crates.
- [ ] Verify all semantic commands enter one transaction implementation and
      append a work event before commit.
- [ ] Confirm `work_events` replaces task/run ledgers rather than mirroring them.
- [ ] Confirm Personal/workflow seeds are deterministic and idempotent.
- [ ] Confirm task/run generation and contract IDs cross the store/runtime
      boundary without stringly typed shadow fields.
- [ ] Run schema, command, event, query, and recovery-focused store tests.

### 4. Integrate runtime and tool paths

- [ ] Wire Planner into run dispatch and exhaustive `RunKind` matches.
- [ ] Verify runtime stage changes call the store command path rather than
      directly updating task rows.
- [ ] Verify the reconciler schedules one next action and can be repeated.
- [ ] Replace the old completion-only delivery path with the generalized work
      notification outbox.
- [ ] Replace old resume semantics with explicit Answer/Retry commands while
      keeping task messages and run continuations causal.
- [ ] Verify primary, planner, executor, and reviewer tool visibility tests.
- [ ] Run focused runtime task, tool, completion, continuation, and recovery
      tests.

### 5. Integrate GraphQL

- [ ] Wire all Work queries, semantic mutations, and cursor subscriptions into
      the schema root.
- [ ] Remove obsolete GraphQL `TaskStatus` and generic resume/cancel projections
      only after all callers use the new contract.
- [ ] Verify resolvers use bounded store projections and do not construct task
      state from current runs.
- [ ] Verify safe error codes and `expectedRevision`/`clientMutationId` behavior.
- [ ] Export the schema once the API-focused tests pass.

### 6. Regenerate and integrate the frontend

- [ ] From `apps/web`, run `bun run gen:types` and `bun run gen:routes`.
- [ ] Inspect generated diffs; do not accept unrelated schema or route churn.
- [ ] Connect the frontend agent's operation documents to generated types.
- [ ] Remove handwritten transition/status compatibility mappings.
- [ ] Verify chat task cards and `/work` use the same task detail/query contract.
- [ ] Verify shell navigation recognizes `/work` and `/work/tasks/$taskId`.
- [ ] Run `bun run check:generated`, `bun run lint`, `bun run build`, and
      `bun run build:tauri`.

### 7. Remove superseded implementation

- [ ] Use `rg` to find every remaining `TaskStatus`, legacy status wire value,
      `task_events`, `run_events`, old status outbox, and arbitrary status setter.
- [ ] Classify each match as removed, renamed to an intentional run-local status,
      or a historical document/test fixture that must be updated.
- [ ] Remove old store methods and runtime/API adapters once no callers remain.
- [ ] Ensure `TaskReference` transcript payloads expose stage and derived
      attention/run data rather than the removed status.
- [ ] Run `cargo check --workspace` after removal before broader cleanup.

### 8. Run cross-layer acceptance scenarios

- [ ] Capture and queue.
- [ ] Planner clarification and correlated answer.
- [ ] Executor/reviewer automated revision.
- [ ] Reviewer approval and human acceptance.
- [ ] Exhausted recovery and retry.
- [ ] Cancellation, stale completion rejection, and reopen.
- [ ] Restart reconciliation from every documented crash boundary.
- [ ] Project-context snapshot without capability/model changes.

Implement these as unit-test compositions in the owning Rust crates. Do not add
smoke tests, fixture tests, frontend tests, or browser automation.

### 9. Run the complete gate

- [ ] `cargo fmt --all --check`
- [ ] `cargo check --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --no-fail-fast`
- [ ] Frontend generated, lint, web build, and Tauri build gates from the
      validation document.
- [ ] `git diff --check`

Do not modify `CARGO_BUILD_RUSTC_WRAPPER` or interfere with `sccache`.

### 10. Update durable context and program status

- [ ] Replace planned language in `docs/context/current.md` with the exact
      implemented schema, runtime, API, UI, and validation result.
- [ ] Update the status table in `docs/workspaces/README.md` for accepted packets.
- [ ] Record any deliberately deferred item in `09-future-roadmap.md`; do not
      leave an unowned TODO in an implementation document.
- [ ] Verify all links under `docs/workspaces` resolve.

### 11. Serialize focused commits

Pause parallel writers before touching the Git index. For each accepted unit:

- [ ] Stage only explicit packet paths.
- [ ] Inspect `git diff --cached --stat` and `git diff --cached --name-status`.
- [ ] Inspect the full cached diff for generated or integration commits.
- [ ] Commit the unit with the packet's expected commit subject.
- [ ] Re-run `git status --short --branch` and report remaining unrelated files.

Suggested sequence:

1. `feat(workspaces): add workspace and task workflow contracts`
2. `feat(workspaces): persist work commands and event ledger`
3. `feat(workspaces): orchestrate planned agent work`
4. `feat(workspaces): expose work graphql contract`
5. `feat(workspaces): add work management interface`
6. `docs(workspaces): record implemented work system`

If packets land as fewer coherent commits, preserve dependency order and never
mix unrelated dirty-worktree changes into them.

## Contract-Change Procedure

When integration exposes a real contract defect:

1. Stop the dependent integration step.
2. Record the failing evidence and affected public contract.
3. Return the change to its owning packet.
4. Update the authoritative design document if the settled behavior changes.
5. Notify every downstream owner and regenerate affected client contracts.
6. Re-run the owner's focused gate before resuming integration.

Mechanical import, generated-output, and exhaustive-match fixes do not require
a contract change, but the integrator must keep them narrow.

## Handoff Checklist

- [ ] All prerequisite packet handoffs are accepted.
- [ ] Cargo dependency graph is acyclic and contains no generic replacement
      domain crate.
- [ ] One task stage, command writer, event ledger, and notification path remain.
- [ ] Generated GraphQL and route files are current.
- [ ] Every acceptance-matrix row has a passing owner.
- [ ] Complete Rust and frontend gates pass.
- [ ] `docs/context/current.md` and program status describe implemented reality.
- [ ] Staged diffs were inspected before each commit.
- [ ] Remaining unrelated modified, deleted, and untracked files are reported.
- [ ] Every integration assumption is stated, or explicitly reported as `none`.
- [ ] Nothing was pushed without explicit user authorization.
