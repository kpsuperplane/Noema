# Task 1 Fix Report: Remove Persistent SurrealDB Compatibility Store

## Implementation

- Removed the `NoemaStore.db: Surreal<Db>` runtime field so the Task 1 runtime contract is now limited to the SQLite connection, Noema home path, and write locks.
- Removed the `NoemaStore::open()` creation of `${NOEMA_HOME}/db/surrealdb-compat`.
- Replaced the persistent RocksDB compatibility client with a process-local in-memory SurrealDB adapter initialized through `Surreal::new::<Mem>(())`. This keeps unported repository modules compiling without making `NoemaStore` own or open a persistent SurrealDB/RocksDB database.
- Rewrote unported repository call sites from direct `self.db` field access to the existing `self.db()` adapter method.
- Enabled the `kv-mem` SurrealDB feature while keeping `kv-rocksdb` for later tasks that have not removed the dependency yet.
- Updated the SurrealDB error wording to describe a transitional compatibility operation instead of the embedded store path.
- Added a regression test proving `NoemaStore::open()` does not create `db/surrealdb-compat`.

## Tests

- RED: `cargo test -p noema-core store::tests::opening_sqlite_store_does_not_create_surreal_compat_db -- --nocapture`
  - Failed as expected because `NoemaStore::open()` created `db/surrealdb-compat`.
- GREEN: `cargo test -p noema-core store::tests::opening_sqlite_store_does_not_create_surreal_compat_db -- --nocapture`
  - Passed.
- `cargo test -p noema-core paths::tests::sqlite_db_path_lives_under_db_dir -- --nocapture`
  - Passed, 1 test.
- `cargo test -p noema-core store::tests:: -- --nocapture`
  - Passed, 3 tests.
- `cargo check -p noema-core`
  - Passed.
- `cargo fmt --all --check`
  - Passed.
- `git diff --check`
  - Passed.

## Files Changed

- `Cargo.lock`
- `Cargo.toml`
- `crates/noema-core/src/store/agent_runtime_preferences.rs`
- `crates/noema-core/src/store/agents.rs`
- `crates/noema-core/src/store/auxiliary_model_preferences.rs`
- `crates/noema-core/src/store/claims/consolidation.rs`
- `crates/noema-core/src/store/claims/graph.rs`
- `crates/noema-core/src/store/claims/inspection.rs`
- `crates/noema-core/src/store/claims/write.rs`
- `crates/noema-core/src/store/context_summaries.rs`
- `crates/noema-core/src/store/conversations.rs`
- `crates/noema-core/src/store/error.rs`
- `crates/noema-core/src/store/mcp/approvals.rs`
- `crates/noema-core/src/store/mcp/calibrations.rs`
- `crates/noema-core/src/store/mcp/servers.rs`
- `crates/noema-core/src/store/mcp/tools.rs`
- `crates/noema-core/src/store/mcp/trusted_identities.rs`
- `crates/noema-core/src/store/ontology.rs`
- `crates/noema-core/src/store/provider_accounts.rs`
- `crates/noema-core/src/store/provider_capability_bindings.rs`
- `crates/noema-core/src/store/retrieval.rs`
- `crates/noema-core/src/store/runtime.rs`
- `crates/noema-core/src/store/tests.rs`
- `.superpowers/sdd/task-1-fix-report.md`

## Self-Review

- `NoemaStore` no longer has a SurrealDB field.
- `NoemaStore::open()` no longer imports or uses `RocksDb`.
- `NoemaStore::open()` no longer creates any persistent compatibility database path.
- A targeted scan found no remaining `RocksDb`, `pub(super) db`, or runtime `surrealdb-compat` references. The only `surrealdb-compat` reference is the regression assertion.
- The adapter is intentionally narrow and temporary: it preserves compileability for repository modules that later tasks will port, without expanding Task 1 into a full repository migration.

## Concerns

- The process-local in-memory SurrealDB adapter is shared across `NoemaStore` handles and is only a compile bridge for unported modules. It is not a canonical store and should be removed when those modules move to SQLite.
- Enabling `kv-mem` adds lockfile entries for the in-memory SurrealDB engine. This is directly tied to avoiding a persistent compatibility database while preserving compileability.
