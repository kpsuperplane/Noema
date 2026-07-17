# Validation And Rollout

## Purpose

This document is the release gate for Noema Work. It maps each settled product
and architecture requirement to an owning test layer, defines milestone and
full-workspace validation, and specifies the pre-V1 schema reset. The release is
ready only when every required row below passes without disabling caches,
loosening warnings, or substituting manual inspection for an automated domain,
store, runtime, or API invariant.

UI/frontend implementation follows the project rule: do not add frontend tests
for this feature. Validate generated contracts, TypeScript, lint, and production
builds instead.

## Test Ownership

| Layer | What it proves |
| --- | --- |
| `noema-workspaces` unit tests | Workspace/project IDs, normalization, Personal constants, project lifecycle validation |
| `noema-tasks` unit tests | Stage behaviors, commands, transition planning, immutable contracts, gates, generation and revision rules |
| `noema-store` unit tests | Schema, seeds, transaction atomicity, constraints, indexes, cursor replay, idempotency, queue claiming, and persisted projections |
| `noema-runtime` unit tests | Planner/executor/reviewer routing, reconciliation, tools, safe context, gates, recovery, and notification delivery |
| `noema-api` unit tests | GraphQL projections, semantic mutations, pagination, subscriptions, authorization, and safe errors |
| Frontend static gates | Generated schema/operations, route generation, TypeScript, ESLint, and production bundling |

Tests may share deterministic builders, but each invariant belongs at its lowest
authoritative layer. API tests must not be the only proof of a store constraint;
runtime tests must not duplicate a pure domain transition matrix.

## Requirement-To-Test Matrix

### Bootstrap, hierarchy, and workflow

| Requirement | Required proof |
| --- | --- |
| A fresh database contains Personal, its local-owner membership, one default workflow, and seven ordered stages | Store schema/seed test checks exact IDs, behaviors, order, terminal visibility, and membership role |
| Every task belongs to a workspace and workflow; project is optional | Domain validation plus SQLite non-null and composite-reference tests |
| A project and task must belong to the same workspace | Domain error test and transaction rollback test |
| A stage must belong to the task's workflow | Domain validation plus store foreign-key/transaction test |
| Only five stages appear on the active board | API projection test excludes TerminalSuccess and TerminalCancelled behaviors |
| Stage is the sole task-level state | Compile-time removal of `TaskStatus`; schema test rejects legacy `status`/execution-phase columns; GraphQL schema snapshot exposes no replacement axis |

### Task commands and contracts

| Requirement | Required proof |
| --- | --- |
| Capture creates Inbox without a run | Domain command plan and atomic store test |
| Queue is the only user authorization for execution | Invalid-transition tests and store test showing no run is created from Inbox edits |
| Title, description, and project are directly editable only in Inbox | Domain matrix covers every other stage; store leaves projection/event ledger unchanged on rejection |
| Every mutation checks expected revision and generation | Stale revision/generation tests for every command family |
| A repeated idempotency key returns the original result | Store tests compare task, event cursor, contract, gate, and run counts before/after replay |
| Execution contracts are immutable | Update/delete attempts fail; later changes create a new version linked to the prior version |
| Request Changes records feedback rather than rewriting evidence | Domain/store test verifies a task message and new contract revision while prior contract/submission/review remain unchanged |
| Accept requires Review plus an approved latest review | Domain tests reject acceptance from every other stage and with stale/unapproved review evidence |

### Dispatch, planning, execution, and review

| Requirement | Required proof |
| --- | --- |
| Runnable work is claimed FIFO | Store test fixes timestamps and verifies `queued_at`, then run ID ordering with no priority input or column |
| One active run exists per task generation | Unique/transaction test races two reconciliation attempts and observes one run |
| Queue without a complete contract schedules Planner | Reconciler unit test and persisted integration-style unit test |
| Queue with a complete contract schedules Executor | Reconciler unit test verifies Planner is skipped |
| First claimed run moves Queue to Doing | Atomic claim test checks stage event and lease in the same transaction boundary |
| Planner completion freezes a contract and schedules Executor | Runtime/store tests verify model snapshots, context snapshots, criteria, and run causality |
| Executor submission schedules Reviewer while task stays Doing | Runtime/store test checks stage and run kind/status independently |
| Reviewer request changes queues another Executor and stays Doing | Review-planner test checks incremented revision and triggering review ID |
| Reviewer approval moves to Review without completing | Runtime/store/API tests verify Review, ReviewReady attention, and no completion timestamp |
| Human acceptance moves Review to Completed | Command/store test checks accepted submission, event, notification, and history visibility |
| Review-round exhaustion creates Recovery/Waiting | Runtime test exhausts the configured bound and verifies no further run is queued |

