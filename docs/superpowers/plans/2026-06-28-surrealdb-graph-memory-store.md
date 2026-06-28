# SurrealDB Graph Memory Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Postgres/SQLx persistence with an embedded SurrealDB store at `NOEMA_HOME/db`, and make durable memory strict graph claims with predicate ontology, provenance evidence relations, deterministic retrieval, and retrieval packets.

**Architecture:** Add a server-owned `NoemaStore` backed by embedded SurrealDB/RocksDB, then port existing provider, conversation, memory, retrieval, inspection, daemon, GraphQL, and CLI paths onto it. Keep the repository boundary domain-focused rather than backend-portable; SurrealDB candidate generation feeds a Rust policy engine, and Postgres code is removed after the replacement is complete.

**Tech Stack:** Rust, Tokio, SurrealDB Rust SDK with `kv-rocksdb`, serde/serde_json, ring digest, async-graphql, existing Noema daemon/runtime/web/CLI modules.

---

## Scope Check

This plan intentionally covers the full storage replacement because the approved design rejects an incremental dual-store architecture. The work is large, so each task has its own tests and commit. If a task uncovers an embedded SurrealDB blocker, stop after the failing task and document the blocker in `docs/context/current.md` before changing direction.

## File Structure

- Modify `Cargo.toml`: add `surrealdb` workspace dependency with `kv-rocksdb`; remove `sqlx` after Postgres code is gone.
- Modify `crates/noema-core/Cargo.toml`: use `surrealdb.workspace = true`.
- Delete `crates/noema-core/src/database.rs`: remove Postgres URL configuration.
- Modify `crates/noema-core/src/lib.rs`: export `NoemaStore`, store errors, graph-memory types, and remove `PostgresMemoryRepository`.
- Modify `crates/noema-core/src/paths.rs`: use `NoemaPaths::db_dir()` as the embedded database path and remove `postgres_data_dir`.
- Modify `crates/noema-core/src/config.rs`: remove `database.url`, `NOEMA_DATABASE_URL`, and `DatabaseConfig` from daemon config.
- Create `crates/noema-core/src/store.rs`: top-level store module exports.
- Create `crates/noema-core/src/store/error.rs`: `StoreError` and conversions.
- Create `crates/noema-core/src/store/runtime.rs`: embedded SurrealDB open/bootstrap runtime and `NoemaStore`.
- Create `crates/noema-core/src/store/schema.rs`: schemafull SurrealQL bootstrap and built-in predicate seed.
- Create `crates/noema-core/src/store/ids.rs`: stable ID allocation helpers.
- Create `crates/noema-core/src/store/objects.rs`: object refs, actor refs, entity refs, and record-id helpers.
- Create `crates/noema-core/src/store/conversations.rs`: humans, agents, provider accounts, conversations, turns, and items.
- Create `crates/noema-core/src/store/ontology.rs`: entity, predicate, and predicate proposal records.
- Create `crates/noema-core/src/store/claims.rs`: claim writes, evidence relations, reinforcement, supersession, and summaries.
- Create `crates/noema-core/src/store/retrieval.rs`: candidate loading, deterministic policy, retrieval packets, and memory-tool formatting support.
- Create `crates/noema-core/src/store/inspection.rs`: graph inspection read model.
- Create `crates/noema-core/src/store/tests.rs`: embedded store integration tests using temporary homes.
- Modify `crates/noema-core/src/memory/model.rs`: replace fine-grained `Purpose` with coarse `UseMode` and align retrieval structs with claims.
- Modify `crates/noema-core/src/memory/store.rs`: evaluate claim retrieval policy instead of Postgres memory rows.
- Modify `crates/noema-core/src/memory/tests.rs`: cover `UseMode` and claim policy gates.
- Modify `crates/noema-core/src/memory_persistence.rs` and delete `crates/noema-core/src/memory_persistence/*`: replace module users with `store`.
- Delete `crates/noema-core/src/postgres_memory_retrieval.rs` and `crates/noema-core/src/postgres_retrieval_policy_fingerprint.rs`.
- Modify `crates/noema-core/src/daemon/protocol.rs`: remove `database_url` from `DaemonServerConfig`.
- Modify `crates/noema-core/src/daemon/server.rs`: open one `NoemaStore` and pass clones to runtime and web state.
- Modify `crates/noema-core/src/daemon/runtime.rs`: use `NoemaStore`, claim candidates, evidence relations, and retrieval packets.
- Modify `crates/noema-core/src/daemon/memory_pipeline.rs`: convert extraction proposals into graph claim candidates.
- Modify `crates/noema-core/src/daemon/memory_consolidation.rs`: consolidate claims by fingerprint and evidence.
- Modify `crates/noema-core/src/daemon/memory_tool.rs`: accept `use_mode`, call claim retrieval, and return claim results.
- Modify `crates/noema-core/src/daemon/web/mod.rs`: replace `PostgresMemoryRepository` with `NoemaStore`.
- Modify `crates/noema-core/src/graphql/schema.rs`, `resolvers.rs`, and `types.rs`: keep existing field names stable unless their stored data shape is replaced by graph claims; expose graph memory details only through explicit graph/inspection fields.
- Modify `crates/noema-cli/src/inspection.rs`, `context_graph_output.rs`, `context_graph_text.rs`, and tests: inspect claims, predicates, evidence, and retrieval packets.
- Modify docs: `docs/project.md`, `docs/memory.md`, `docs/postgres.md`, `docs/sqlite.md`, `docs/harness.md`, `docs/context/current.md`, and stale plan/spec references where they describe active storage.

## Task 1: Add Embedded SurrealDB Runtime

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/paths.rs`
- Create: `crates/noema-core/src/store.rs`
- Create: `crates/noema-core/src/store/error.rs`
- Create: `crates/noema-core/src/store/runtime.rs`
- Create: `crates/noema-core/src/store/schema.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add the SurrealDB dependency**

Update workspace dependencies in `Cargo.toml`:

```toml
surrealdb = { version = "2", default-features = false, features = ["kv-rocksdb"] }
```

Update `crates/noema-core/Cargo.toml`:

```toml
surrealdb.workspace = true
```

- [ ] **Step 2: Add embedded DB path tests**

Modify `crates/noema-core/src/paths.rs` tests so `db_dir()` is the canonical embedded database path and `postgres_data_dir()` is no longer expected:

```rust
#[test]
fn db_dir_is_the_embedded_database_root() {
    let paths = NoemaPaths::from_noema_home("/tmp/custom-noema").expect("paths");

    assert_eq!(paths.db_dir(), PathBuf::from("/tmp/custom-noema/db"));
}
```

Remove assertions that call `postgres_data_dir()`.

- [ ] **Step 3: Run path tests and confirm the obsolete helper is still present**

Run:

```bash
cargo test -p noema-core paths --no-fail-fast
rg "postgres_data_dir" crates/noema-core/src/paths.rs
```

Expected: path tests PASS and `rg` prints the existing `postgres_data_dir` method before the next step removes it.

- [ ] **Step 4: Remove `postgres_data_dir`**

In `crates/noema-core/src/paths.rs`, delete:

```rust
/// Path to the local Postgres data directory.
#[must_use]
pub fn postgres_data_dir(&self) -> PathBuf {
    self.db_dir().join("postgres")
}
```

Keep `db_dir()` returning `self.root.join("db")`.

- [ ] **Step 5: Create the store module skeleton**

Create `crates/noema-core/src/store.rs`:

```rust
//! Embedded SurrealDB-backed canonical Noema store.

mod error;
mod runtime;
mod schema;

#[cfg(test)]
mod tests;

pub use error::StoreError;
pub use runtime::{NoemaStore, StoreConfig};
```

Create `crates/noema-core/src/store/error.rs`:

```rust
use thiserror::Error;

/// Errors produced by the embedded canonical store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Store path could not be prepared.
    #[error("failed to prepare store path: {0}")]
    PreparePath(std::io::Error),
    /// Embedded SurrealDB operation failed.
    #[error("embedded store operation failed: {0}")]
    Surreal(#[from] surrealdb::Error),
    /// Schema bootstrap returned an invalid result.
    #[error("store schema bootstrap failed: {0}")]
    Schema(String),
}
```

Create `crates/noema-core/src/store/schema.rs`:

```rust
/// Namespace used by the embedded Noema store.
pub const NOEMA_NAMESPACE: &str = "noema";

/// Database used by the embedded Noema store.
pub const NOEMA_DATABASE: &str = "main";

/// Current schema version for pre-stable local data.
pub const STORE_SCHEMA_VERSION: i64 = 1;

/// Minimal bootstrap used by the first store-runtime test.
pub const STORE_SCHEMA_SQL: &str = r#"
DEFINE TABLE schema_state SCHEMAFULL;
DEFINE FIELD version ON TABLE schema_state TYPE int ASSERT $value >= 1;
DEFINE FIELD name ON TABLE schema_state TYPE string;
DEFINE FIELD applied_at ON TABLE schema_state TYPE datetime DEFAULT time::now();
UPSERT schema_state:current SET version = 1, name = 'surreal_graph_store_v1', applied_at = time::now();
"#;
```

Create `crates/noema-core/src/store/runtime.rs`:

```rust
use std::{fs, path::PathBuf};

use surrealdb::{
    Surreal,
    engine::local::{Db, RocksDb},
};

use super::{
    error::StoreError,
    schema::{NOEMA_DATABASE, NOEMA_NAMESPACE, STORE_SCHEMA_SQL},
};

/// Configuration for the embedded Noema store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// Directory used by the embedded database.
    pub path: PathBuf,
}

impl StoreConfig {
    /// Build store config from resolved Noema paths.
    #[must_use]
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self {
            path: paths.db_dir(),
        }
    }
}

/// Server-owned embedded canonical store.
#[derive(Debug, Clone)]
pub struct NoemaStore {
    db: Surreal<Db>,
}

impl NoemaStore {
    /// Open and bootstrap the embedded store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database directory cannot be prepared or
    /// SurrealDB cannot be opened or bootstrapped.
    pub async fn open(config: &StoreConfig) -> Result<Self, StoreError> {
        fs::create_dir_all(&config.path).map_err(StoreError::PreparePath)?;
        let db = Surreal::new::<RocksDb>(&config.path).await?;
        db.use_ns(NOEMA_NAMESPACE).use_db(NOEMA_DATABASE).await?;
        db.query(STORE_SCHEMA_SQL).await?.check()?;
        Ok(Self { db })
    }

    /// Access the embedded SurrealDB client for repository modules.
    #[must_use]
    pub(crate) fn db(&self) -> &Surreal<Db> {
        &self.db
    }

    /// Return the current schema marker version.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the schema marker cannot be queried.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        #[derive(serde::Deserialize)]
        struct Row {
            version: i64,
        }

        let row: Option<Row> = self.db.select(("schema_state", "current")).await?;
        row.map(|row| row.version)
            .ok_or_else(|| StoreError::Schema("missing schema_state:current".to_string()))
    }
}
```

