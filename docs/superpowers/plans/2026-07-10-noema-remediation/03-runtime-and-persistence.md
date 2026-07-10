# Phase 3: Runtime and Persistence

**Target time:** 2 hours 30 minutes
**Hard cap:** 3 aggregate agent-hours
**LOC target:** at least 800 net lines deleted
**Risk:** high; one 20-minute read-only correctness review is allowed
**Prerequisite:** Phases 1 and 2 are validated

## Goal

Replace ad hoc task ownership, cancellation, and synchronous SQLite access with
Tokio's standard lifecycle utilities and a dedicated SQLite worker. Fix the
small set of transactional invariants that can cause duplicate or corrupt live
state without attempting a complete schema redesign.

## Files in Scope

Runtime:

- `crates/noema-core/src/daemon/runtime.rs`
- `crates/noema-core/src/daemon/runtime/actor.rs`
- `crates/noema-core/src/daemon/runtime/handle.rs`
- `crates/noema-core/src/daemon/runtime/turn.rs`
- `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- runtime host, memory lifecycle, provider task, and subscription ownership
  modules
- `crates/noema-core/src/daemon/tests.rs`

Persistence:

- `crates/noema-core/src/store/runtime.rs`
- `crates/noema-core/src/store/sqlite.rs`
- `crates/noema-core/src/store/conversations.rs`
- `crates/noema-core/src/store/context_summaries.rs`
- `crates/noema-core/src/store/provider_accounts.rs`
- `crates/noema-core/src/store/mcp/**`
- credential/config/artifact filesystem writers touched by the selected
  transaction paths

## Sequence

### Checkpoint 3A — Owned tasks and cancellation (60 minutes)

- [x] Add direct `tokio-util` `CancellationToken` and `TaskTracker` dependencies
  with the minimum feature set.
- [ ] Give the runtime host one root cancellation token and task tracker.
- [ ] Replace detached memory ingestion, compaction, provider generation,
  connection, and subscription relays with tracked tasks or explicitly
  documented short-lived request tasks.
- [x] Implement dependency-ordered shutdown: stop accepting work, cancel turns,
  close queues, wait for tracked tasks, then stop Mnemosyne/model proxy/bridges.
- [ ] Replace the single global turn actor with a small conversation-worker
  registry only if the existing actor can be removed in this checkpoint.
  Otherwise add per-conversation queue ownership behind the current handle and
  defer physical actor deletion.
- [ ] Expose explicit `submit`, `cancel`, and `interrupt_and_submit` operations.
  Ordinary sends queue; cancel and replacement are distinct.
- [ ] Consolidate turn completed/failed/cancelled/interrupted finalization into
  one scoped finalizer and delete repeated cleanup branches.
- [ ] Add focused unit coverage for two concurrent conversations, duplicate
  same-conversation submission, cancellation, interruption, and clean shutdown.
- [x] Commit the runtime checkpoint before altering SQLite plumbing.

Checkpoint target: at least 500 net lines deleted.

### Checkpoint 3B — Dedicated SQLite worker (55 minutes)

- [ ] Replace `Arc<Mutex<rusqlite::Connection>>` with
  `tokio_rusqlite::Connection` 0.7, which uses the workspace's current
  `rusqlite` 0.37.
- [ ] Define one narrow `Store::call`/transaction boundary and route all
  runtime-callable store operations through it. Do not retain a second direct
  connection path outside `cfg(test)`.
- [ ] Keep row mapping and SQL in owning store modules; do not move every query
  into one closure file.
- [ ] Make primary-conversation lookup/create and active-summary replacement
  single `IMMEDIATE` transactions with their existing product semantics.
- [ ] Wrap provider and MCP multi-row deletes in transactions.
- [ ] Enable SQLite foreign-key enforcement at every opened connection and add
  only the relationship constraints needed by touched invariants.
- [ ] Remove lock helpers, duplicated connection error adapters, and tests whose
  only purpose was the old mutex implementation.
- [ ] Run store and concurrency tests before committing.

If a complete store conversion cannot compile within 40 minutes, revert the
conversion checkpoint and implement a single bounded `spawn_blocking` store
executor around the existing connection as a temporary coherent boundary.
Never leave some production calls on the async executor and others on a new
worker without an explicit facade.

### Checkpoint 3C — Atomic private state (35 minutes)

- [x] Evaluate `atomic-write-file` against current macOS/Linux permission,
  same-directory rename, sync, and replacement requirements.
- [ ] Adopt one private atomic writer only if it replaces provider, MCP,
  configuration, and artifact temp-write helpers in this checkpoint.
- [ ] Apply `0700` to Noema private directories and `0600` to credential,
  database, WAL/SHM, configuration, artifact, and diagnostic files where the OS
  supports Unix modes.
- [ ] New private files explicitly request Unix mode `0600`; do not rely on
  `atomic-write-file`'s default `0666 & umask` creation mode.
- [ ] Repair unsafe existing permissions during startup without reading or
  logging secret content.
- [ ] Persist refreshed provider credentials through the shared atomic writer.
- [ ] Add focused permission/atomic replacement tests on Unix; use portable
  semantic tests elsewhere.

## Explicit Schema Limit

Do not recreate the discarded schema-v2 structural-fingerprint program in this
phase. The program permits:

- enabling foreign keys;
- adding constraints required by touched transactions;
- direct pre-V1 schema edits;
- a clear reset-required error for incompatible stores.

Defer a complete ownership/foreign-key rewrite until it has its own product and
data model brief.

## Validation

```bash
cargo fmt --all --check
cargo check -p noema-core -p noema-desktop
cargo clippy -p noema-core --all-targets -- -D warnings
cargo test -p noema-core daemon::runtime -- --test-threads=1
cargo test -p noema-core store:: -- --test-threads=1
cargo test -p noema-core daemon::tests:: -- --test-threads=1
git diff --check
```

Run only the relevant daemon test filters during checkpoints; charge the full
workspace suite to the phase gate or Phase 6.

## Exit Evidence

- [ ] At least 800 net source lines deleted.
- [ ] No runtime-owned detached task remains without a documented owner.
- [x] Cancellation and shutdown tests pass without fixed sleeps.
- [ ] Runtime SQLite operations do not block Tokio worker threads.
- [ ] Critical multi-step store operations are transactional.
- [ ] Atomic private writes use one implementation.
- [x] Correctness review has no unresolved Critical finding.

## Completion Record

Phase 3 stopped at its timebox after commit `03543b22`. Maintained source is
`92,387` lines: `+230` in this phase and `-1,537` from the program baseline.
The phase intentionally missed its deletion target rather than forcing an
incomplete persistence conversion.

The landed unit uses `tokio-util`'s `CancellationToken` and `TaskTracker` to own
provider one-shots, memory ingestion, and background compaction. Explicit and
last-handle shutdown cancel those jobs, interrupt blocked inline turns, atomically
recover nonterminal items/turns to cancelled with the conversation idle, drain
tracked work, and only then stop Mnemosyne. Independent Luna-max final review
found no Critical or Important issue. The full workspace gate passed with 669
core library, 10 development-binary, and 7 desktop tests.

`tokio-rusqlite` 0.7 is dependency-compatible with the workspace's `rusqlite`
0.37, but conversion requires 75 production closures across 14 modules and was
estimated at 90–150 minutes plus 100–250 added lines because `call` requires
owned `Send + 'static` closures and new error adaptation. Partial conversion was
rejected. `atomic-write-file` 0.3 was also rejected here: only four mutable file
writes qualify, while its mode/durability glue and new `rand`/`nix` versions
would be LOC-positive. Dedicated SQLite execution, remaining proxy/GraphQL task
ownership, public turn cancellation/interruption, and private-file permission
repair remain deferred rather than hidden behind partial abstractions.

## Stop Conditions

- Revert an incomplete SQLite conversion rather than extending the phase.
- Do not split runtime files merely to satisfy file size; consolidation must
  remove duplication or clarify a lifecycle boundary.
- Do not add a general saga framework or migration layer.
- Do not consume reserve time for exhaustive schema constraints.