### Gates, messages, and recovery

| Requirement | Required proof |
| --- | --- |
| Clarification, approval, and recovery are persisted gates | Domain parsing and store round-trip tests for all kinds |
| Only one unresolved task gate exists | Store uniqueness/race test |
| Waiting always has an unresolved gate | Transaction invariant tests reject orphan Waiting and gate creation outside the corresponding transition |
| Answer resolves the gate, appends a message, and returns Queue | Atomic store test checks all three changes and emitted events |
| A resolved gate resumes the role that opened it | Table-driven runtime/store tests cover Planner, Executor, Reviewer, and failed-role Recovery continuations |
| Human input is consumed at a safe run boundary | Runtime prompt/context test proves only the next admitted continuation sees the message |
| Reviewer can finish after a human answer | Store/runtime test persists immutable `needs_human` attempt 1 and a linked final attempt 2 for the same submission |
| Automatic retry stays Doing | Reconciler test verifies incremented `attemptIndex` and no human gate before exhaustion |
| Exhausted failure creates one Recovery gate and one notification | Runtime/outbox idempotency test repeats reconciliation and delivery |
| Recovery continuation is explicit | Domain/store/runtime tests cover the closed reason/action matrix, verify infrastructure resumes the failed role, review exhaustion offers one Executor round, unsafe effects cannot blind-Retry, and invariant recovery exposes neither Answer nor Retry |
| Restart reconciliation is idempotent | Table-driven tests cover every documented crash point with two consecutive reconciler passes |

### Cancellation, reopening, and stale work

| Requirement | Required proof |
| --- | --- |
| Cancel increments generation and requests cancellation for runnable runs | Atomic store test checks task, runs, and event |
| A stale lease cannot submit after cancellation or revision | Submission/review/run-finalization tests use the old generation and lease token and receive a typed conflict |
| Cancelled appears only in history | API list/filter tests |
| Reopen preserves evidence and creates a fresh cycle in Inbox | Store test compares old contracts/runs/submissions/reviews before and after reopen |
| Repeated Cancel or Reopen is idempotent only with the same command key | Domain/store tests distinguish replay from a new invalid command |

### Events, subscriptions, and notifications

| Requirement | Required proof |
| --- | --- |
| Projection update and work event are atomic | Fault-injection transaction tests force failure before commit and observe neither change |
| Event sequence is globally monotonic | Concurrent append test checks uniqueness and strict ordering |
| Cursors replay without gaps or duplicates | Store/API pagination tests span identical timestamps and multiple aggregate kinds |
| Workspace/project/task/run filters preserve global cursors | API tests page filtered streams and resume from the last delivered cursor |
| Notification delivery is idempotent per event/destination | Outbox test crashes after append and after delivery acknowledgment |
| Chat receives only creation, Waiting, Review readiness, recovery, and accepted completion | Runtime notification policy test rejects routine run/stage events |
| Notification destination is the global primary conversation, not provenance | Runtime/store test creates a task from Work and proves its attention card resolves through `human:local` while source conversation remains null |
| Subscription reconnect catches up before live delivery | API subscription test appends during the replay/live handoff and observes each event once |

### Context, authority, and chat intent

| Requirement | Required proof |
| --- | --- |
| Workspace/project names and descriptions are snapshotted separately | Domain/store/runtime tests compare snapshots with source records |
| Editing project metadata does not mutate an admitted contract | Store immutability test |
| Workspace/project context does not change agent/model/capabilities | Runtime context and capability tests compare otherwise identical scoped/unscoped runs |
| Planner has no general action tools | Tool-visibility test asserts its exact allowlist |
| Executor/reviewer retain current role restrictions | Existing capability tests are updated, not replaced |
| Short work may remain foreground; capture uses Inbox; delegation uses Queue | Structured fake-provider tests exercise all three tool-choice outcomes |
| The roughly-15-second policy is not phrase matching | Prompt/tool-policy test checks structured instructions; source audit rejects prefix/English routing helpers |
| Ambiguous gate answers are not guessed | Runtime test with two open task cards requires explicit correlation and performs no mutation otherwise |