- [ ] **Step 6: Register the store module**

Modify `crates/noema-core/src/lib.rs`:

```rust
/// Embedded canonical structured store.
pub mod store;
```

Add exports:

```rust
pub use store::{NoemaStore, StoreConfig, StoreError};
```

- [ ] **Step 7: Add embedded store runtime tests**

Create `crates/noema-core/src/store/tests.rs`:

```rust
use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

#[tokio::test]
async fn opens_embedded_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(paths.db_dir().exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
}

#[tokio::test]
async fn reopens_existing_embedded_store() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let first = NoemaStore::open(&config).await.expect("first open");
    assert_eq!(first.schema_version().await.expect("first version"), 1);
    drop(first);

    let second = NoemaStore::open(&config).await.expect("second open");
    assert_eq!(second.schema_version().await.expect("second version"), 1);
}
```

- [ ] **Step 8: Run the focused tests**

Run:

```bash
cargo test -p noema-core paths store::tests --no-fail-fast
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock crates/noema-core/Cargo.toml crates/noema-core/src/lib.rs crates/noema-core/src/paths.rs crates/noema-core/src/store.rs crates/noema-core/src/store
git commit -m "feat: add embedded surrealdb store runtime"
```

## Task 2: Remove Database URL Configuration

**Files:**
- Delete: `crates/noema-core/src/database.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/config.rs`
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Test: `crates/noema-core/src/config/tests.rs`
- Test: `crates/noema-core/src/daemon/protocol.rs`

- [ ] **Step 1: Add config tests for daemon config without database URL**

In `crates/noema-core/src/config/tests.rs`, add:

```rust
#[test]
fn daemon_config_does_not_require_database_url() {
    let temp = tempfile::tempdir().expect("temp");
    let config_path = temp.path().join("config.yaml");
    std::fs::write(
        &config_path,
        r#"
provider: codex
codex:
  model: gpt-5.1
"#,
    )
    .expect("write config");

    let resolved = crate::Config::load_daemon(Some(config_path), crate::CliOverrides::default())
        .expect("daemon config");

    assert_eq!(resolved.codex.default_model.as_deref(), Some("gpt-5.1"));
}
```

- [ ] **Step 2: Replace daemon protocol database-url test**

In `crates/noema-core/src/daemon/protocol.rs`, replace `daemon_server_config_keeps_database_url_explicit` with:

```rust
#[test]
fn daemon_server_config_uses_embedded_store_path_from_noema_home() {
    let config = DaemonServerConfig::new(
        PathBuf::from("/tmp/noema.sock"),
        CodexProviderConfig::default(),
        WebConfig::default(),
    );

    assert_eq!(config.socket_path, PathBuf::from("/tmp/noema.sock"));
}
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
cargo test -p noema-core config::tests::daemon_config_does_not_require_database_url daemon::protocol::tests::daemon_server_config_uses_embedded_store_path_from_noema_home --no-fail-fast
```

Expected: FAIL because daemon config still requires `NOEMA_DATABASE_URL` and `DaemonServerConfig::new` still takes `database_url`.

- [ ] **Step 4: Remove database config from `config.rs`**

Modify `crates/noema-core/src/config.rs`:

- Remove `DatabaseConfig` and `NOEMA_DATABASE_URL_ENV` imports.
- Remove `"database.url"` from `CONFIG_ENV_KEYS`.
- Remove `database: DatabaseConfig` from `DaemonResolvedConfig`.
- Remove `database: RawDatabaseConfig` from `RawConfig`.
- Remove `RawDatabaseConfig` and `FileDatabaseConfig`.
- Remove `resolve_database_config`.
- In `resolve_daemon_config`, return:

```rust
Ok(DaemonResolvedConfig {
    codex,
    web: self.web,
})
```

- Remove `ConfigError::MissingDatabaseUrl` and `ConfigError::Database`.

- [ ] **Step 5: Remove daemon database URL field**

Modify `crates/noema-core/src/daemon/protocol.rs`:

```rust
pub struct DaemonServerConfig {
    /// Unix socket path to bind.
    pub socket_path: PathBuf,
    /// Codex provider configuration used by daemon conversations.
    pub codex: CodexProviderConfig,
    /// Local web UI configuration.
    pub web: WebConfig,
}

impl DaemonServerConfig {
    #[must_use]
    pub fn new(socket_path: PathBuf, codex: CodexProviderConfig, web: WebConfig) -> Self {
        Self {
            socket_path,
            codex,
            web,
        }
    }
}
```

- [ ] **Step 6: Update daemon server store initialization**

In `crates/noema-core/src/daemon/server.rs`, replace `database_url` handling with:

```rust
let paths = crate::NoemaPaths::from_process_env()?;
let store_config = crate::StoreConfig::from_paths(&paths);
let store = crate::NoemaStore::open(&store_config).await?;
let runtime = CodexRuntimeHandle::spawn(config.codex, store.clone()).await?;
let web_store = store.clone();
web_store.ensure_default_provider_account().await?;
```

Pass `web_store` to `WebState::new`.

- [ ] **Step 7: Delete `database.rs` exports**

Delete `crates/noema-core/src/database.rs`. In `crates/noema-core/src/lib.rs`, remove:

```rust
pub mod database;
pub use database::{DatabaseConfig, DatabaseConfigError, NOEMA_DATABASE_URL_ENV};
```

- [ ] **Step 8: Add daemon error conversion**

In `crates/noema-core/src/daemon/protocol.rs`, replace `Database(#[from] DatabaseConfigError)` with:

```rust
/// Embedded store failed.
#[error(transparent)]
Store(#[from] crate::StoreError),
```

- [ ] **Step 9: Run config and protocol tests**

Run:

```bash
cargo test -p noema-core config daemon::protocol store::tests --no-fail-fast
```

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/noema-core/src/config.rs crates/noema-core/src/config/tests.rs crates/noema-core/src/daemon/protocol.rs crates/noema-core/src/daemon/server.rs crates/noema-core/src/lib.rs
git add -u crates/noema-core/src/database.rs
git commit -m "refactor: remove postgres database configuration"
```

## Task 3: Bootstrap Strict SurrealDB Schema

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Create: `crates/noema-core/src/store/objects.rs`
- Create: `crates/noema-core/src/store/ids.rs`
- Modify: `crates/noema-core/src/store.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add schema validation tests**

Add to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn strict_schema_rejects_invalid_sensitivity() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect("open store");

    let result = store
        .db()
        .query(
            r#"
            CREATE claim:test SET
              subject = entity:test_subject,
              object = entity:test_object,
              predicate = predicate:likes,
              fact = 'Kevin likes trains.',
              status = 'active',
              sensitivity = 'galaxy',
              valid_from = NONE,
              valid_to = NONE,
              confidence = 1.0,
              dedupe_fingerprint = 'bad';
            "#,
        )
        .await;

    assert!(result.is_err(), "invalid sensitivity must fail schema assertions");
}

#[tokio::test]
async fn built_in_personal_predicates_are_seeded() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect("open store");

    #[derive(serde::Deserialize)]
    struct PredicateRow {
        label: String,
        default_sensitivity: String,
    }

    let row: Option<PredicateRow> = store
        .db()
        .select(("predicate", "likes"))
        .await
        .expect("select predicate");

    let row = row.expect("predicate:likes");
    assert_eq!(row.label, "likes");
    assert_eq!(row.default_sensitivity, "normal");
}
```

- [ ] **Step 2: Run tests to verify schema is too small**

Run:

```bash
cargo test -p noema-core store::tests::strict_schema_rejects_invalid_sensitivity store::tests::built_in_personal_predicates_are_seeded --no-fail-fast
```

Expected: FAIL because `claim` and `predicate:likes` are not defined yet.

- [ ] **Step 3: Add ID and object helpers**

Create `crates/noema-core/src/store/ids.rs`:

```rust
use ring::digest;

/// Produce a stable lowercase hex digest prefixed for a Noema record id.
#[must_use]
pub fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut context = digest::Context::new(&digest::SHA256);
    for part in parts {
        context.update(part.as_bytes());
        context.update(b"\0");
    }
    let digest = context.finish();
    let mut out = String::with_capacity(prefix.len() + 1 + 24);
    out.push_str(prefix);
    out.push('_');
    for byte in digest.as_ref().iter().take(12) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
```

Create `crates/noema-core/src/store/objects.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Concrete object reference used by repository APIs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectRef {
    /// Table or object kind.
    pub object_type: String,
    /// Stable object id.
    pub object_id: String,
}

impl ObjectRef {
    /// Build an object reference.
    #[must_use]
    pub fn new(object_type: impl Into<String>, object_id: impl Into<String>) -> Self {
        Self {
            object_type: object_type.into(),
            object_id: object_id.into(),
        }
    }
}

/// Actor reference used by provenance and runtime policy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorRef {
    /// Actor table or kind.
    pub actor_type: String,
    /// Stable actor id.
    pub actor_id: String,
}

impl ActorRef {
    /// Local human actor.
    #[must_use]
    pub fn human(id: impl Into<String>) -> Self {
        Self {
            actor_type: "human".to_string(),
            actor_id: id.into(),
        }
    }

    /// Agent actor.
    #[must_use]
    pub fn agent(id: impl Into<String>) -> Self {
        Self {
            actor_type: "agent".to_string(),
            actor_id: id.into(),
        }
    }
}
```

Register modules in `crates/noema-core/src/store.rs`:

```rust
mod ids;
mod objects;
```

Export:

```rust
pub use objects::{ActorRef, ObjectRef};
```

- [ ] **Step 4: Replace schema bootstrap with strict tables**

Replace `STORE_SCHEMA_SQL` in `crates/noema-core/src/store/schema.rs` with schemafull definitions for these tables:

```surql
DEFINE TABLE schema_state SCHEMAFULL;
DEFINE FIELD version ON TABLE schema_state TYPE int ASSERT $value >= 1;
DEFINE FIELD name ON TABLE schema_state TYPE string;
DEFINE FIELD applied_at ON TABLE schema_state TYPE datetime DEFAULT time::now();

DEFINE TABLE human SCHEMAFULL;
DEFINE FIELD handle ON TABLE human TYPE string;
DEFINE FIELD display_name ON TABLE human TYPE option<string>;
DEFINE FIELD primary_conversation ON TABLE human TYPE option<record<conversation>>;
DEFINE FIELD created_at ON TABLE human TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE human TYPE datetime DEFAULT time::now();

DEFINE TABLE agent SCHEMAFULL;
DEFINE FIELD handle ON TABLE agent TYPE string;
DEFINE FIELD display_name ON TABLE agent TYPE option<string>;
DEFINE FIELD created_at ON TABLE agent TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE agent TYPE datetime DEFAULT time::now();

