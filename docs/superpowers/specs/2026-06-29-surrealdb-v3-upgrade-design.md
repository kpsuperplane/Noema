# SurrealDB v3 Upgrade Design

## Goal

Upgrade Noema from SurrealDB Rust SDK v2 to the latest stable v3 line.

The current workspace dependency is:

```toml
surrealdb = { version = "2", default-features = false, features = ["kv-rocksdb"] }
```

The upgrade should move that to stable v3 while keeping the same embedded
RocksDB architecture:

```toml
surrealdb = { version = "3", default-features = false, features = ["kv-rocksdb"] }
```

Cargo registry metadata checked during design showed the current local
resolution as `2.6.5`, latest stable v3 as `3.1.5`, and a newer beta
`3.2.0-beta.2`. This design intentionally targets stable v3, not the beta
line.

## Non-Goals

- No data migration or backwards compatibility layer for existing local v2
  development databases.
- No remote SurrealDB server mode.
- No Surrealist live-debug integration.
- No storage engine rethink. `RocksDb` remains the embedded engine for this
  upgrade.
- No graph-memory schema redesign beyond fixes required by v3 behavior.

## Constraints

- Noema remains an embedded-store application: the daemon is the only process
  that opens `${NOEMA_HOME:-$HOME/.noema}/db`.
- Pre-stable schema changes may rewrite local data and local development
  databases may be deleted.
- The current store boundary should stay focused around `NoemaStore`,
  `StoreConfig`, strict schema bootstrap, and repository methods.
- Any dependency upgrade fallout should be fixed at the smallest surface that
  keeps the current architecture intact.

## Current Usage Surface

Noema uses a small subset of the SurrealDB Rust API:

- `surrealdb::Surreal`
- `surrealdb::engine::local::{Db, RocksDb}`
- `Surreal::new::<RocksDb>(path)`
- `use_ns("noema").use_db("main")`
- `query(...).await?.check()?`
- `select`, `create`, and typed query response handling through store modules

The strict schema currently depends on these SurrealQL constructs:

- `DEFINE TABLE/FIELD/INDEX IF NOT EXISTS`
- `SCHEMAFULL`
- `TYPE option<T>`
- `FLEXIBLE TYPE object` and `FLEXIBLE TYPE array`
- `ASSERT`
- `array::all(..., |$value| ...)`
- `UPSERT`
- `CREATE type::thing(...)`
- Unique indexes over string IDs and optional fields

The upgrade should assume these are the primary risk points.

## Recommended Approach

Use a strict dependency upgrade with focused compile and runtime repair.

1. Update the workspace `surrealdb` dependency to v3 stable.
2. Regenerate `Cargo.lock`.
3. Compile the workspace.
4. Fix only API, feature, or SurrealQL behavior changes exposed by the compiler
   or tests.
5. Run the full Rust validation suite.
6. Update docs/context only if the upgrade changes developer workflow or local
   database reset expectations.

This keeps the upgrade mechanical and avoids mixing it with new storage
features.

## Data Compatibility

No backwards compatibility is required.

If a v3 embedded open fails against an existing local v2 RocksDB directory,
developers should delete the local development database and restart Noema:

```bash
rm -rf .noema-dev/db
```

For the default home:

```bash
rm -rf "${NOEMA_HOME:-$HOME/.noema}/db"
```

This is acceptable because Noema is pre-stable and the user explicitly approved
tearing down local test databases for schema/runtime changes.

## Risk Areas

### Embedded Store Open

`NoemaStore::open` currently creates the DB directory, opens `RocksDb`, selects
namespace/database, applies strict schema, and validates the schema marker.

The upgrade must verify this still works for a fresh store and a reopened fresh
store.

### Strict Schema Bootstrap

SurrealDB v3 parser or schema semantics may reject existing bootstrap SQL. The
most likely candidates are assertions, flexible object/array fields, optional
datetime fields, and uniqueness over optional values.

The store tests that insert invalid values should remain the first signal for
these changes.

### Graph Claim Writes

Claim creation, reinforcement, evidence insertion, entity upsert, predicate
seeding, and dedupe behavior all rely on strict schema plus deterministic IDs.
These paths must remain source-equivalent after the dependency upgrade.

### Retrieval

Graph-claim retrieval currently uses simple fact/hint text matching and
predicate `use_mode` gates. It must continue to return approved claims while
reporting redacted policy omissions.

### Daemon and GraphQL

The daemon and GraphQL schema should continue to use `NoemaStore` as the store
boundary. The CLI memory inspection path should keep going through GraphQL,
not through raw SurrealDB access.

## Validation

Run the standard Rust validation suite:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Also run these focused scans or checks:

```bash
cargo tree -p noema-core | rg "surrealdb"
rg -n "surrealdb = " Cargo.toml Cargo.lock
```

Expected dependency result after upgrade:

- `surrealdb v3.x` on the stable line
- `surrealdb-core v3.x`
- `kv-rocksdb` still enabled

## Implementation Notes

The implementation should not manually edit `Cargo.lock` except through Cargo.
If the lockfile tries to resolve `3.2.0-beta.2`, pin stable explicitly during
update and keep the manifest on `version = "3"` unless Cargo requires a more
specific stable constraint.

If v3 introduces a local engine versioning requirement, prefer the minimal
change that keeps the embedded RocksDB store owned by Noema. Do not introduce a
server process or remote client mode as part of this upgrade.

## Success Criteria

- Workspace dependency is on stable SurrealDB v3.
- Fresh embedded RocksDB store opens under `NOEMA_HOME/db`.
- Strict schema bootstrap succeeds.
- Provider accounts, conversations, conversation items, graph claims, evidence,
  retrieval, daemon startup, GraphQL memory inspection, and CLI memory
  inspection tests pass.
- Docs mention local DB teardown only if needed by observed behavior.
- No new compatibility code is added for v2 database files.