### GraphQL and client projections

| Requirement | Required proof |
| --- | --- |
| No generic stage mutation exists | Exported schema assertion |
| Semantic mutations return task plus event cursor | Resolver tests for every mutation |
| Work queries are bounded and cursor-paginated | Limit clamping, exclusive `after`, cursor-family mismatch, and empty-page tests |
| Task summaries avoid per-card reads | Store/API query-count or prepared-projection test at the repository boundary |
| Attention is derived | API tests cover Clarification, Approval, Recovery, ReviewReady, and absence |
| `validActions` agrees with domain commands | Table-driven resolver test across every stage/gate/review combination |
| Safe errors are stable | Tests assert code and safe metadata for stale revision, invalid transition, configuration missing, unresolved gate, and not found |
| Generated frontend contracts are current | `bun run check:generated` passes with a clean generated diff |

## Acceptance Scenarios

These scenarios are cross-layer unit-test compositions, not smoke tests.

1. **Capture and queue:** capture in chat, inspect Inbox in Work, edit it, queue
   it, and observe one Planner or Executor run.
2. **Clarification:** queue an underspecified task, receive one Waiting card,
   answer from the correlated card, and resume without duplicate gates or runs.
3. **Automated revision:** executor submits, reviewer requests changes, a new
   executor run uses the feedback, and the task remains Doing.
4. **Human acceptance:** reviewer approves, task moves Review, Work and chat
   expose Accept/Request Changes, and only Accept moves it to Completed.
5. **Recovery:** exhaust automatic retries, create a Recovery gate, retry from
   Waiting, and prevent the failed stale run from writing afterward.
6. **Cancel and reopen:** cancel active work, reject a late completion, reopen
   to Inbox, and preserve the original evidence timeline.
7. **Restart:** stop at every reconciler crash point, reopen the same database,
   and converge without duplicate events, gates, contracts, runs, or notices.
8. **Scoped context:** execute otherwise identical standalone/project tasks and
   verify only the bounded context section changes.

## Milestone Validation

Run focused tests continuously, then run the full gate at the end of every
accepted implementation packet that compiles the workspace.

### Rust gate

From the repository root:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Never unset, replace, or bypass `CARGO_BUILD_RUSTC_WRAPPER`; never interfere
with `sccache`. Run unit tests only.

### Frontend gate

From `apps/web`:

```bash
bun run check:generated
bun run lint
bun run build
bun run build:tauri
```

Do not add frontend tests or use browser inspection for this program unless the
user explicitly expands the validation scope.

### Diff gate

Before every packet commit and before final integration:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

The coordinator must verify that staged files belong only to the accepted
packet and must report all remaining unstaged/untracked files.

## Pre-V1 Rollout

This release rewrites the bootstrap schema from `sqlite_store_v2` to
`sqlite_store_v3`. It does not migrate or operate a mixture of versions.

1. Stop the Noema host so no process owns the database or WAL.
2. If development data matters, copy the explicit database family rooted at
   `${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3` to a separate backup location.
   Include its `-wal` and `-shm` siblings when present.
3. Move the explicit v2 database family aside or use the project's normal local
   reset workflow. Never target a broad directory such as `$HOME`, `~`, or the
   Noema home root.
4. Start the new binary. Bootstrap must create v3 atomically, seed Personal and
   the default workflow, and reject any non-empty incompatible database rather
   than partially modifying it.
5. Verify the schema marker, seed rows, default provider selections, primary
   conversation, empty Work board, event cursor, and worker startup.

There is no feature flag, dual write, legacy read path, or fallback to v2. An
implementation rollback means reverting the complete Work implementation and
restoring/resetting a matching development database; it never means running an
old binary against v3 or a new binary against v2.

## Release Gate

The Work release is complete only when:

- every required matrix row has an automated owner and passes;
- all six implementation packets satisfy their handoff checklists;
- the complete Rust and frontend gates pass;
- generated GraphQL and route files are clean;
- the schema bootstrap and incompatible-schema rejection tests pass;
- no legacy task-status axis, task/run event ledger, or alternate mutation path
  remains reachable;
- `docs/context/current.md` describes the implemented state rather than the
  planned state; and
- the coordinator has inspected the final staged diff and recorded remaining
  unrelated worktree changes.