DEFINE TABLE provider_account SCHEMAFULL;
DEFINE FIELD provider_kind ON TABLE provider_account TYPE string;
DEFINE FIELD account_key ON TABLE provider_account TYPE string;
DEFINE FIELD display_name ON TABLE provider_account TYPE option<string>;
DEFINE FIELD auth_method ON TABLE provider_account TYPE string ASSERT $value INSIDE ['oauth_device_code','secret_input','external_manual','none'];
DEFINE FIELD status ON TABLE provider_account TYPE string ASSERT $value INSIDE ['unknown','checking','authenticated','unauthenticated','unavailable'];
DEFINE FIELD metadata ON TABLE provider_account TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE provider_account TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE provider_account TYPE datetime DEFAULT time::now();
DEFINE INDEX provider_account_unique ON TABLE provider_account COLUMNS provider_kind, account_key UNIQUE;

DEFINE TABLE conversation SCHEMAFULL;
DEFINE FIELD owner ON TABLE conversation TYPE record<human>;
DEFINE FIELD primary_human ON TABLE conversation TYPE record<human>;
DEFINE FIELD primary_agent ON TABLE conversation TYPE record<agent>;
DEFINE FIELD title ON TABLE conversation TYPE string;
DEFINE FIELD model ON TABLE conversation TYPE option<string>;
DEFINE FIELD cwd ON TABLE conversation TYPE option<string>;
DEFINE FIELD lifecycle_status ON TABLE conversation TYPE string ASSERT $value INSIDE ['active','archived','deleted'];
DEFINE FIELD agent_status ON TABLE conversation TYPE string ASSERT $value INSIDE ['idle','input_received','thinking','tool_running','waiting_for_previous_turn_completion','interrupting','error'];
DEFINE FIELD metadata ON TABLE conversation TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE conversation TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE conversation TYPE datetime DEFAULT time::now();

DEFINE TABLE conversation_turn SCHEMAFULL;
DEFINE FIELD conversation ON TABLE conversation_turn TYPE record<conversation>;
DEFINE FIELD trigger_item ON TABLE conversation_turn TYPE option<record<conversation_item>>;
DEFINE FIELD status ON TABLE conversation_turn TYPE string ASSERT $value INSIDE ['input_received','running','waiting_for_tool','interrupted','completed','failed','cancelled'];
DEFINE FIELD turn_index ON TABLE conversation_turn TYPE int ASSERT $value >= 1;
DEFINE FIELD metadata ON TABLE conversation_turn TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE conversation_turn TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE conversation_turn TYPE datetime DEFAULT time::now();

DEFINE TABLE conversation_item SCHEMAFULL;
DEFINE FIELD conversation ON TABLE conversation_item TYPE record<conversation>;
DEFINE FIELD turn ON TABLE conversation_item TYPE option<record<conversation_turn>>;
DEFINE FIELD parent_item ON TABLE conversation_item TYPE option<record<conversation_item>>;
DEFINE FIELD kind ON TABLE conversation_item TYPE string ASSERT $value INSIDE ['user_text','assistant_text','activity','a2ui_card','tool_call','tool_result','approval_request','approval_result','error_notice'];
DEFINE FIELD status ON TABLE conversation_item TYPE string ASSERT $value INSIDE ['pending','running','completed','failed','cancelled','interrupted'];
DEFINE FIELD author_type ON TABLE conversation_item TYPE string ASSERT $value INSIDE ['human','agent','tool','system'];
DEFINE FIELD author_id ON TABLE conversation_item TYPE string;
DEFINE FIELD content_text ON TABLE conversation_item TYPE option<string>;
DEFINE FIELD payload_json ON TABLE conversation_item TYPE object DEFAULT {};
DEFINE FIELD metadata ON TABLE conversation_item TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE conversation_item TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE conversation_item TYPE datetime DEFAULT time::now();

DEFINE TABLE entity SCHEMAFULL;
DEFINE FIELD entity_type ON TABLE entity TYPE string ASSERT $value INSIDE ['human','agent','person','organization','project','workspace','conversation','document','tool','place','task','goal','concept','other'];
DEFINE FIELD canonical_name ON TABLE entity TYPE string;
DEFINE FIELD aliases ON TABLE entity TYPE array<string> DEFAULT [];
DEFINE FIELD linked_object_type ON TABLE entity TYPE option<string>;
DEFINE FIELD linked_object_id ON TABLE entity TYPE option<string>;
DEFINE FIELD metadata ON TABLE entity TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE entity TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE entity TYPE datetime DEFAULT time::now();

DEFINE TABLE predicate SCHEMAFULL;
DEFINE FIELD label ON TABLE predicate TYPE string;
DEFINE FIELD description ON TABLE predicate TYPE string;
DEFINE FIELD allowed_subject_types ON TABLE predicate TYPE array<string>;
DEFINE FIELD allowed_object_types ON TABLE predicate TYPE array<string>;
DEFINE FIELD allowed_use_modes ON TABLE predicate TYPE array<string> ASSERT array::all($value, |$mode| $mode INSIDE ['answer','personalize','plan','act','notify','inspect','export']);
DEFINE FIELD default_sensitivity ON TABLE predicate TYPE string ASSERT $value INSIDE ['public','normal','private','sensitive','secret'];
DEFINE FIELD conflict_policy ON TABLE predicate TYPE string ASSERT $value INSIDE ['allow_many','single_current','mutually_exclusive'];
DEFINE FIELD review_policy ON TABLE predicate TYPE string ASSERT $value INSIDE ['auto_candidate','auto_active','requires_review'];
DEFINE FIELD extraction_hints ON TABLE predicate TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE predicate TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE predicate TYPE datetime DEFAULT time::now();

DEFINE TABLE predicate_proposal SCHEMAFULL;
DEFINE FIELD label ON TABLE predicate_proposal TYPE string;
DEFINE FIELD description ON TABLE predicate_proposal TYPE string;
DEFINE FIELD proposed_predicate ON TABLE predicate_proposal TYPE object;
DEFINE FIELD status ON TABLE predicate_proposal TYPE string ASSERT $value INSIDE ['candidate','approved','rejected','merged'];
DEFINE FIELD source ON TABLE predicate_proposal TYPE option<record<conversation_item>>;
DEFINE FIELD created_at ON TABLE predicate_proposal TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE predicate_proposal TYPE datetime DEFAULT time::now();

DEFINE TABLE claim SCHEMAFULL;
DEFINE FIELD subject ON TABLE claim TYPE record<entity>;
DEFINE FIELD object ON TABLE claim TYPE record<entity>;
DEFINE FIELD predicate ON TABLE claim TYPE record<predicate>;
DEFINE FIELD fact ON TABLE claim TYPE string;
DEFINE FIELD status ON TABLE claim TYPE string ASSERT $value INSIDE ['candidate','active','confirmed','disputed','superseded','archived','deleted'];
DEFINE FIELD sensitivity ON TABLE claim TYPE string ASSERT $value INSIDE ['public','normal','private','sensitive','secret'];
DEFINE FIELD valid_from ON TABLE claim TYPE option<datetime>;
DEFINE FIELD valid_to ON TABLE claim TYPE option<datetime>;
DEFINE FIELD observed_at ON TABLE claim TYPE option<datetime>;
DEFINE FIELD confidence ON TABLE claim TYPE option<float> ASSERT $value = NONE OR ($value >= 0 AND $value <= 1);
DEFINE FIELD dedupe_fingerprint ON TABLE claim TYPE string;
DEFINE FIELD retrieval_hints ON TABLE claim TYPE object DEFAULT {};
DEFINE FIELD policy_overrides ON TABLE claim TYPE object DEFAULT {};
DEFINE FIELD metadata ON TABLE claim TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE claim TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE claim TYPE datetime DEFAULT time::now();
DEFINE INDEX claim_dedupe_live ON TABLE claim COLUMNS dedupe_fingerprint UNIQUE WHERE status != 'deleted';

DEFINE TABLE supported_by TYPE RELATION FROM claim TO conversation_item SCHEMAFULL;
DEFINE FIELD authority ON TABLE supported_by TYPE string ASSERT $value INSIDE ['human_correction','explicit_human_statement','document_source','repeated_observation','agent_inference','weak_inference','system_rule'];
DEFINE FIELD excerpt ON TABLE supported_by TYPE option<string>;
DEFINE FIELD observed_at ON TABLE supported_by TYPE datetime DEFAULT time::now();
DEFINE FIELD created_by ON TABLE supported_by TYPE string;
DEFINE FIELD metadata ON TABLE supported_by TYPE object DEFAULT {};

DEFINE TABLE corrected_by TYPE RELATION FROM claim TO conversation_item SCHEMAFULL;
DEFINE FIELD authority ON TABLE corrected_by TYPE string ASSERT $value = 'human_correction';
DEFINE FIELD excerpt ON TABLE corrected_by TYPE option<string>;
DEFINE FIELD observed_at ON TABLE corrected_by TYPE datetime DEFAULT time::now();
DEFINE FIELD created_by ON TABLE corrected_by TYPE string;
DEFINE FIELD metadata ON TABLE corrected_by TYPE object DEFAULT {};

DEFINE TABLE contradicted_by TYPE RELATION FROM claim TO conversation_item SCHEMAFULL;
DEFINE FIELD authority ON TABLE contradicted_by TYPE string ASSERT $value INSIDE ['explicit_human_statement','document_source','agent_inference','weak_inference'];
DEFINE FIELD excerpt ON TABLE contradicted_by TYPE option<string>;
DEFINE FIELD observed_at ON TABLE contradicted_by TYPE datetime DEFAULT time::now();
DEFINE FIELD created_by ON TABLE contradicted_by TYPE string;
DEFINE FIELD metadata ON TABLE contradicted_by TYPE object DEFAULT {};

DEFINE TABLE supersedes TYPE RELATION FROM claim TO claim SCHEMAFULL;
DEFINE FIELD reason ON TABLE supersedes TYPE string;
DEFINE FIELD created_at ON TABLE supersedes TYPE datetime DEFAULT time::now();

DEFINE TABLE derived_from TYPE RELATION FROM claim TO claim SCHEMAFULL;
DEFINE FIELD reason ON TABLE derived_from TYPE string;
DEFINE FIELD created_at ON TABLE derived_from TYPE datetime DEFAULT time::now();

DEFINE TABLE retrieval_packet SCHEMAFULL;
DEFINE FIELD run_id ON TABLE retrieval_packet TYPE string;
DEFINE FIELD requesting_agent ON TABLE retrieval_packet TYPE record<agent>;
DEFINE FIELD active_humans ON TABLE retrieval_packet TYPE array<record<human>>;
DEFINE FIELD active_objects ON TABLE retrieval_packet TYPE array<object> DEFAULT [];
DEFINE FIELD use_mode ON TABLE retrieval_packet TYPE string ASSERT $value INSIDE ['answer','personalize','plan','act','notify','inspect','export'];
DEFINE FIELD included_claims ON TABLE retrieval_packet TYPE array<record<claim>> DEFAULT [];
DEFINE FIELD redacted_omissions ON TABLE retrieval_packet TYPE array<object> DEFAULT [];
DEFINE FIELD policy_version ON TABLE retrieval_packet TYPE int ASSERT $value >= 1;
DEFINE FIELD created_at ON TABLE retrieval_packet TYPE datetime DEFAULT time::now();
```

Append built-in predicates:

```surql
UPSERT predicate:likes SET label = 'likes', description = 'Subject likes or enjoys object.', allowed_subject_types = ['human','person'], allowed_object_types = ['concept','place','tool','project','other'], allowed_use_modes = ['answer','personalize','plan'], default_sensitivity = 'normal', conflict_policy = 'allow_many', review_policy = 'auto_active', extraction_hints = { examples: ['Kevin likes trains.'] };
UPSERT predicate:dislikes SET label = 'dislikes', description = 'Subject dislikes or avoids object.', allowed_subject_types = ['human','person'], allowed_object_types = ['concept','place','tool','project','other'], allowed_use_modes = ['answer','personalize','plan'], default_sensitivity = 'normal', conflict_policy = 'allow_many', review_policy = 'auto_active', extraction_hints = { examples: ['Kevin dislikes surprise meetings.'] };
UPSERT predicate:prefers SET label = 'prefers', description = 'Subject prefers object or approach.', allowed_subject_types = ['human','person'], allowed_object_types = ['concept','tool','project','other'], allowed_use_modes = ['answer','personalize','plan'], default_sensitivity = 'normal', conflict_policy = 'allow_many', review_policy = 'auto_active', extraction_hints = { examples: ['Kevin prefers direct implementation updates.'] };
UPSERT predicate:uses SET label = 'uses', description = 'Subject uses object as a tool, system, or practice.', allowed_subject_types = ['human','person','agent'], allowed_object_types = ['tool','concept','other'], allowed_use_modes = ['answer','personalize','plan','act'], default_sensitivity = 'normal', conflict_policy = 'allow_many', review_policy = 'auto_candidate', extraction_hints = { examples: ['Kevin uses Codex for Noema development.'] };
UPSERT predicate:works_on SET label = 'works_on', description = 'Subject works on object.', allowed_subject_types = ['human','person','agent'], allowed_object_types = ['project','concept','other'], allowed_use_modes = ['answer','personalize','plan'], default_sensitivity = 'normal', conflict_policy = 'allow_many', review_policy = 'auto_active', extraction_hints = { examples: ['Kevin works on Noema.'] };
UPSERT predicate:prefers_interaction_style SET label = 'prefers_interaction_style', description = 'Subject prefers a style of interaction.', allowed_subject_types = ['human','person'], allowed_object_types = ['concept','other'], allowed_use_modes = ['answer','personalize'], default_sensitivity = 'normal', conflict_policy = 'allow_many', review_policy = 'auto_active', extraction_hints = { examples: ['Kevin prefers direct engineering prose.'] };
```

- [ ] **Step 5: Run schema tests**

Run:

```bash
cargo test -p noema-core store::tests --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/store.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/ids.rs crates/noema-core/src/store/objects.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: bootstrap strict surrealdb schema"
```

## Task 4: Port Provider Accounts And Core Conversation Persistence

**Files:**
- Create: `crates/noema-core/src/store/conversations.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Test: `crates/noema-core/src/store/tests.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add store API tests for provider accounts and conversations**

Add tests to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn default_provider_account_round_trips() {
    let store = temp_store().await;

    store.ensure_default_provider_account().await.expect("ensure default");
    let account = store
        .active_provider_account("codex")
        .await
        .expect("active account")
        .expect("codex account");

    assert_eq!(account.provider_kind, "codex");
    assert_eq!(account.account_key, "default");
}

#[tokio::test]
async fn primary_conversation_is_reused() {
    let store = temp_store().await;
    store.ensure_default_actors().await.expect("default actors");

    let first = store
        .get_or_create_primary_conversation("human:local", Some("gpt-5.1".to_string()), None)
        .await
        .expect("first conversation");
    let second = store
        .get_or_create_primary_conversation("human:local", Some("gpt-5.1".to_string()), None)
        .await
        .expect("second conversation");

    assert_eq!(first.conversation_id, second.conversation_id);
}
```

Add helper at the bottom of the test module:

```rust
async fn temp_store() -> NoemaStore {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.into_path()).expect("paths");
    NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect("open store")
}
```

- [ ] **Step 2: Run tests to verify missing methods**

Run:

```bash
cargo test -p noema-core store::tests::default_provider_account_round_trips store::tests::primary_conversation_is_reused --no-fail-fast
```

Expected: FAIL because `NoemaStore` does not expose provider account or conversation methods.

- [ ] **Step 3: Create conversation/provider record types**

Create `crates/noema-core/src/store/conversations.rs` with these public record names preserved from the current provider/conversation API:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{StoreError, ids::stable_id, runtime::NoemaStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthMethod {
    OauthDeviceCode,
    SecretInput,
    ExternalManual,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAccountStatus {
    Unknown,
    Checking,
    Authenticated,
    Unauthenticated,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAccountRecord {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub display_name: Option<String>,
    pub auth_method: ProviderAuthMethod,
    pub status: ProviderAccountStatus,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    InputReceived,
    Thinking,
    ToolRunning,
    WaitingForPreviousTurnCompletion,
    Interrupting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub conversation_id: String,
    pub title: String,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub agent_status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConversation {
    pub model: Option<String>,
    pub cwd: Option<String>,
}

impl NewConversation {
    #[must_use]
    pub fn local_chat(model: Option<String>, cwd: Option<String>) -> Self {
        Self { model, cwd }
    }
}
```

- [ ] **Step 4: Implement provider account and conversation methods**

Add an `impl NoemaStore` block in `conversations.rs` with these methods:

```rust
impl NoemaStore {
    pub async fn ensure_default_actors(&self) -> Result<(), StoreError> {
        self.db()
            .query(
                r#"
                UPSERT human:local SET handle = 'local', display_name = 'Local human', updated_at = time::now();
                UPSERT agent:primary SET handle = 'primary', display_name = 'Primary agent', updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        Ok(())
    }

    pub async fn ensure_default_provider_account(&self) -> Result<(), StoreError> {
        self.db()
            .query(
                r#"
                UPSERT provider_account:codex_default SET
                  provider_kind = 'codex',
                  account_key = 'default',
                  display_name = 'Codex',
                  auth_method = 'oauth_device_code',
                  status = 'unknown',
                  metadata = {},
                  updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        Ok(())
    }

    pub async fn active_provider_account(
        &self,
        provider_kind: &str,
    ) -> Result<Option<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT
                  id.id() AS provider_account_id,
                  provider_kind,
                  account_key,
                  display_name,
                  auth_method,
                  status,
                  metadata
                FROM provider_account
                WHERE provider_kind = $provider_kind
                ORDER BY created_at ASC
                LIMIT 1;
                "#,
            )
            .bind(("provider_kind", provider_kind.to_string()))
            .await?;
        let rows: Vec<ProviderAccountRecord> = response.take(0)?;
        Ok(rows.into_iter().next())
    }

    pub async fn get_or_create_primary_conversation(
        &self,
        human_id: &str,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<ConversationRecord, StoreError> {
        self.ensure_default_actors().await?;
        let existing: Option<ConversationRecord> = self
            .db()
            .query(
                r#"
                SELECT
                  primary_conversation.id() AS conversation_id,
                  primary_conversation.title AS title,
                  primary_conversation.model AS model,
                  primary_conversation.cwd AS cwd,
                  primary_conversation.agent_status AS agent_status
                FROM type::thing('human', $human_id)
                FETCH primary_conversation;
                "#,
            )
            .bind(("human_id", human_id.trim_start_matches("human:").to_string()))
            .await?
            .take::<Vec<ConversationRecord>>(0)?
            .into_iter()
            .next();
        if let Some(existing) = existing {
            return Ok(existing);
        }

        let conversation_id = stable_id("conversation", &[human_id, "primary"]);
        let mut response = self
            .db()
            .query(
                r#"
                UPSERT type::thing('conversation', $conversation_id) SET
                  owner = type::thing('human', $human_id),
                  primary_human = type::thing('human', $human_id),
                  primary_agent = agent:primary,
                  title = 'Home',
                  model = $model,
                  cwd = $cwd,
                  lifecycle_status = 'active',
                  agent_status = 'idle',
                  metadata = {},
                  updated_at = time::now();
                UPDATE type::thing('human', $human_id) SET primary_conversation = type::thing('conversation', $conversation_id), updated_at = time::now();
                SELECT id.id() AS conversation_id, title, model, cwd, agent_status FROM type::thing('conversation', $conversation_id);
                "#,
            )
            .bind(("conversation_id", conversation_id))
            .bind(("human_id", human_id.trim_start_matches("human:").to_string()))
            .bind(("model", model))
            .bind(("cwd", cwd))
            .await?;
        let rows: Vec<ConversationRecord> = response.take(2)?;
        rows.into_iter()
            .next()
            .ok_or_else(|| StoreError::Schema("primary conversation was not returned".to_string()))
    }
}
```

- [ ] **Step 5: Register and export conversation types**

Modify `crates/noema-core/src/store.rs`:

```rust
mod conversations;

pub use conversations::{
    AgentStatus, ConversationRecord, NewConversation, ProviderAccountRecord,
    ProviderAccountStatus, ProviderAuthMethod,
};
```

- [ ] **Step 6: Run focused store tests**

Run:

```bash
cargo test -p noema-core store::tests::default_provider_account_round_trips store::tests::primary_conversation_is_reused --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Replace runtime constructor signatures**

Modify `crates/noema-core/src/daemon/runtime.rs`:

- Replace `database_url: String` parameters with `store: crate::NoemaStore`.
- Store `NoemaStore` in `CodexRuntimeActor`.
- Pass `store.clone()` into `MemoryExtractionWorkerHandle::spawn`.
- Remove `DatabaseConfig::new` and `PostgresMemoryRepository::connect`.

The constructor shape should be:

```rust
pub(crate) async fn spawn(
    mut codex_config: CodexProviderConfig,
    store: crate::NoemaStore,
) -> Result<Self, DaemonError> {
    let paths = crate::NoemaPaths::from_process_env()?;
    let account_home = paths.provider_account_home("codex", "default");
    crate::provider_auth::ensure_provider_account_home(&account_home)?;
    apply_provider_account_home(&mut codex_config, &account_home);

    let provider = Arc::new(CodexResponsesProvider::new(codex_config)?);
    Self::spawn_with_provider(provider, store).await
}
```

- [ ] **Step 8: Run daemon compile check**

Run:

```bash
cargo check -p noema-core
```

Expected: FAIL on methods not yet ported beyond provider/conversation startup. Record the first missing method group and continue with Task 5.

- [ ] **Step 9: Commit the provider/conversation slice**

```bash
git add crates/noema-core/src/store.rs crates/noema-core/src/store/conversations.rs crates/noema-core/src/store/tests.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/server.rs crates/noema-core/src/daemon/web/mod.rs
git commit -m "feat: port core conversations to surrealdb"
```

## Task 5: Port Conversation Items And Replay

**Files:**
- Modify: `crates/noema-core/src/store/conversations.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Test: `crates/noema-core/src/store/tests.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add conversation item replay tests**

Add to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn conversation_items_replay_in_created_order() {
    let store = temp_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            turn_index: 1,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");

    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: crate::store::ActorRef::human("human:local"),
            content_text: Some("hello".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({ "turn_index": 1 }),
        })
        .await
        .expect("user item");
    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: crate::store::ActorRef::agent("agent:primary"),
            content_text: Some("hi".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({ "turn_index": 1 }),
        })
        .await
        .expect("assistant item");

    let replay = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");

    assert_eq!(replay.len(), 2);
    assert_eq!(replay[0].content_text.as_deref(), Some("hello"));
    assert_eq!(replay[1].content_text.as_deref(), Some("hi"));
}
```

- [ ] **Step 2: Run test to verify missing item APIs**

Run:

```bash
cargo test -p noema-core store::tests::conversation_items_replay_in_created_order --no-fail-fast
```

Expected: FAIL because turn/item types and methods are missing.

- [ ] **Step 3: Add item enums and records**

In `store/conversations.rs`, define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTurnStatus {
    InputReceived,
    Running,
    WaitingForTool,
    Interrupted,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationItemKind {
    UserText,
    AssistantText,
    Activity,
    A2uiCard,
    ToolCall,
    ToolResult,
    ApprovalRequest,
    ApprovalResult,
    ErrorNotice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    All,
    Visible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConversationTurn {
    pub conversation_id: String,
    pub trigger_item_id: Option<String>,
    pub turn_index: u64,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTurnRecord {
    pub turn_id: String,
    pub conversation_id: String,
    pub turn_index: u64,
    pub status: ConversationTurnStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConversationItem {
    pub conversation_id: String,
    pub turn_id: Option<String>,
    pub parent_item_id: Option<String>,
    pub kind: ConversationItemKind,
    pub status: ConversationItemStatus,
    pub author: crate::store::ActorRef,
    pub content_text: Option<String>,
    pub payload_json: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationItemRecord {
    pub item_id: String,
    pub conversation_id: String,
    pub turn_id: Option<String>,
    pub parent_item_id: Option<String>,
    pub kind: ConversationItemKind,
    pub status: ConversationItemStatus,
    pub author_type: String,
    pub author_id: String,
    pub content_text: Option<String>,
    pub payload_json: Value,
    pub metadata: Value,
    pub created_at: String,
}
```

Export these from `store.rs`.

- [ ] **Step 4: Implement turn/item methods**

Add methods to `impl NoemaStore`:

```rust
pub async fn create_conversation_turn(
    &self,
    input: NewConversationTurn,
) -> Result<ConversationTurnRecord, StoreError> {
    let turn_id = stable_id("turn", &[&input.conversation_id, &input.turn_index.to_string()]);
    let mut response = self
        .db()
        .query(
            r#"
            CREATE type::thing('conversation_turn', $turn_id) SET
              conversation = type::thing('conversation', $conversation_id),
              trigger_item = $trigger_item,
              status = 'input_received',
              turn_index = $turn_index,
              metadata = $metadata,
              updated_at = time::now();
            SELECT id.id() AS turn_id, conversation.id() AS conversation_id, turn_index, status
            FROM type::thing('conversation_turn', $turn_id);
            "#,
        )
        .bind(("turn_id", turn_id))
        .bind(("conversation_id", input.conversation_id))
        .bind(("trigger_item", input.trigger_item_id.map(|id| format!("conversation_item:{id}"))))
        .bind(("turn_index", input.turn_index as i64))
        .bind(("metadata", input.metadata))
        .await?;
    let rows: Vec<ConversationTurnRecord> = response.take(1)?;
    rows.into_iter()
        .next()
        .ok_or_else(|| StoreError::Schema("conversation turn was not returned".to_string()))
}

pub async fn append_conversation_item(
    &self,
    input: NewConversationItem,
) -> Result<ConversationItemRecord, StoreError> {
    let discriminator = format!(
        "{}:{}:{}",
        input.conversation_id,
        input.turn_id.clone().unwrap_or_default(),
        input.content_text.clone().unwrap_or_else(|| input.payload_json.to_string())
    );
    let item_id = stable_id("item", &[&discriminator]);
    let mut response = self
        .db()
        .query(
            r#"
            CREATE type::thing('conversation_item', $item_id) SET
              conversation = type::thing('conversation', $conversation_id),
              turn = $turn,
              parent_item = $parent_item,
              kind = $kind,
              status = $status,
              author_type = $author_type,
              author_id = $author_id,
              content_text = $content_text,
              payload_json = $payload_json,
              metadata = $metadata,
              updated_at = time::now();
            SELECT id.id() AS item_id, conversation.id() AS conversation_id, turn.id() AS turn_id,
              parent_item.id() AS parent_item_id, kind, status, author_type, author_id,
              content_text, payload_json, metadata, string::from(created_at) AS created_at
            FROM type::thing('conversation_item', $item_id);
            "#,
        )
        .bind(("item_id", item_id))
        .bind(("conversation_id", input.conversation_id))
        .bind(("turn", input.turn_id.map(|id| format!("conversation_turn:{id}"))))
        .bind(("parent_item", input.parent_item_id.map(|id| format!("conversation_item:{id}"))))
        .bind(("kind", serde_json::to_value(input.kind)?.as_str().unwrap_or("activity").to_string()))
        .bind(("status", serde_json::to_value(input.status)?.as_str().unwrap_or("completed").to_string()))
        .bind(("author_type", input.author.actor_type))
        .bind(("author_id", input.author.actor_id))
        .bind(("content_text", input.content_text))
        .bind(("payload_json", input.payload_json))
        .bind(("metadata", input.metadata))
        .await?;
    let rows: Vec<ConversationItemRecord> = response.take(1)?;
    rows.into_iter()
        .next()
        .ok_or_else(|| StoreError::Schema("conversation item was not returned".to_string()))
}
```

Also implement `list_conversation_items`, `list_recent_conversation_items_for_context`, `next_conversation_turn_index`, and `update_conversation_agent_status` with SurrealQL queries ordered by `created_at ASC`.

- [ ] **Step 5: Run store conversation item tests**

Run:

```bash
cargo test -p noema-core store::tests::conversation_items_replay_in_created_order --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Update runtime and web imports**

Replace imports from `memory_persistence` with `store` for:

- `AgentStatus`
- `ConversationItemKind`
- `ConversationItemRecord`
- `ConversationItemStatus`
- `ConversationRecord`
- `ConversationTurnRecord`
- `ConversationTurnStatus`
- `NewConversation`
- `NewConversationItem`
- `NewConversationTurn`
- `ReplayMode`

- [ ] **Step 7: Run daemon conversation tests**

Run:

```bash
cargo test -p noema-core daemon::tests --no-fail-fast
```

Expected: PASS for conversation persistence tests; memory-specific tests may fail until claim tasks are complete. If tests fail only on memory symbols, proceed to Task 6.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/store/conversations.rs crates/noema-core/src/store.rs crates/noema-core/src/store/tests.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/web/mod.rs
git commit -m "feat: port conversation items to surrealdb"
```

## Task 6: Add Predicate Ontology, Claims, And Evidence

**Files:**
- Create: `crates/noema-core/src/store/ontology.rs`
- Create: `crates/noema-core/src/store/claims.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/store/tests.rs`
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`

- [ ] **Step 1: Add claim write tests**

Add to `store/tests.rs`:

```rust
#[tokio::test]
async fn known_predicate_claim_gets_evidence() {
    let store = temp_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            turn_index: 1,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");
    let source = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id,
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: crate::store::ActorRef::human("human:local"),
            content_text: Some("I like trains.".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source item");

    let claim = store
        .create_or_reinforce_claim(NewClaimCandidate {
            subject: EntityCandidate::local_human(),
            object: EntityCandidate::concept("trains", "trains"),
            predicate_id: "likes".to_string(),
            fact: "Kevin likes trains.".to_string(),
            sensitivity: Sensitivity::Normal,
            status: ClaimStatus::Active,
            confidence: Some(1.0),
            evidence: EvidenceCandidate {
                source_item_id: source.item_id,
                authority: EvidenceAuthority::ExplicitHumanStatement,
                excerpt: Some("I like trains.".to_string()),
            },
            retrieval_hints: serde_json::json!({"keywords": ["trains"]}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("claim");

    assert_eq!(claim.predicate_id, "likes");
    assert_eq!(claim.status, ClaimStatus::Active);
    assert_eq!(claim.evidence_count, 1);
}
```

- [ ] **Step 2: Run test to verify missing graph claim APIs**

Run:

```bash
cargo test -p noema-core store::tests::known_predicate_claim_gets_evidence --no-fail-fast
```

Expected: FAIL because claim candidate types and methods are missing.

- [ ] **Step 3: Add ontology types**

Create `crates/noema-core/src/store/ontology.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Human,
    Agent,
    Person,
    Organization,
    Project,
    Workspace,
    Conversation,
    Document,
    Tool,
    Place,
    Task,
    Goal,
    Concept,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCandidate {
    pub entity_id: String,
    pub entity_type: EntityType,
    pub canonical_name: String,
}

impl EntityCandidate {
    #[must_use]
    pub fn local_human() -> Self {
        Self {
            entity_id: "human_local".to_string(),
            entity_type: EntityType::Human,
            canonical_name: "Kevin".to_string(),
        }
    }

    #[must_use]
    pub fn concept(id_fragment: &str, name: &str) -> Self {
        Self {
            entity_id: format!("concept_{}", id_fragment.replace(|ch: char| !ch.is_ascii_alphanumeric(), "_")),
            entity_type: EntityType::Concept,
            canonical_name: name.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateRecord {
    pub predicate_id: String,
    pub label: String,
    pub default_sensitivity: String,
    pub allowed_use_modes: Vec<String>,
}
```

- [ ] **Step 4: Add claim and evidence types**

Create `crates/noema-core/src/store/claims.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::memory::Sensitivity;

use super::{
    StoreError,
    ids::stable_id,
    ontology::EntityCandidate,
    runtime::NoemaStore,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Candidate,
    Active,
    Confirmed,
    Disputed,
    Superseded,
    Archived,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAuthority {
    HumanCorrection,
    ExplicitHumanStatement,
    DocumentSource,
    RepeatedObservation,
    AgentInference,
    WeakInference,
    SystemRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceCandidate {
    pub source_item_id: String,
    pub authority: EvidenceAuthority,
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewClaimCandidate {
    pub subject: EntityCandidate,
    pub object: EntityCandidate,
    pub predicate_id: String,
    pub fact: String,
    pub sensitivity: Sensitivity,
    pub status: ClaimStatus,
    pub confidence: Option<f64>,
    pub evidence: EvidenceCandidate,
    pub retrieval_hints: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimSummary {
    pub claim_id: String,
    pub subject_entity_id: String,
    pub object_entity_id: String,
    pub predicate_id: String,
    pub fact: String,
    pub status: ClaimStatus,
    pub sensitivity: Sensitivity,
    pub evidence_count: i64,
}
```

- [ ] **Step 5: Implement claim creation and reinforcement**

In `claims.rs`, implement:

```rust
impl NoemaStore {
    pub async fn create_or_reinforce_claim(
        &self,
        candidate: NewClaimCandidate,
    ) -> Result<ClaimSummary, StoreError> {
        let fingerprint = claim_fingerprint(&candidate);
        let subject_id = ensure_entity(self, &candidate.subject).await?;
        let object_id = ensure_entity(self, &candidate.object).await?;
        let mut response = self
            .db()
            .query(
                r#"
                LET $existing = (SELECT id FROM claim WHERE dedupe_fingerprint = $fingerprint AND status != 'deleted' LIMIT 1);
                IF array::len($existing) = 0 THEN {
                    CREATE type::thing('claim', $claim_id) SET
                      subject = type::thing('entity', $subject_id),
                      object = type::thing('entity', $object_id),
                      predicate = type::thing('predicate', $predicate_id),
                      fact = $fact,
                      status = $status,
                      sensitivity = $sensitivity,
                      valid_from = NONE,
                      valid_to = NONE,
                      observed_at = time::now(),
                      confidence = $confidence,
                      dedupe_fingerprint = $fingerprint,
                      retrieval_hints = $retrieval_hints,
                      policy_overrides = {},
                      metadata = $metadata,
                      updated_at = time::now();
                };
                LET $claim = (SELECT id FROM claim WHERE dedupe_fingerprint = $fingerprint AND status != 'deleted' LIMIT 1)[0].id;
                RELATE $claim->supported_by->type::thing('conversation_item', $source_item_id)
                  SET authority = $authority,
                      excerpt = $excerpt,
                      observed_at = time::now(),
                      created_by = 'agent:primary',
                      metadata = {};
                SELECT
                  id.id() AS claim_id,
                  subject.id() AS subject_entity_id,
                  object.id() AS object_entity_id,
                  predicate.id() AS predicate_id,
                  fact,
                  status,
                  sensitivity,
                  array::len(<-supported_by) AS evidence_count
                FROM $claim;
                "#,
            )
            .bind(("fingerprint", fingerprint.clone()))
            .bind(("claim_id", stable_id("claim", &[&fingerprint])))
            .bind(("subject_id", subject_id))
            .bind(("object_id", object_id))
            .bind(("predicate_id", candidate.predicate_id))
            .bind(("fact", candidate.fact))
            .bind(("status", enum_json_string(candidate.status)?))
            .bind(("sensitivity", sensitivity_string(candidate.sensitivity).to_string()))
            .bind(("confidence", candidate.confidence))
            .bind(("retrieval_hints", candidate.retrieval_hints))
            .bind(("metadata", candidate.metadata))
            .bind(("source_item_id", candidate.evidence.source_item_id))
            .bind(("authority", enum_json_string(candidate.evidence.authority)?))
            .bind(("excerpt", candidate.evidence.excerpt))
            .await?;
        let rows: Vec<ClaimSummary> = response.take(3)?;
        rows.into_iter()
            .next()
            .ok_or_else(|| StoreError::Schema("claim summary was not returned".to_string()))
    }
}
```

Add private helpers `ensure_entity`, `claim_fingerprint`, `enum_json_string`, and `sensitivity_string` in the same file. `ensure_entity` should `UPSERT type::thing('entity', $entity_id)` with `entity_type`, `canonical_name`, `aliases = []`, and `metadata = {}`.

- [ ] **Step 6: Register exports**

Modify `store.rs`:

```rust
mod claims;
mod ontology;

pub use claims::{ClaimStatus, ClaimSummary, EvidenceAuthority, EvidenceCandidate, NewClaimCandidate};
pub use ontology::{EntityCandidate, EntityType, PredicateRecord};
```

- [ ] **Step 7: Run claim tests**

Run:

```bash
cargo test -p noema-core store::tests::known_predicate_claim_gets_evidence --no-fail-fast
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/store.rs crates/noema-core/src/store/ontology.rs crates/noema-core/src/store/claims.rs crates/noema-core/src/store/tests.rs crates/noema-core/src/daemon/memory_pipeline.rs
git commit -m "feat: add graph claims and evidence"
```

## Task 7: Replace Retrieval Purpose With Use Mode And Claim Policy

**Files:**
- Modify: `crates/noema-core/src/memory/model.rs`
- Modify: `crates/noema-core/src/memory/store.rs`
- Modify: `crates/noema-core/src/memory/tests.rs`
- Create: `crates/noema-core/src/store/retrieval.rs`
- Modify: `crates/noema-core/src/store.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add use-mode policy tests**

Add to `crates/noema-core/src/memory/tests.rs`:

```rust
#[test]
fn normal_claim_requires_active_human() {
    let request = ClaimRetrievalRequest {
        requesting_agent_id: "agent:primary".to_string(),
        active_human_ids: vec!["human:local".to_string()],
        active_object_ids: Vec::new(),
        use_mode: UseMode::Personalize,
        explicit_memory_request: false,
        sensitivity_ceiling: Sensitivity::Normal,
        approved_secret_access: false,
    };
    let claim = PolicyClaim {
        claim_id: "claim_1".to_string(),
        subject_entity_id: "human:local".to_string(),
        object_entity_id: "concept:trains".to_string(),
        predicate_allowed_use_modes: vec![UseMode::Answer, UseMode::Personalize],
        status: ClaimStatusForPolicy::Active,
        sensitivity: Sensitivity::Normal,
    };

    assert!(claim_policy_allows(&claim, &request).is_ok());
}

#[test]
fn sensitive_claim_requires_unlock() {
    let request = ClaimRetrievalRequest {
        requesting_agent_id: "agent:primary".to_string(),
        active_human_ids: vec!["human:local".to_string()],
        active_object_ids: Vec::new(),
        use_mode: UseMode::Personalize,
        explicit_memory_request: false,
        sensitivity_ceiling: Sensitivity::Sensitive,
        approved_secret_access: false,
    };
    let claim = PolicyClaim {
        claim_id: "claim_1".to_string(),
        subject_entity_id: "human:local".to_string(),
        object_entity_id: "concept:health".to_string(),
        predicate_allowed_use_modes: vec![UseMode::Answer, UseMode::Personalize],
        status: ClaimStatusForPolicy::Active,
        sensitivity: Sensitivity::Sensitive,
    };

    assert_eq!(
        claim_policy_allows(&claim, &request),
        Err(ClaimDenialReason::SensitiveUnlockMissing)
    );
}
```

- [ ] **Step 2: Run tests to verify missing policy types**

Run:

```bash
cargo test -p noema-core memory::tests::normal_claim_requires_active_human memory::tests::sensitive_claim_requires_unlock --no-fail-fast
```

Expected: FAIL because `UseMode`, `PolicyClaim`, and `claim_policy_allows` are missing.

- [ ] **Step 3: Add use-mode and claim policy model**

In `crates/noema-core/src/memory/model.rs`, replace `Purpose`-centered retrieval with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UseMode {
    Answer,
    Personalize,
    Plan,
    Act,
    Notify,
    Inspect,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimStatusForPolicy {
    Candidate,
    Active,
    Confirmed,
    Disputed,
    Superseded,
    Archived,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRetrievalRequest {
    pub requesting_agent_id: String,
    pub active_human_ids: Vec<String>,
    pub active_object_ids: Vec<String>,
    pub use_mode: UseMode,
    pub explicit_memory_request: bool,
    pub sensitivity_ceiling: Sensitivity,
    pub approved_secret_access: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyClaim {
    pub claim_id: String,
    pub subject_entity_id: String,
    pub object_entity_id: String,
    pub predicate_allowed_use_modes: Vec<UseMode>,
    pub status: ClaimStatusForPolicy,
    pub sensitivity: Sensitivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimDenialReason {
    StatusDenied,
    UseModeDenied,
    SensitivityCeiling,
    OutsideActiveHumanContext,
    SensitiveUnlockMissing,
    SecretApprovalMissing,
}
```

Add:

```rust
pub fn claim_policy_allows(
    claim: &PolicyClaim,
    request: &ClaimRetrievalRequest,
) -> Result<(), ClaimDenialReason> {
    if !matches!(claim.status, ClaimStatusForPolicy::Active | ClaimStatusForPolicy::Confirmed) {
        return Err(ClaimDenialReason::StatusDenied);
    }
    if !claim.predicate_allowed_use_modes.contains(&request.use_mode) {
        return Err(ClaimDenialReason::UseModeDenied);
    }
    if claim.sensitivity > request.sensitivity_ceiling {
        return Err(ClaimDenialReason::SensitivityCeiling);
    }
    let active_human_match = request
        .active_human_ids
        .iter()
        .any(|human_id| human_id == &claim.subject_entity_id || human_id == &claim.object_entity_id);
    let active_object_match = request
        .active_object_ids
        .iter()
        .any(|object_id| object_id == &claim.subject_entity_id || object_id == &claim.object_entity_id);

    match claim.sensitivity {
        Sensitivity::Public => Ok(()),
        Sensitivity::Normal => {
            if active_human_match {
                Ok(())
            } else {
                Err(ClaimDenialReason::OutsideActiveHumanContext)
            }
        }
        Sensitivity::Private => {
            if request.explicit_memory_request || active_object_match {
                Ok(())
            } else {
                Err(ClaimDenialReason::OutsideActiveHumanContext)
            }
        }
        Sensitivity::Sensitive => {
            if request.explicit_memory_request || active_object_match {
                Ok(())
            } else {
                Err(ClaimDenialReason::SensitiveUnlockMissing)
            }
        }
        Sensitivity::Secret => {
            if request.explicit_memory_request && request.approved_secret_access {
                Ok(())
            } else {
                Err(ClaimDenialReason::SecretApprovalMissing)
            }
        }
    }
}
```

- [ ] **Step 4: Add SurrealDB claim retrieval**

Create `crates/noema-core/src/store/retrieval.rs` with:

```rust
use serde::{Deserialize, Serialize};

use crate::memory::{
    ClaimRetrievalRequest, PolicyClaim, UseMode, claim_policy_allows,
};

use super::{ClaimSummary, StoreError, runtime::NoemaStore};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievedClaim {
    pub claim_id: String,
    pub fact: String,
    pub predicate_id: String,
    pub rank_score: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClaimRetrievalResult {
    pub included: Vec<RetrievedClaim>,
    pub redacted_omission_count: usize,
}

impl NoemaStore {
    pub async fn retrieve_claims(
        &self,
        request: &ClaimRetrievalRequest,
        query_text: &str,
        limit: usize,
    ) -> Result<ClaimRetrievalResult, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT
                  id.id() AS claim_id,
                  subject.id() AS subject_entity_id,
                  object.id() AS object_entity_id,
                  predicate.allowed_use_modes AS predicate_allowed_use_modes,
                  predicate.id() AS predicate_id,
                  fact,
                  status,
                  sensitivity
                FROM claim
                WHERE status INSIDE ['active','confirmed']
                  AND (string::lowercase(fact) CONTAINS string::lowercase($query_text)
                       OR $query_text = '')
                FETCH predicate
                LIMIT $limit;
                "#,
            )
            .bind(("query_text", query_text.to_string()))
            .bind(("limit", limit as i64))
            .await?;
        let candidates: Vec<ClaimCandidateRow> = response.take(0)?;
        let mut result = ClaimRetrievalResult::default();
        for candidate in candidates {
            let policy_claim = candidate.policy_claim()?;
            if claim_policy_allows(&policy_claim, request).is_ok() {
                result.included.push(RetrievedClaim {
                    claim_id: candidate.claim_id,
                    fact: candidate.fact,
                    predicate_id: candidate.predicate_id,
                    rank_score: 100,
                });
            } else {
                result.redacted_omission_count += 1;
            }
        }
        Ok(result)
    }
}
```

Define `ClaimCandidateRow` in the same file with a `policy_claim()` method that maps stored strings into `PolicyClaim`.

- [ ] **Step 5: Register retrieval module**

Modify `store.rs`:

```rust
mod retrieval;

pub use retrieval::{ClaimRetrievalResult, RetrievedClaim};
```

- [ ] **Step 6: Run policy and retrieval tests**

Run:

```bash
cargo test -p noema-core memory::tests store::tests --no-fail-fast
```

Expected: PASS for policy tests and existing store tests.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/memory/model.rs crates/noema-core/src/memory/store.rs crates/noema-core/src/memory/tests.rs crates/noema-core/src/store.rs crates/noema-core/src/store/retrieval.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: add deterministic claim retrieval policy"
```

## Task 8: Route Memory Extraction And Search Through Graph Claims

**Files:**
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`
- Modify: `crates/noema-core/src/daemon/memory_consolidation.rs`
- Modify: `crates/noema-core/src/daemon/memory_tool.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add daemon graph-memory tests**

In `crates/noema-core/src/daemon/tests.rs`, replace Postgres memory append assertions with tests named:

```rust
#[tokio::test]
async fn explicit_remember_creates_claim_with_source_evidence() {
    let harness = RuntimeHarness::new().await;
    let started = harness.start_primary_conversation().await;

    harness
        .turn(&started.conversation_id, "remember: I like trains")
        .await
        .expect("turn");

    let claims = harness
        .store
        .retrieve_claims(
            &crate::memory::ClaimRetrievalRequest {
                requesting_agent_id: "agent:primary".to_string(),
                active_human_ids: vec!["human:local".to_string()],
                active_object_ids: Vec::new(),
                use_mode: crate::memory::UseMode::Personalize,
                explicit_memory_request: true,
                sensitivity_ceiling: crate::memory::Sensitivity::Normal,
                approved_secret_access: false,
            },
            "trains",
            8,
        )
        .await
        .expect("claims");

    assert!(claims.included.iter().any(|claim| claim.fact.contains("trains")));
}
```

- [ ] **Step 2: Run daemon graph-memory test to verify failure**

Run:

```bash
cargo test -p noema-core daemon::tests::explicit_remember_creates_claim_with_source_evidence --no-fail-fast
```

Expected: FAIL because explicit remembers still create memory rows.

- [ ] **Step 3: Convert explicit remember to claim candidate**

In `memory_pipeline.rs`, replace `NewMemoryCandidate` output for explicit remembers with `NewClaimCandidate`:

```rust
pub(super) fn explicit_memory_claim_candidate(
    content: &str,
    context: &ConversationMemoryContext,
) -> NewClaimCandidate {
    NewClaimCandidate {
        subject: EntityCandidate::local_human(),
        object: EntityCandidate::concept(&title_from_memory_content(content), &title_from_memory_content(content)),
        predicate_id: if content.to_ascii_lowercase().contains("prefer") {
            "prefers".to_string()
        } else {
            "likes".to_string()
        },
        fact: content.to_string(),
        sensitivity: infer_chat_sensitivity(content),
        status: ClaimStatus::Active,
        confidence: Some(1.0),
        evidence: EvidenceCandidate {
            source_item_id: context.user_item_id.clone(),
            authority: EvidenceAuthority::ExplicitHumanStatement,
            excerpt: Some(content.to_string()),
        },
        retrieval_hints: serde_json::json!({ "keywords": [title_from_memory_content(content)] }),
        metadata: serde_json::json!({
            "turn_id": context.turn_id,
            "turn_index": context.turn_index,
            "trigger": "explicit_remember"
        }),
    }
}
```

- [ ] **Step 4: Replace memory consolidation outcomes with claim outcomes**

In `memory_consolidation.rs`, rename outcome fields from `memory_id` to `claim_id` and make `Reinforced` represent an added evidence edge. Keep transcript JSON keys stable enough for UI by emitting:

```json
{
  "outcome": "reinforced",
  "claim_id": "claim_...",
  "proposal_content": "I like trains",
  "reason": "repeated_evidence"
}
```

- [ ] **Step 5: Update runtime memory calls**

In `runtime.rs`:

- Replace `append_memory_candidate` with `create_or_reinforce_claim`.
- Replace `retrieve_memories` tool path with `retrieve_claims`.
- Replace memory transcript text such as "Memory saved" with "Memory claim saved" only in activity metadata, not broad UI copy.

- [ ] **Step 6: Update `search_memory` arguments**

In `memory_tool.rs`, replace `purpose` with `use_mode`:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchMemoryArguments {
    query: String,
    #[serde(default)]
    use_mode: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}
```

Accept only `answer`, `personalize`, `plan`, `act`, `notify`, `inspect`, and `export`. Return successful tool payload:

```json
{
  "claims": [
    {
      "id": "claim_...",
      "fact": "Kevin likes trains.",
      "predicate": "likes",
      "why": "active_human"
    }
  ],
  "omissions": [
    {
      "reason": "policy_restricted_context"
    }
  ]
}
```

- [ ] **Step 7: Run daemon memory tests**

Run:

```bash
cargo test -p noema-core daemon::tests memory::tests store::tests --no-fail-fast
```

Expected: PASS after all memory tests reference graph claims.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/daemon/memory_pipeline.rs crates/noema-core/src/daemon/memory_consolidation.rs crates/noema-core/src/daemon/memory_tool.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: route memory through graph claims"
```

## Task 9: Add Retrieval Packets And Graph Inspection

**Files:**
- Create: `crates/noema-core/src/store/inspection.rs`
- Modify: `crates/noema-core/src/store/retrieval.rs`
- Modify: `crates/noema-core/src/context_graph.rs`
- Modify: `crates/noema-cli/src/inspection.rs`
- Modify: `crates/noema-cli/src/inspection/context_graph_output.rs`
- Modify: `crates/noema-cli/src/inspection/context_graph_text.rs`
- Test: `crates/noema-cli/src/inspection_tests.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add retrieval packet test**

Add to `store/tests.rs`:

```rust
#[tokio::test]
async fn retrieval_records_packet_with_redacted_omission_count() {
    let store = temp_store().await;
    let request = crate::memory::ClaimRetrievalRequest {
        requesting_agent_id: "agent:primary".to_string(),
        active_human_ids: vec!["human:local".to_string()],
        active_object_ids: Vec::new(),
        use_mode: crate::memory::UseMode::Answer,
        explicit_memory_request: false,
        sensitivity_ceiling: crate::memory::Sensitivity::Normal,
        approved_secret_access: false,
    };

    let packet = store
        .record_retrieval_packet("run_test", &request, &[], 2)
        .await
        .expect("packet");

    assert_eq!(packet.run_id, "run_test");
    assert_eq!(packet.redacted_omission_count, 2);
}
```

- [ ] **Step 2: Run retrieval packet test to verify missing method**

Run:

```bash
cargo test -p noema-core store::tests::retrieval_records_packet_with_redacted_omission_count --no-fail-fast
```

Expected: FAIL because retrieval packet methods are missing.

- [ ] **Step 3: Implement retrieval packet records**

In `store/retrieval.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalPacketRecord {
    pub retrieval_packet_id: String,
    pub run_id: String,
    pub redacted_omission_count: usize,
}

impl NoemaStore {
    pub async fn record_retrieval_packet(
        &self,
        run_id: &str,
        request: &ClaimRetrievalRequest,
        included_claim_ids: &[String],
        redacted_omission_count: usize,
    ) -> Result<RetrievalPacketRecord, StoreError> {
        let packet_id = stable_id("retrieval_packet", &[run_id, &format!("{:?}", request.use_mode)]);
        let mut response = self
            .db()
            .query(
                r#"
                CREATE type::thing('retrieval_packet', $packet_id) SET
                  run_id = $run_id,
                  requesting_agent = type::thing('agent', $requesting_agent),
                  active_humans = $active_humans,
                  active_objects = $active_objects,
                  use_mode = $use_mode,
                  included_claims = $included_claims,
                  redacted_omissions = $redacted_omissions,
                  policy_version = 1;
                SELECT id.id() AS retrieval_packet_id, run_id, array::len(redacted_omissions) AS redacted_omission_count
                FROM type::thing('retrieval_packet', $packet_id);
                "#,
            )
            .bind(("packet_id", packet_id))
            .bind(("run_id", run_id.to_string()))
            .bind(("requesting_agent", request.requesting_agent_id.trim_start_matches("agent:").to_string()))
            .bind(("active_humans", request.active_human_ids.iter().map(|id| format!("human:{}", id.trim_start_matches("human:"))).collect::<Vec<_>>()))
            .bind(("active_objects", serde_json::json!(request.active_object_ids)))
            .bind(("use_mode", use_mode_string(request.use_mode)))
            .bind(("included_claims", included_claim_ids.iter().map(|id| format!("claim:{id}")).collect::<Vec<_>>()))
            .bind(("redacted_omissions", vec![serde_json::json!({"reason": "policy_restricted_context"}); redacted_omission_count]))
            .await?;
        let rows: Vec<RetrievalPacketRecord> = response.take(1)?;
        rows.into_iter()
            .next()
            .ok_or_else(|| StoreError::Schema("retrieval packet was not returned".to_string()))
    }
}
```

- [ ] **Step 4: Create graph inspection read model**

Create `store/inspection.rs` with `GraphClaimNode`, `GraphPredicateNode`, `GraphEvidenceEdge`, `GraphRetrievalPacket`, and `GraphSummary` structs. Implement:

```rust
impl NoemaStore {
    pub async fn inspect_graph(&self, limit: u32) -> Result<GraphSummary, StoreError> {
        let limit = limit.clamp(1, 200) as i64;
        let claims = self.inspect_claims(limit).await?;
        let predicates = self.inspect_predicates(limit).await?;
        let evidence = self.inspect_evidence(limit).await?;
        let retrieval_packets = self.inspect_retrieval_packets(limit).await?;
        Ok(GraphSummary {
            claims,
            predicates,
            evidence,
            retrieval_packets,
        })
    }
}
```

Each helper should query the matching SurrealDB table and decode typed rows.

- [ ] **Step 5: Replace CLI context graph rendering**

Update `crates/noema-cli/src/inspection.rs`:

- Use `NoemaStore::open(&StoreConfig::from_paths(&NoemaPaths::from_process_env()?))`.
- Replace `repo.inspect_context_graph_with_filter` with `store.inspect_graph`.
- Keep `--limit` and `--format`.
- Remove `--run-id` and `--packet-id` only after GraphQL/CLI generated docs are updated.

- [ ] **Step 6: Run inspection tests**

Run:

```bash
cargo test -p noema-core store::tests::retrieval_records_packet_with_redacted_omission_count --no-fail-fast
cargo test -p noema-cli inspection --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/store/retrieval.rs crates/noema-core/src/store/inspection.rs crates/noema-core/src/store.rs crates/noema-core/src/context_graph.rs crates/noema-cli/src/inspection.rs crates/noema-cli/src/inspection/context_graph_output.rs crates/noema-cli/src/inspection/context_graph_text.rs crates/noema-cli/src/inspection_tests.rs
git commit -m "feat: add graph inspection and retrieval packets"
```

## Task 10: Update GraphQL And Web State

**Files:**
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/graphql/resolvers.rs`
- Modify: `crates/noema-core/src/graphql/types.rs`
- Modify: `crates/noema-core/web/src/generated/schema.graphql`
- Modify: `crates/noema-core/web/src/generated/graphql.ts`
- Test: `crates/noema-core/src/graphql/resolvers.rs`

- [ ] **Step 1: Add GraphQL local status test for embedded store wording**

Update `local_status_query_returns_running_codex` expected data only if schema fields change. Keep `memoryStorage: READY` stable:

```rust
assert_eq!(
    response.data,
    Value::from_json(serde_json::json!({
        "localStatus": {
            "localService": "RUNNING",
            "assistantConnection": "CODEX",
            "memoryStorage": "READY"
        }
    }))
    .expect("valid json")
);
```

- [ ] **Step 2: Rename web state accessors**

In `daemon/web/mod.rs`, rename field and accessor:

```rust
store: crate::NoemaStore,

pub(crate) fn store(&self) -> &crate::NoemaStore {
    &self.store
}
```

Replace resolver calls from `web.memory_repository()` to `web.store()`.

- [ ] **Step 3: Update provider auth persistence trait implementation**

Move `ProviderAccountStatusStore for PostgresMemoryRepository` to `ProviderAccountStatusStore for crate::NoemaStore`. Keep method names:

```rust
impl ProviderAccountStatusStore for crate::NoemaStore {
    async fn active_provider_account(
        &self,
        provider_kind: &str,
    ) -> Result<Option<crate::ProviderAccountRecord>, crate::StoreError> {
        self.active_provider_account(provider_kind).await
    }
}
```

Adjust trait error type from `MemoryPersistenceError` to `StoreError`.

- [ ] **Step 4: Run GraphQL and web tests**

Run:

```bash
cargo test -p noema-core graphql daemon::web --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Regenerate frontend GraphQL types**

Run:

```bash
bun run gen:types
```

from `crates/noema-core/web`.

Expected: generated schema/types update only if public schema changed.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/daemon/web/mod.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/graphql/resolvers.rs crates/noema-core/src/graphql/types.rs crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "refactor: point graphql at embedded store"
```

## Task 11: Delete Postgres Persistence And SQLx

**Files:**
- Delete: `crates/noema-core/src/memory_persistence.rs`
- Delete: `crates/noema-core/src/memory_persistence/*`
- Delete: `crates/noema-core/src/postgres_memory_retrieval.rs`
- Delete: `crates/noema-core/src/postgres_retrieval_policy_fingerprint.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-cli/src/inspection.rs`
- Test: workspace compile

- [ ] **Step 1: Search remaining Postgres symbols**

Run:

```bash
rg "PostgresMemoryRepository|memory_persistence|postgres_memory_retrieval|postgres_retrieval_policy_fingerprint|DatabaseConfig|NOEMA_DATABASE_URL|sqlx" crates Cargo.toml
```

Expected: results remain before cleanup.

- [ ] **Step 2: Remove public exports**

In `lib.rs`, remove:

```rust
pub mod memory_persistence;
mod postgres_memory_retrieval;
mod postgres_retrieval_policy_fingerprint;
```

Remove all `pub use memory_persistence::{...};` exports. Export corresponding store types instead:

```rust
pub use store::{
    ActorRef, AgentStatus, ClaimStatus, ClaimSummary, ConversationItemKind,
    ConversationItemRecord, ConversationItemStatus, ConversationRecord,
    ConversationTurnRecord, ConversationTurnStatus, EntityCandidate, EntityType,
    EvidenceAuthority, EvidenceCandidate, NewClaimCandidate, NewConversation,
    NewConversationItem, NewConversationTurn, NoemaStore, PredicateRecord,
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod, ReplayMode,
    StoreConfig, StoreError,
};
```

- [ ] **Step 3: Delete Postgres modules**

Delete:

```bash
git rm crates/noema-core/src/memory_persistence.rs
git rm -r crates/noema-core/src/memory_persistence
git rm crates/noema-core/src/postgres_memory_retrieval.rs
git rm crates/noema-core/src/postgres_retrieval_policy_fingerprint.rs
```

- [ ] **Step 4: Remove SQLx dependencies**

Remove `sqlx` from root `Cargo.toml` and from any crate-level dependency block. Run:

```bash
cargo update -p sqlx --precise 0.8.0
```

Expected: command may report package not found after removal. That is acceptable if `rg "sqlx" Cargo.toml Cargo.lock crates` returns no runtime references.

- [ ] **Step 5: Verify no Postgres runtime symbols remain**

Run:

```bash
rg "Postgres|postgres|SQLx|sqlx|NOEMA_DATABASE_URL|DatabaseConfig|memory_persistence" crates Cargo.toml
```

Expected: no results in `crates` or `Cargo.toml`. Historical docs may still mention Postgres until Task 12.

- [ ] **Step 6: Run compile**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/noema-core/Cargo.toml crates/noema-core/src/lib.rs crates/noema-cli/src/inspection.rs
git add -u crates/noema-core/src
git commit -m "refactor: remove postgres persistence"
```

## Task 12: Update Docs And Final Validation

**Files:**
- Modify: `docs/project.md`
- Modify: `docs/memory.md`
- Modify: `docs/harness.md`
- Modify: `docs/context/current.md`
- Modify/Delete: `docs/postgres.md`
- Modify/Delete: `docs/sqlite.md`
- Modify: older superseded specs/plans only where they claim to be current
- Test: full validation

- [ ] **Step 1: Update project storage docs**

In `docs/project.md`, replace the storage tree:

```text
~/.noema/
  config.yaml
  db/                    # embedded SurrealDB canonical structured store
  providers/
  humans/
  agents/
  conversations/
  workspaces/
  system/
```

Replace the source-of-truth row for structured state with:

```markdown
| Structured humans, agents, tools, conversations, transcript items, provider account metadata, entities, predicates, claims, evidence relations, retrieval packets, tasks, permissions, and audit/security events | Embedded SurrealDB under `db/` |
```

- [ ] **Step 2: Replace memory storage language**

In `docs/memory.md`, replace references to `memory_items`, `relationships`, `object_provenance_edges`, `memory_use_records`, and Postgres FTS with:

```markdown
Durable memories are graph claims over entities and promoted predicate records.
Claims carry lifecycle, sensitivity, temporal validity, retrieval hints, and
policy overrides. Evidence relations connect claims to conversation items or
other source objects. Retrieval packets record assembled context and redacted
omissions.
```

- [ ] **Step 3: Retire Postgres docs**

Replace `docs/postgres.md` with a short redirect:

```markdown
# Superseded Postgres Schema

Postgres is no longer the target canonical structured store. See
[`docs/superpowers/specs/2026-06-28-surrealdb-graph-memory-store-design.md`](superpowers/specs/2026-06-28-surrealdb-graph-memory-store-design.md)
for the approved embedded SurrealDB graph memory store direction.
```

Replace `docs/sqlite.md` with:

```markdown
# Superseded SQLite Notes

SQLite is not the target canonical structured store for Noema. Embedded
SurrealDB under `NOEMA_HOME/db` is the approved direction.
```

- [ ] **Step 4: Update durable context**

In `docs/context/current.md`, change "current implementation still contains Postgres-backed persistence" to:

```markdown
The current implementation now uses embedded SurrealDB-backed repositories for
canonical structured state.
```

Update open loops to remove "replace Postgres persistence".

- [ ] **Step 5: Run documentation scan**

Run:

```bash
rg "Postgres|postgres|SQLite|sqlite|SQLx|sqlx|NOEMA_DATABASE_URL" docs crates Cargo.toml
```

Expected: matches only in superseded historical plans/specs or explicit "superseded" notes. No active docs or runtime code should describe Postgres as current.

- [ ] **Step 6: Run full validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Run frontend validation if GraphQL changed**

From `crates/noema-core/web`, run:

```bash
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 8: Final status and commit**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: only intended docs/code changes are unstaged, and `git diff --check` prints no output.

Commit:

```bash
git add docs crates Cargo.toml Cargo.lock
git commit -m "docs: align storage docs with surrealdb"
```

## Plan Self-Review

- Spec coverage: Tasks cover embedded store runtime, strict schema, personal-agent predicate ontology, graph claims, evidence relations, deterministic `use_mode` retrieval, retrieval packets, API/CLI integration, Postgres removal, docs, and validation.
- Scope: The plan is a full replacement because the approved design rejects a dual-store interim architecture. Tasks are ordered so each subsystem is validated before deleting the old Postgres code.
- Type consistency: The plan consistently uses `NoemaStore`, `StoreConfig`, `StoreError`, `UseMode`, `ClaimStatus`, `NewClaimCandidate`, `EvidenceCandidate`, `ClaimRetrievalRequest`, `RetrievedClaim`, and `RetrievalPacketRecord`.
- Execution note: If SurrealDB relation-to-relation targeting is awkward during Task 6 or Task 9, keep `claim` as a schemafull record with `subject`, `object`, and `predicate` record fields. That still preserves graph-queryable claim traversal and stable evidence targets, matching the approved design.
