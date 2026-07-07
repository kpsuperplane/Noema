# Supermemory SQLite Reset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Noema's SurrealDB-backed in-house memory graph with SQLite for Noema app state and Supermemory for memory.

**Architecture:** SQLite becomes Noema's only canonical structured store at `NOEMA_HOME/db/noema.sqlite3`, implemented through the existing `NoemaStore` repository facade. Supermemory is a separate memory subsystem with a managed-child-process default and external-service override. The first slice keeps `search_memory` as explicit tool-only recall, removes Noema memory proposals and `/remember`, and redirects `/memory` to Settings > Memory.

**Tech Stack:** Rust 1.96, Tokio, rusqlite, reqwest, async-graphql, React, TanStack Router, Apollo Client, Bun, StyleX/Astryx.

## Global Constraints

- Clean pre-V1 reset: do not migrate SurrealDB data.
- No local mirror of Supermemory's graph.
- No Noema-owned memory extraction, contradiction resolution, predicate review, or graph visualization.
- No automatic pre-turn memory injection.
- No transcript markers for automatic memory capture or explicit memory writes.
- No user-facing Supermemory graph browser in the first slice.
- Use `rusqlite` for SQLite.
- SQLite path is `${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`.
- Supermemory managed data path is `${NOEMA_HOME:-$HOME/.noema}/supermemory/data`.
- Supermemory secret path is `${NOEMA_HOME:-$HOME/.noema}/supermemory/secrets/`.
- `search_memory` remains the only model-visible recall path.
- `/remember` is ordinary chat text.
- `/memory` redirects to `/settings/memory` until Supermemory-backed browsing exists.
- Secrets stay out of SQLite.
- Preserve unrelated dirty worktree changes.
- Before each commit, run `git status --short --branch` and `git diff --check`.

---

## Scope Check

This reset touches storage, memory runtime, GraphQL, frontend settings, and docs. Keep it as one implementation sequence because each later phase depends on the earlier phase: SQLite store first, Supermemory lifecycle second, runtime memory replacement third, UI/API fourth, cleanup fifth.

## File Structure

Create or heavily modify these Rust modules:

- `crates/noema-core/src/store/runtime.rs`: SQLite connection owner, store config, schema bootstrap, shared locks.
- `crates/noema-core/src/store/schema.rs`: SQLite schema version and DDL.
- `crates/noema-core/src/store/sqlite.rs`: small helper functions for JSON, timestamps, optional rows, and transaction mapping.
- `crates/noema-core/src/store/error.rs`: convert rusqlite and JSON errors into `StoreError`.
- `crates/noema-core/src/store/{agents,provider_accounts,provider_capability_bindings,agent_runtime_preferences,auxiliary_model_preferences,conversations,context_summaries,mcp}.rs`: port existing repositories from SurrealDB to SQLite.
- `crates/noema-core/src/store/memory_service.rs`: new SQLite repository for Supermemory settings/status/jobs.
- `crates/noema-core/src/supermemory.rs`: facade module.
- `crates/noema-core/src/supermemory/client.rs`: HTTP client for Supermemory v4 APIs.
- `crates/noema-core/src/supermemory/config.rs`: config records and model backend environment mapping.
- `crates/noema-core/src/supermemory/lifecycle.rs`: managed child process and readiness polling.
- `crates/noema-core/src/daemon/memory/tool.rs`: adapt `search_memory` to Supermemory.
- `crates/noema-core/src/daemon/memory/pipeline.rs`, `crates/noema-core/src/daemon/memory/writes.rs`, `crates/noema-core/src/memory/{consolidation,extraction,model}.rs`, `crates/noema-core/src/store/claims*`, `crates/noema-core/src/store/ontology.rs`, `crates/noema-core/src/store/retrieval.rs`: remove after provider memory proposals and graph APIs are gone.
- `crates/noema-core/src/graphql/memory.rs`: replace graph claim API with Memory service settings API.
- `crates/noema-core/src/graphql/schema.rs`: remove old memory read fields and add Memory settings mutations.
- `crates/noema-core/src/runtime_host.rs`: start Supermemory lifecycle after SQLite store startup.
- `crates/noema-core/src/paths.rs`: add Supermemory paths.

Create or modify these frontend files:

- `crates/noema-core/web/src/routes/settings/memory.tsx`: new Settings > Memory route.
- `crates/noema-core/web/src/components/settings/MemorySettingsPane.tsx`: GraphQL container.
- `crates/noema-core/web/src/components/settings/MemorySettingsPaneContent.tsx`: presentational settings UI.
- `crates/noema-core/web/src/components/settings/memorySettingsModel.ts`: pure display/state helpers.
- `crates/noema-core/web/src/pages/SettingsPage.tsx`: add Memory section copy and pane.
- `crates/noema-core/web/src/app/routes.ts`: add `/settings/memory`; redirect `/memory`.
- `crates/noema-core/web/src/components/shell/shellNavigation.ts`: remove primary Memory item and add Settings > Memory.
- `crates/noema-core/web/src/routes/memory.tsx`, `crates/noema-core/web/src/routes/memory/graph.tsx`, `crates/noema-core/web/src/pages/MemoryHomePage.tsx`, `crates/noema-core/web/src/pages/MemoryGraphPage.tsx`, `crates/noema-core/web/src/components/memory/*`: remove graph route/components after references are gone.

Docs to update:

- `docs/project.md`
- `docs/context/current.md`
- `docs/memory.md`
- `docs/sqlite.md`
- `docs/frontend/current-contract.md`

## Task 1: Add SQLite Store Foundation

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-core/src/store/runtime.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/error.rs`
- Create: `crates/noema-core/src/store/sqlite.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/paths.rs`
- Modify: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Produces: `NoemaStore { conn: Arc<Mutex<rusqlite::Connection>>, noema_home: PathBuf, append_item_lock: Arc<Mutex<()>>, claim_write_lock: Arc<Mutex<()>> }`
- Produces: `StoreConfig::from_paths(paths: &NoemaPaths) -> StoreConfig` with `path = paths.sqlite_db_path()`
- Produces: `NoemaPaths::sqlite_db_path(&self) -> PathBuf`
- Produces: `NoemaStore::connection_for_tests(&self) -> Arc<Mutex<rusqlite::Connection>>`
- Consumes: existing `NoemaPaths::db_dir()`

- [ ] **Step 1: Add the dependency**

Edit the workspace dependency block:

```toml
# Cargo.toml
[workspace.dependencies]
rusqlite = { version = "0.37", features = ["bundled", "time"] }
```

Edit `noema-core` dependencies:

```toml
# crates/noema-core/Cargo.toml
[dependencies]
rusqlite.workspace = true
```

Remove `surrealdb.workspace = true` from `crates/noema-core/Cargo.toml` only after all SurrealDB imports are gone in Task 9.

- [ ] **Step 2: Add SQLite paths**

In `crates/noema-core/src/paths.rs`, add:

```rust
impl NoemaPaths {
    /// Path to the canonical SQLite database file.
    #[must_use]
    pub fn sqlite_db_path(&self) -> PathBuf {
        self.db_dir().join("noema.sqlite3")
    }

    /// Path to Supermemory-owned state.
    #[must_use]
    pub fn supermemory_dir(&self) -> PathBuf {
        self.root.join("supermemory")
    }

    /// Path to Supermemory managed data.
    #[must_use]
    pub fn supermemory_data_dir(&self) -> PathBuf {
        self.supermemory_dir().join("data")
    }

    /// Path to Supermemory secret files.
    #[must_use]
    pub fn supermemory_secrets_dir(&self) -> PathBuf {
        self.supermemory_dir().join("secrets")
    }
}
```

Add this test:

```rust
#[test]
fn sqlite_db_path_lives_under_db_dir() {
    let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

    assert_eq!(
        paths.sqlite_db_path(),
        PathBuf::from("/tmp/noema/db/noema.sqlite3")
    );
}
```

Run:

```bash
cargo test -p noema-core paths::tests::sqlite_db_path_lives_under_db_dir -- --nocapture
```

Expected: pass.

- [ ] **Step 3: Replace schema constants**

Replace `crates/noema-core/src/store/schema.rs` with SQLite DDL:

```rust
/// Current schema version for pre-stable local SQLite data.
pub const STORE_SCHEMA_VERSION: i64 = 1;

/// SQLite bootstrap used by the Noema store.
pub const STORE_SCHEMA_SQL: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS schema_state (
  name TEXT PRIMARY KEY NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO schema_state (name, version, applied_at)
VALUES ('sqlite_store_v1', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(name) DO UPDATE SET
  version = excluded.version,
  applied_at = excluded.applied_at;

CREATE TABLE IF NOT EXISTS humans (
  human_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  primary_conversation_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS agents (
  agent_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS agent_runtime_preferences (
  agent_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS auxiliary_model_preferences (
  task_id TEXT PRIMARY KEY NOT NULL CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'supermemory_extraction')),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS provider_accounts (
  provider_account_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'exa')),
  account_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  auth_method TEXT NOT NULL CHECK (auth_method IN ('oauth_device_code', 'secret_input', 'external_manual', 'none')),
  is_active INTEGER NOT NULL CHECK (is_active IN (0, 1)),
  is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
  status TEXT NOT NULL CHECK (status IN ('unknown', 'checking', 'authenticated', 'unauthenticated', 'unavailable')),
  last_checked_at TEXT,
  last_authenticated_at TEXT,
  last_error_code TEXT,
  last_error_message TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(provider_kind, account_key)
);

CREATE TABLE IF NOT EXISTS provider_capability_bindings (
  binding_id TEXT PRIMARY KEY NOT NULL,
  tool_name TEXT NOT NULL CHECK (tool_name IN ('web.search', 'web.fetch')),
  capability_id TEXT NOT NULL,
  provider_account_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(tool_name, capability_id)
);

CREATE TABLE IF NOT EXISTS conversations (
  conversation_id TEXT PRIMARY KEY NOT NULL,
  title TEXT,
  owner_object_type TEXT NOT NULL CHECK (owner_object_type IN ('human', 'agent', 'conversation', 'workspace', 'project', 'task', 'tool')),
  owner_object_id TEXT NOT NULL,
  primary_human_id TEXT,
  primary_agent_id TEXT,
  provider TEXT NOT NULL CHECK (provider IN ('codex', 'openai', 'foundation_local')),
  model TEXT,
  cwd TEXT,
  lifecycle_status TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle_status IN ('active', 'archived')),
  agent_status TEXT NOT NULL DEFAULT 'idle' CHECK (agent_status IN ('idle', 'input_received', 'thinking', 'tool_running', 'waiting_for_previous_turn_completion', 'interrupting', 'error')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS conversation_turns (
  turn_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  trigger_item_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('input_received', 'running', 'waiting_for_tool', 'interrupted', 'completed', 'failed', 'cancelled')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS conversation_turns_conversation_id
ON conversation_turns(conversation_id);

CREATE TABLE IF NOT EXISTS conversation_items (
  item_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  turn_id TEXT,
  parent_item_id TEXT,
  sequence_index INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('user_text', 'assistant_text', 'activity', 'a2ui_card', 'tool_call', 'tool_result', 'reasoning', 'approval_request', 'approval_result', 'error_notice')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled', 'interrupted')),
  author_actor_id TEXT NOT NULL,
  content_text TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(conversation_id, sequence_index)
);

CREATE INDEX IF NOT EXISTS conversation_items_conversation_sequence
ON conversation_items(conversation_id, sequence_index);

CREATE TABLE IF NOT EXISTS memory_service_settings (
  settings_id TEXT PRIMARY KEY NOT NULL CHECK (settings_id = 'default'),
  mode TEXT NOT NULL CHECK (mode IN ('managed', 'external')),
  base_url TEXT NOT NULL,
  port INTEGER CHECK (port IS NULL OR (port > 0 AND port <= 65535)),
  provider_account_id TEXT,
  provider_kind TEXT CHECK (provider_kind IS NULL OR provider_kind IN ('codex', 'openai', 'foundation_local')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS memory_service_status (
  status_id TEXT PRIMARY KEY NOT NULL CHECK (status_id = 'default'),
  status TEXT NOT NULL CHECK (status IN ('not_configured', 'starting', 'ready', 'unavailable', 'auth_error')),
  checked_at TEXT,
  last_error_code TEXT,
  last_error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS memory_ingest_jobs (
  job_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  turn_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'submitted', 'failed')),
  supermemory_conversation_id TEXT NOT NULL,
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(turn_id)
);

INSERT INTO memory_service_settings (settings_id, mode, base_url, port)
VALUES ('default', 'managed', 'http://127.0.0.1:6767', 6767)
ON CONFLICT(settings_id) DO NOTHING;

INSERT INTO memory_service_status (status_id, status)
VALUES ('default', 'not_configured')
ON CONFLICT(status_id) DO NOTHING;
"#;
```

The remaining tables are added in Tasks 2 and 3 when their repositories are ported.

- [ ] **Step 4: Add SQLite helpers**

Create `crates/noema-core/src/store/sqlite.rs`:

```rust
use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::StoreError;

pub(super) fn json_to_string(value: &Value) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(StoreError::Json)
}

pub(super) fn serialize_json<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(StoreError::Json)
}

pub(super) fn json_from_string(value: String) -> Result<Value, StoreError> {
    serde_json::from_str(&value).map_err(StoreError::Json)
}

pub(super) fn deserialize_json<T: DeserializeOwned>(value: String) -> Result<T, StoreError> {
    serde_json::from_str(&value).map_err(StoreError::Json)
}

pub(super) fn optional_row<T, F>(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    mapper: F,
) -> Result<Option<T>, StoreError>
where
    F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
{
    conn.query_row(sql, params, mapper)
        .optional()
        .map_err(StoreError::Sqlite)
}

pub(super) fn now_timestamp_sql() -> &'static str {
    "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
}
```

- [ ] **Step 5: Add SQLite errors**

In `crates/noema-core/src/store/error.rs`, add these variants:

```rust
/// SQLite operation failed.
#[error("sqlite store operation failed: {0}")]
Sqlite(#[from] rusqlite::Error),

/// JSON encoding or decoding failed.
#[error("store JSON encoding failed: {0}")]
Json(#[from] serde_json::Error),
```

Keep existing domain variants such as `InvalidEnum`, `Schema`, and not-found errors.

- [ ] **Step 6: Replace store runtime**

Replace SurrealDB fields in `crates/noema-core/src/store/runtime.rs`:

```rust
use std::{fs, path::PathBuf, sync::Arc};

use rusqlite::Connection;
use tokio::sync::Mutex;

use super::{
    error::StoreError,
    schema::{STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    pub path: PathBuf,
    pub noema_home: PathBuf,
}

impl StoreConfig {
    #[must_use]
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self {
            path: paths.sqlite_db_path(),
            noema_home: paths.root().to_path_buf(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoemaStore {
    pub(super) conn: Arc<Mutex<Connection>>,
    pub(super) noema_home: PathBuf,
    pub(super) append_item_lock: Arc<Mutex<()>>,
    pub(super) claim_write_lock: Arc<Mutex<()>>,
}

impl NoemaStore {
    pub async fn open(config: &StoreConfig) -> Result<Self, StoreError> {
        if let Some(parent) = config.path.parent() {
            fs::create_dir_all(parent).map_err(StoreError::PreparePath)?;
        }
        let conn = Connection::open(&config.path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.execute_batch(STORE_SCHEMA_SQL)?;
        debug_assert_eq!(STORE_SCHEMA_VERSION, 1);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            noema_home: config.noema_home.clone(),
            append_item_lock: Arc::new(Mutex::new(())),
            claim_write_lock: Arc::new(Mutex::new(())),
        })
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn connection_for_tests(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub(crate) async fn with_connection<T>(
        &self,
        work: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let conn = self.conn.lock().await;
        work(&conn)
    }

    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT version FROM schema_state WHERE name = 'sqlite_store_v1'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    pub async fn close(self) -> Result<(), StoreError> {
        Ok(())
    }
}
```

- [ ] **Step 7: Update `store.rs` module exports**

Add:

```rust
mod sqlite;
mod memory_service;

pub use memory_service::{
    MemoryIngestJobRecord, MemoryServiceMode, MemoryServiceSettingsRecord,
    MemoryServiceStatus, MemoryServiceStatusRecord, NewMemoryIngestJob,
    SaveMemoryServiceSettings,
};
```

Keep `mod claims`, `mod ontology`, and `mod retrieval` until Task 7 removes memory graph callers.

- [ ] **Step 8: Replace foundation tests**

Replace SurrealDB-specific tests in `crates/noema-core/src/store/tests.rs` with:

```rust
use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

#[tokio::test]
async fn opens_sqlite_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(paths.db_dir().exists());
    assert!(paths.sqlite_db_path().exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
}

#[tokio::test]
async fn sqlite_store_config_is_stable_for_reopen() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    assert_eq!(config.path, paths.sqlite_db_path());
    assert_eq!(StoreConfig::from_paths(&paths), config);
}

pub(crate) async fn test_store() -> crate::NoemaStore {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("open store");
    std::mem::forget(home);
    store
}
```

- [ ] **Step 9: Run the foundation tests**

Run:

```bash
cargo test -p noema-core store::tests::opens_sqlite_store_under_noema_db_dir store::tests::sqlite_store_config_is_stable_for_reopen -- --nocapture
```

Expected: pass.

- [ ] **Step 10: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add Cargo.toml crates/noema-core/Cargo.toml crates/noema-core/src/paths.rs crates/noema-core/src/store.rs crates/noema-core/src/store/runtime.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/error.rs crates/noema-core/src/store/sqlite.rs crates/noema-core/src/store/tests.rs
git commit -m "Replace store foundation with SQLite"
```

## Task 2: Port Core SQLite Repositories

**Files:**
- Modify: `crates/noema-core/src/store/agents.rs`
- Modify: `crates/noema-core/src/store/provider_accounts.rs`
- Modify: `crates/noema-core/src/store/provider_capability_bindings.rs`
- Modify: `crates/noema-core/src/store/agent_runtime_preferences.rs`
- Modify: `crates/noema-core/src/store/auxiliary_model_preferences.rs`
- Modify: `crates/noema-core/src/store/conversations.rs`
- Modify: `crates/noema-core/src/store/context_summaries.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Consumes: `NoemaStore::with_connection`
- Produces: current public repository methods with the same signatures currently used by GraphQL/runtime.
- Produces: no SurrealDB imports in these modules.

- [ ] **Step 1: Write actor repository tests**

Add tests to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn sqlite_default_actors_round_trip() {
    let store = test_store().await;

    store.ensure_default_actors().await.expect("default actors");

    let human = store
        .get_human("human:local")
        .await
        .expect("get human")
        .expect("human exists");
    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");

    assert_eq!(human.human_id, "human:local");
    assert_eq!(agent.agent_id, "agent:primary");
    assert_eq!(agent.display_name, None);
}

#[tokio::test]
async fn sqlite_agent_display_name_updates() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");

    store
        .update_agent_display_name("agent:primary", "Noema")
        .await
        .expect("update agent");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Noema"));
}
```

Run:

```bash
cargo test -p noema-core store::tests::sqlite_default_actors_round_trip store::tests::sqlite_agent_display_name_updates -- --nocapture
```

Expected: fail before porting `agents.rs`.

- [ ] **Step 2: Port `agents.rs`**

Use `INSERT ... ON CONFLICT` and `query_row`:

```rust
impl NoemaStore {
    pub async fn ensure_default_actors(&self) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO humans (human_id, display_name)
                VALUES ('human:local', 'You')
                ON CONFLICT(human_id) DO NOTHING
                "#,
                [],
            )?;
            conn.execute(
                r#"
                INSERT INTO agents (agent_id, display_name)
                VALUES ('agent:primary', NULL)
                ON CONFLICT(agent_id) DO NOTHING
                "#,
                [],
            )?;
            Ok(())
        })
        .await
    }
}
```

Map rows with existing `AgentRecord` and `HumanRecord` fields. Parse timestamps as strings because current GraphQL/read models already expose strings.

- [ ] **Step 3: Run actor tests**

Run:

```bash
cargo test -p noema-core store::tests::sqlite_default_actors_round_trip store::tests::sqlite_agent_display_name_updates -- --nocapture
```

Expected: pass.

- [ ] **Step 4: Write provider and preference tests**

Add tests covering existing public methods:

```rust
#[tokio::test]
async fn sqlite_provider_accounts_seed_and_list() {
    let store = test_store().await;

    store.ensure_default_provider_account().await.expect("codex");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation");

    let accounts = store
        .list_provider_accounts()
        .await
        .expect("provider accounts");
    let ids = accounts
        .iter()
        .map(|account| account.provider_account_id.as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&"provider_account:codex:default"));
    assert!(ids.contains(&"provider_account:foundation_local:default"));
}

#[tokio::test]
async fn sqlite_agent_model_preference_round_trip() {
    let store = test_store().await;
    store.ensure_default_provider_account().await.expect("provider");

    store
        .save_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Medium),
        })
        .await
        .expect("save preference");

    let preference = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("get preference")
        .expect("preference exists");
    assert_eq!(preference.model_profile, "gpt-5.5");
    assert_eq!(preference.reasoning_effort, Some(crate::provider::ReasoningEffort::Medium));
}
```

Run:

```bash
cargo test -p noema-core store::tests::sqlite_provider_accounts_seed_and_list store::tests::sqlite_agent_model_preference_round_trip -- --nocapture
```

Expected: fail before porting repositories.

- [ ] **Step 5: Port provider and preference repositories**

For enum fields, preserve existing parse helpers. Store booleans as `0`/`1`. Store JSON metadata through `store::sqlite::json_to_string`.

Use this upsert shape for preferences:

```rust
conn.execute(
    r#"
    INSERT INTO agent_runtime_preferences
      (agent_id, provider_kind, provider_account_id, model_profile, reasoning_effort, updated_at)
    VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(agent_id) DO UPDATE SET
      provider_kind = excluded.provider_kind,
      provider_account_id = excluded.provider_account_id,
      model_profile = excluded.model_profile,
      reasoning_effort = excluded.reasoning_effort,
      updated_at = excluded.updated_at
    "#,
    rusqlite::params![
        input.agent_id,
        input.provider_kind,
        input.provider_account_id,
        input.model_profile,
        input.reasoning_effort.map(|effort| effort.as_str().to_string()),
    ],
)?;
```

Use the same pattern for `auxiliary_model_preferences`.

- [ ] **Step 6: Run provider and preference tests**

Run:

```bash
cargo test -p noema-core store::tests::sqlite_provider_accounts_seed_and_list store::tests::sqlite_agent_model_preference_round_trip -- --nocapture
```

Expected: pass.

- [ ] **Step 7: Write conversation repository tests**

Preserve the current durable transcript behavior:

```rust
#[tokio::test]
async fn sqlite_conversation_items_page_in_sequence_order() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .ensure_primary_conversation("human:local")
        .await
        .expect("conversation");

    let turn = store
        .append_conversation_turn(crate::NewConversationTurn {
            turn_id: "turn:test".to_string(),
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            status: crate::ConversationTurnStatus::Running,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");

    store
        .append_conversation_item(crate::NewConversationItem {
            item_id: "item:user".to_string(),
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: crate::ConversationItemKind::UserText,
            status: crate::ConversationItemStatus::Completed,
            author_actor_id: "human:local".to_string(),
            content_text: Some("hello".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("append user");

    let page = store
        .list_visible_conversation_item_page(
            &conversation.conversation_id,
            crate::ConversationItemPageCursor::Latest,
            20,
        )
        .await
        .expect("page");

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].item_id, "item:user");
    assert_eq!(page.items[0].sequence_index, 1);
}
```

Run:

```bash
cargo test -p noema-core store::tests::sqlite_conversation_items_page_in_sequence_order -- --nocapture
```

Expected: fail before porting `conversations.rs`.

- [ ] **Step 8: Port `conversations.rs` and `context_summaries.rs`**

Use `append_item_lock` around sequence assignment:

```rust
let _guard = self.append_item_lock.lock().await;
self.with_connection(|conn| {
    let next_sequence = conn.query_row(
        "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM conversation_items WHERE conversation_id = ?1",
        [&item.conversation_id],
        |row| row.get::<_, i64>(0),
    )?;
    conn.execute(
        r#"
        INSERT INTO conversation_items
          (item_id, conversation_id, turn_id, parent_item_id, sequence_index, kind, status,
           author_actor_id, content_text, payload_json, metadata_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        rusqlite::params![
            item.item_id,
            item.conversation_id,
            item.turn_id,
            item.parent_item_id,
            next_sequence,
            item.kind.as_str(),
            item.status.as_str(),
            item.author_actor_id,
            item.content_text,
            crate::store::sqlite::json_to_string(&item.payload_json)?,
            crate::store::sqlite::json_to_string(&item.metadata)?,
        ],
    )?;
    Ok(next_sequence)
})
.await?;
```

For transcript page queries, use `ORDER BY sequence_index DESC` for older windows and reverse in Rust before returning if the current API expects ascending display order.

- [ ] **Step 9: Run conversation and context tests**

Run:

```bash
cargo test -p noema-core store::tests::sqlite_conversation_items_page_in_sequence_order -- --nocapture
cargo test -p noema-core store::tests -- --nocapture
```

Expected: store tests pass except tests still owned by unported MCP/claim modules.

- [ ] **Step 10: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/store/agents.rs crates/noema-core/src/store/provider_accounts.rs crates/noema-core/src/store/provider_capability_bindings.rs crates/noema-core/src/store/agent_runtime_preferences.rs crates/noema-core/src/store/auxiliary_model_preferences.rs crates/noema-core/src/store/conversations.rs crates/noema-core/src/store/context_summaries.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/tests.rs
git commit -m "Port core repositories to SQLite"
```

## Task 3: Port MCP And Approval Repositories

**Files:**
- Modify: `crates/noema-core/src/store/mcp.rs`
- Modify: `crates/noema-core/src/store/mcp/*.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/tests/mcp.rs`

**Interfaces:**
- Consumes: `NoemaStore::with_connection`
- Produces: existing MCP store method signatures used by GraphQL and Capability Gateway.

- [ ] **Step 1: Add SQLite DDL for MCP tables**

Append tables from the design to `STORE_SCHEMA_SQL` using the same column names as current record structs:

```sql
CREATE TABLE IF NOT EXISTS mcp_servers (
  mcp_server_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  transport_kind TEXT NOT NULL CHECK (transport_kind IN ('stdio', 'sse', 'streamable_http')),
  safe_config_json TEXT NOT NULL DEFAULT '{}',
  auth_status TEXT NOT NULL CHECK (auth_status IN ('none', 'needs_auth', 'authenticated', 'unavailable')),
  health_status TEXT NOT NULL CHECK (health_status IN ('unknown', 'healthy', 'unavailable')),
  enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
  metadata_fingerprint TEXT,
  last_discovered_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS mcp_tools (
  mcp_tool_id TEXT PRIMARY KEY NOT NULL,
  mcp_server_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  input_schema_json TEXT NOT NULL DEFAULT '{}',
  output_schema_json TEXT,
  annotations_json TEXT NOT NULL DEFAULT '{}',
  metadata_fingerprint TEXT NOT NULL,
  discovered_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(mcp_server_id, name)
);

CREATE TABLE IF NOT EXISTS tool_calibrations (
  calibration_id TEXT PRIMARY KEY NOT NULL,
  mcp_tool_id TEXT NOT NULL UNIQUE,
  read_classification TEXT NOT NULL CHECK (read_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  write_classification TEXT NOT NULL CHECK (write_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  export_classification TEXT NOT NULL CHECK (export_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  owner_extractors_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL CHECK (status IN ('needs_review', 'blocked_unresolved_ownership', 'ready', 'disabled')),
  reviewed_by TEXT,
  reviewed_metadata_fingerprint TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS trusted_identity_selectors (
  selector_id TEXT PRIMARY KEY NOT NULL,
  owner_scope_id TEXT NOT NULL,
  selector_kind TEXT NOT NULL CHECK (selector_kind IN ('email', 'phone', 'domain')),
  normalized_value TEXT NOT NULL,
  effect TEXT NOT NULL CHECK (effect IN ('trust', 'restrict')),
  issuer_actor_id TEXT NOT NULL,
  revoked_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(owner_scope_id, selector_kind, normalized_value)
);

CREATE TABLE IF NOT EXISTS approval_requests (
  approval_id TEXT PRIMARY KEY NOT NULL,
  action_summary TEXT NOT NULL,
  tool_invocation_id TEXT NOT NULL,
  mcp_server_id TEXT,
  mcp_tool_id TEXT,
  requester_actor_id TEXT NOT NULL,
  owner_scope_id TEXT NOT NULL,
  active_scope_id TEXT NOT NULL,
  destination_summary TEXT NOT NULL,
  data_source_summary TEXT NOT NULL,
  source_owner_identity TEXT NOT NULL,
  source_owner_trust TEXT NOT NULL CHECK (source_owner_trust IN ('trusted', 'untrusted', 'mixed', 'unresolved')),
  destination_owner_identity TEXT NOT NULL,
  destination_owner_trust TEXT NOT NULL CHECK (destination_owner_trust IN ('trusted', 'untrusted', 'mixed', 'unresolved')),
  export_summary TEXT NOT NULL,
  payload_preview_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied', 'cancelled')),
  decision_actor_id TEXT,
  decision_comment TEXT,
  decided_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

- [ ] **Step 2: Run existing MCP tests to capture failures**

Run:

```bash
cargo test -p noema-core store::tests::mcp -- --nocapture
```

Expected: fail because MCP repositories still use SurrealDB.

- [ ] **Step 3: Port MCP rows and repositories**

For each file under `crates/noema-core/src/store/mcp/`, replace `SurrealValue` rows with plain `rusqlite` row mappers. Use helper functions:

```rust
fn bool_from_i64(value: i64) -> bool {
    value != 0
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}
```

Use `serde_json::from_str` for `safe_config_json`, `input_schema_json`, `output_schema_json`, `annotations_json`, `owner_extractors_json`, and `payload_preview_json`.

- [ ] **Step 4: Verify gateway-facing queries**

Run the tests that depend on calibrated MCP advertising:

```bash
cargo test -p noema-core daemon::runtime::model_tools -- --nocapture
cargo test -p noema-core capability::gateway -- --nocapture
```

Expected: pass after repository port.

- [ ] **Step 5: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/store/mcp.rs crates/noema-core/src/store/mcp crates/noema-core/src/store/schema.rs crates/noema-core/src/store/tests/mcp.rs
git commit -m "Port MCP repositories to SQLite"
```

## Task 4: Add Supermemory Service Repository

**Files:**
- Create: `crates/noema-core/src/store/memory_service.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Produces: `MemoryServiceMode::{Managed, External}`
- Produces: `MemoryServiceStatus::{NotConfigured, Starting, Ready, Unavailable, AuthError}`
- Produces: `NoemaStore::memory_service_settings()`
- Produces: `NoemaStore::save_memory_service_settings(input: SaveMemoryServiceSettings)`
- Produces: `NoemaStore::memory_service_status()`
- Produces: `NoemaStore::save_memory_service_status(status: MemoryServiceStatusRecord)`
- Produces: `NoemaStore::insert_memory_ingest_job(input: NewMemoryIngestJob)`
- Produces: `NoemaStore::mark_memory_ingest_job_submitted(job_id: &str)`
- Produces: `NoemaStore::mark_memory_ingest_job_failed(job_id: &str, code: &str, message: &str)`

- [ ] **Step 1: Write memory service repository tests**

Add:

```rust
#[tokio::test]
async fn sqlite_memory_service_defaults_to_managed() {
    let store = test_store().await;

    let settings = store.memory_service_settings().await.expect("settings");
    let status = store.memory_service_status().await.expect("status");

    assert_eq!(settings.mode, crate::MemoryServiceMode::Managed);
    assert_eq!(settings.base_url, "http://127.0.0.1:6767");
    assert_eq!(settings.port, Some(6767));
    assert_eq!(status.status, crate::MemoryServiceStatus::NotConfigured);
}

#[tokio::test]
async fn sqlite_memory_service_settings_round_trip_external() {
    let store = test_store().await;

    store
        .save_memory_service_settings(crate::SaveMemoryServiceSettings {
            mode: crate::MemoryServiceMode::External,
            base_url: "http://127.0.0.1:7777".to_string(),
            port: None,
            provider_account_id: Some("provider_account:openai:default".to_string()),
            provider_kind: Some("openai".to_string()),
            model_profile: Some("gpt-5.1".to_string()),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Low),
        })
        .await
        .expect("save settings");

    let settings = store.memory_service_settings().await.expect("settings");
    assert_eq!(settings.mode, crate::MemoryServiceMode::External);
    assert_eq!(settings.base_url, "http://127.0.0.1:7777");
    assert_eq!(settings.reasoning_effort, Some(crate::provider::ReasoningEffort::Low));
}
```

Run:

```bash
cargo test -p noema-core store::tests::sqlite_memory_service_defaults_to_managed store::tests::sqlite_memory_service_settings_round_trip_external -- --nocapture
```

Expected: fail before implementation.

- [ ] **Step 2: Implement memory service types**

Create `crates/noema-core/src/store/memory_service.rs`:

```rust
use crate::provider::ReasoningEffort;

use super::{NoemaStore, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceMode {
    Managed,
    External,
}

impl MemoryServiceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::External => "external",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "managed" => Ok(Self::Managed),
            "external" => Ok(Self::External),
            _ => Err(StoreError::InvalidEnum {
                kind: "memory service mode",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceStatus {
    NotConfigured,
    Starting,
    Ready,
    Unavailable,
    AuthError,
}

impl MemoryServiceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
            Self::AuthError => "auth_error",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "not_configured" => Ok(Self::NotConfigured),
            "starting" => Ok(Self::Starting),
            "ready" => Ok(Self::Ready),
            "unavailable" => Ok(Self::Unavailable),
            "auth_error" => Ok(Self::AuthError),
            _ => Err(StoreError::InvalidEnum {
                kind: "memory service status",
                value: value.to_string(),
            }),
        }
    }
}
```

Add record structs with the exact fields from the Interfaces block.

- [ ] **Step 3: Implement repository methods**

Use `INSERT ... ON CONFLICT(settings_id)` for settings and `status_id` for status:

```rust
conn.execute(
    r#"
    INSERT INTO memory_service_settings
      (settings_id, mode, base_url, port, provider_account_id, provider_kind, model_profile, reasoning_effort, updated_at)
    VALUES ('default', ?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(settings_id) DO UPDATE SET
      mode = excluded.mode,
      base_url = excluded.base_url,
      port = excluded.port,
      provider_account_id = excluded.provider_account_id,
      provider_kind = excluded.provider_kind,
      model_profile = excluded.model_profile,
      reasoning_effort = excluded.reasoning_effort,
      updated_at = excluded.updated_at
    "#,
    rusqlite::params![
        input.mode.as_str(),
        input.base_url,
        input.port,
        input.provider_account_id,
        input.provider_kind,
        input.model_profile,
        input.reasoning_effort.map(|effort| effort.as_str().to_string()),
    ],
)?;
```

- [ ] **Step 4: Run memory service tests**

Run:

```bash
cargo test -p noema-core store::tests::sqlite_memory_service_defaults_to_managed store::tests::sqlite_memory_service_settings_round_trip_external -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/store.rs crates/noema-core/src/store/memory_service.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/tests.rs
git commit -m "Add memory service SQLite settings"
```

## Task 5: Add Supermemory Client And Lifecycle

**Files:**
- Create: `crates/noema-core/src/supermemory.rs`
- Create: `crates/noema-core/src/supermemory/client.rs`
- Create: `crates/noema-core/src/supermemory/config.rs`
- Create: `crates/noema-core/src/supermemory/lifecycle.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/runtime_host.rs`
- Create: `crates/noema-core/src/supermemory/tests.rs`

**Interfaces:**
- Consumes: `MemoryServiceSettingsRecord`
- Produces: `SupermemoryClient::search_memories(&self, request: SupermemorySearchRequest) -> Result<SupermemorySearchResponse, SupermemoryClientError>`
- Produces: `SupermemoryClient::ingest_conversation(&self, request: SupermemoryConversationIngestRequest) -> Result<(), SupermemoryClientError>`
- Produces: `SupermemoryLifecycle::start(paths: &NoemaPaths, settings: &MemoryServiceSettingsRecord, store: NoemaStore, system_errors: SystemErrorLogger) -> Result<Self, SupermemoryLifecycleError>`
- Produces: `SupermemoryLifecycle::shutdown(self) -> impl Future<Output = ()>`

- [ ] **Step 1: Create module facade**

Add to `crates/noema-core/src/lib.rs`:

```rust
pub mod supermemory;

pub use supermemory::{
    SupermemoryClient, SupermemoryClientError, SupermemoryConversationIngestRequest,
    SupermemoryLifecycle, SupermemorySearchRequest, SupermemorySearchResponse,
};
```

Create `crates/noema-core/src/supermemory.rs`:

```rust
//! Supermemory local service client and lifecycle support.

mod client;
mod config;
mod lifecycle;

#[cfg(test)]
mod tests;

pub use client::{
    SupermemoryClient, SupermemoryClientError, SupermemoryConversationIngestRequest,
    SupermemorySearchRequest, SupermemorySearchResponse, SupermemorySearchResult,
};
pub use config::SupermemoryRuntimeConfig;
pub use lifecycle::{SupermemoryLifecycle, SupermemoryLifecycleError};
```

- [ ] **Step 2: Write fake HTTP client tests**

In `crates/noema-core/src/supermemory/tests.rs`, use a local `tokio::net::TcpListener` fake server:

```rust
#[tokio::test]
async fn supermemory_search_posts_v4_search() {
    let server = FakeSupermemoryServer::start(
        "/v4/search",
        serde_json::json!({
            "results": [{
                "id": "mem_1",
                "memory": "Kevin prefers concise plans",
                "metadata": {"source": "test"},
                "updatedAt": "2026-07-07T12:00:00.000Z",
                "similarity": 0.91
            }],
            "timing": 2,
            "total": 1
        }),
    )
    .await;
    let client = crate::SupermemoryClient::new(server.base_url(), Some("sm_test".to_string()));

    let response = client
        .search_memories(crate::SupermemorySearchRequest {
            query: "preferences".to_string(),
            container_tag: "human:local".to_string(),
            limit: 5,
        })
        .await
        .expect("search");

    assert_eq!(response.results[0].id, "mem_1");
    assert_eq!(server.last_authorization().await.as_deref(), Some("Bearer sm_test"));
}
```

Implement `FakeSupermemoryServer` in the same test module with `TcpListener`, one accepted request, and stored request headers/body.

Run:

```bash
cargo test -p noema-core supermemory::tests::supermemory_search_posts_v4_search -- --nocapture
```

Expected: fail before client implementation.

- [ ] **Step 3: Implement client**

In `client.rs`:

```rust
#[derive(Debug, Clone)]
pub struct SupermemoryClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl SupermemoryClient {
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http: reqwest::Client::new(),
        }
    }

    pub async fn search_memories(
        &self,
        request: SupermemorySearchRequest,
    ) -> Result<SupermemorySearchResponse, SupermemoryClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v4/search", self.base_url))
            .json(&serde_json::json!({
                "q": request.query,
                "containerTag": request.container_tag,
                "limit": request.limit,
                "searchMode": "memories",
                "include": {
                    "documents": false,
                    "summaries": false,
                    "relatedMemories": false,
                    "forgottenMemories": false,
                    "chunks": false
                }
            }));
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }
        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(SupermemoryClientError::Status(response.status().as_u16()));
        }
        response.json::<SupermemorySearchResponse>().await.map_err(Into::into)
    }
}
```

Define request/response structs using `serde::{Serialize, Deserialize}` and `#[serde(rename = "updatedAt")]`.

- [ ] **Step 4: Run client test**

Run:

```bash
cargo test -p noema-core supermemory::tests::supermemory_search_posts_v4_search -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Implement lifecycle shell**

In `lifecycle.rs`, implement child process startup and no-op external mode:

```rust
pub struct SupermemoryLifecycle {
    child: Option<tokio::process::Child>,
}

impl SupermemoryLifecycle {
    pub async fn start(
        paths: &crate::NoemaPaths,
        settings: &crate::MemoryServiceSettingsRecord,
        store: crate::NoemaStore,
        system_errors: crate::SystemErrorLogger,
    ) -> Result<Self, SupermemoryLifecycleError> {
        match settings.mode {
            crate::MemoryServiceMode::External => Ok(Self { child: None }),
            crate::MemoryServiceMode::Managed => {
                tokio::fs::create_dir_all(paths.supermemory_data_dir()).await?;
                tokio::fs::create_dir_all(paths.supermemory_secrets_dir()).await?;
                store
                    .save_memory_service_status(crate::MemoryServiceStatusRecord::starting())
                    .await?;
                let mut command = tokio::process::Command::new("supermemory-server");
                command
                    .env("SUPERMEMORY_DATA_DIR", paths.supermemory_data_dir())
                    .env("SUPERMEMORY_PORT", settings.port.unwrap_or(6767).to_string())
                    .kill_on_drop(true);
                let child = command.spawn().map_err(|error| {
                    system_errors.try_append(
                        crate::SystemErrorEvent::new(
                            "supermemory_start_failed",
                            "Supermemory managed process could not start",
                        )
                        .with_error_chain([error.to_string()])
                    );
                    SupermemoryLifecycleError::Start(error.to_string())
                })?;
                Ok(Self { child: Some(child) })
            }
        }
    }

    pub async fn shutdown(mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill().await;
        }
    }
}
```

Import `crate::SystemErrorEvent` or use the fully qualified path shown above.

- [ ] **Step 6: Wire runtime host**

Modify `NoemaRuntimeHost` to include:

```rust
supermemory: Option<crate::SupermemoryLifecycle>,
```

After store setup:

```rust
let memory_settings = store
    .memory_service_settings()
    .await
    .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
let supermemory = match crate::SupermemoryLifecycle::start(
    &paths,
    &memory_settings,
    store.clone(),
    system_errors.clone(),
)
.await
{
    Ok(lifecycle) => Some(lifecycle),
    Err(error) => {
        system_errors.try_append(
            crate::SystemErrorEvent::new(
                "supermemory_lifecycle_unavailable",
                "Supermemory lifecycle is unavailable",
            )
            .with_error_chain([error.to_string()])
        );
        None
    }
};
```

In `shutdown`:

```rust
if let Some(supermemory) = self.supermemory {
    supermemory.shutdown().await;
}
self.runtime.shutdown().await;
```

- [ ] **Step 7: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/lib.rs crates/noema-core/src/runtime_host.rs crates/noema-core/src/supermemory.rs crates/noema-core/src/supermemory
git commit -m "Add Supermemory client and lifecycle"
```

## Task 6: Replace `search_memory` With Supermemory Search

**Files:**
- Modify: `crates/noema-core/src/daemon/memory/tool.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Consumes: `SupermemoryClient`
- Produces: `container_tag_for_scope(scope_id: &str) -> Result<String, MemoryToolError>`
- Produces: `execute_search_memory(store, client, context, call_id, payload)`

- [ ] **Step 1: Add unit tests for scope mapping**

In `daemon/memory/tool.rs` tests:

```rust
#[test]
fn supermemory_container_tag_preserves_safe_scope_ids() {
    assert_eq!(
        container_tag_for_scope("human:local").expect("tag"),
        "human:local"
    );
    assert_eq!(
        container_tag_for_scope("conversation:abc_123").expect("tag"),
        "conversation:abc_123"
    );
}

#[test]
fn supermemory_container_tag_rejects_unknown_scope_shape() {
    let error = container_tag_for_scope("not allowed").expect_err("invalid");
    assert!(error.to_string().contains("unsupported scope_id"));
}
```

Run:

```bash
cargo test -p noema-core daemon::memory::tool::tests::supermemory_container_tag_preserves_safe_scope_ids daemon::memory::tool::tests::supermemory_container_tag_rejects_unknown_scope_shape -- --nocapture
```

Expected: fail before implementation.

- [ ] **Step 2: Implement container tag mapping**

Add:

```rust
fn container_tag_for_scope(scope_id: &str) -> Result<String, MemoryToolError> {
    let allowed_prefix = ["human:", "conversation:", "project:", "workspace:", "agent:"]
        .iter()
        .any(|prefix| scope_id.starts_with(prefix));
    let valid_chars = scope_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':'));
    if allowed_prefix && valid_chars {
        Ok(scope_id.to_string())
    } else {
        Err(MemoryToolError::InvalidArguments(format!(
            "unsupported scope_id: {scope_id}"
        )))
    }
}
```

- [ ] **Step 3: Change `execute_search_memory` signature**

Use:

```rust
pub(in crate::daemon) async fn execute_search_memory(
    store: &NoemaStore,
    client: Option<&crate::SupermemoryClient>,
    context: &MemoryToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> MemoryToolResult
```

If `client` is `None`, return:

```rust
json!({
    "error": "memory service unavailable"
})
```

- [ ] **Step 4: Implement bounded Supermemory search**

Inside `execute_search_memory_inner`, replace `store.retrieve_claims_scoped` with:

```rust
let client = client.ok_or_else(|| {
    MemoryToolError::Unavailable("memory service unavailable".to_string())
})?;
let tags = if arguments.scope_ids.is_empty() {
    trusted_active_scope_ids(context)
} else {
    arguments.scope_ids.clone()
};
let mut memories = Vec::new();
for scope_id in tags {
    let container_tag = container_tag_for_scope(&scope_id)?;
    let response = client
        .search_memories(crate::SupermemorySearchRequest {
            query: arguments.query.clone(),
            container_tag: container_tag.clone(),
            limit: arguments.limit(),
        })
        .await
        .map_err(|error| MemoryToolError::Unavailable(error.to_string()))?;
    for result in response.results {
        if let Some(memory) = result.memory {
            memories.push(json!({
                "id": result.id,
                "kind": "supermemory",
                "memory": memory,
                "score": result.similarity,
                "updated_at": result.updated_at,
                "scope_id": scope_id,
                "container_tag": container_tag,
                "metadata": result.metadata.unwrap_or_else(|| json!({})),
            }));
        }
    }
}
memories.sort_by(|left, right| {
    let left_score = left["score"].as_f64().unwrap_or(0.0);
    let right_score = right["score"].as_f64().unwrap_or(0.0);
    right_score
        .partial_cmp(&left_score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| left["id"].as_str().cmp(&right["id"].as_str()))
});
memories.truncate(arguments.limit());
Ok(json!({
    "memories": memories,
    "omissions": [],
    "scope_ids": arguments.scope_ids,
}))
```

Add `MemoryToolError::Unavailable(String)`.

- [ ] **Step 5: Wire runtime client into local tool execution**

Add a `supermemory_client: Option<SupermemoryClient>` field to `CodexRuntimeActor` and `CodexRuntimeHandle::spawn_from_config`. Build it from `store.memory_service_settings()`:

```rust
let memory_settings = store.memory_service_settings().await.ok();
let supermemory_client = memory_settings.map(|settings| {
    crate::SupermemoryClient::new(settings.base_url, None)
});
```

Pass it to `execute_search_memory`.

- [ ] **Step 6: Run memory tool tests**

Run:

```bash
cargo test -p noema-core daemon::memory::tool -- --nocapture
```

Expected: pass after updating expectations from claim-shaped payloads to Supermemory-shaped payloads.

- [ ] **Step 7: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/memory/tool.rs crates/noema-core/src/daemon/runtime/local_tools.rs crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/daemon/runtime/handle.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Route memory search through Supermemory"
```

## Task 7: Remove Noema Memory Proposals And `/remember`

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/noema_response_stream.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/daemon/prompts.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Delete: `crates/noema-core/src/daemon/memory/pipeline.rs`
- Delete: `crates/noema-core/src/daemon/memory/writes.rs`
- Delete: `crates/noema-core/src/memory/extraction.rs`
- Delete: `crates/noema-core/src/memory/consolidation.rs`

**Interfaces:**
- Produces: provider response envelope without `memory_proposals`.
- Produces: no explicit-memory command path.
- Consumes: `search_memory` remains available.

- [ ] **Step 1: Write response parser tests**

In provider contract tests, add:

```rust
#[test]
fn noema_response_no_longer_requires_memory_proposals() {
    let parsed = crate::provider::parse_noema_response(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"ok"}],"tool_calls":[]}"#,
    )
    .expect("response parses");

    assert_eq!(parsed.responses.len(), 1);
    assert!(parsed.tool_calls.is_empty());
}

#[test]
fn noema_response_rejects_memory_proposals_field() {
    let error = crate::provider::parse_noema_response(
        r#"{"response_status":"final","responses":[],"tool_calls":[],"memory_proposals":[]}"#,
    )
    .expect_err("legacy field rejected");

    assert!(error.to_string().contains("memory_proposals"));
}
```

Run:

```bash
cargo test -p noema-core provider::contract::tests::noema_response_no_longer_requires_memory_proposals provider::contract::tests::noema_response_rejects_memory_proposals_field -- --nocapture
```

Expected: fail before parser update.

- [ ] **Step 2: Update provider response structs**

Remove `memory_proposals` from the required response struct and set `#[serde(deny_unknown_fields)]` so the legacy field fails fast.

Update prompt schema examples in provider adapters from:

```json
{"response_status":"final","responses":[],"tool_calls":[],"memory_proposals":[]}
```

to:

```json
{"response_status":"final","responses":[],"tool_calls":[]}
```

- [ ] **Step 3: Remove `/remember` detection**

Delete or stop calling `explicit_memory_content`. Add daemon test:

```rust
#[tokio::test]
async fn slash_remember_is_ordinary_chat_text() {
    let (runtime, store) = test_runtime_with_provider_output(vec![assistant_text_output("ok")]).await;
    let accepted = runtime
        .send_user_turn(crate::daemon::protocol::SendTurn {
            conversation_id: "conversation:test".to_string(),
            user_input: "/remember I like trains".to_string(),
            cwd: None,
        })
        .await
        .expect("turn accepted");

    assert_eq!(accepted.conversation_id, "conversation:test");
    let items = store
        .list_visible_conversation_item_page(
            "conversation:test",
            crate::ConversationItemPageCursor::Latest,
            20,
        )
        .await
        .expect("items");
    assert!(items.items.iter().any(|item| item.content_text.as_deref() == Some("/remember I like trains")));
    assert!(!items.items.iter().any(|item| {
        item.payload_json
            .get("activity_kind")
            .and_then(serde_json::Value::as_str)
            == Some("memory_save")
    }));
}
```

Use the existing daemon test harness setup functions in `daemon/tests.rs` and keep the assertion against `payload_json["activity_kind"]`.

- [ ] **Step 4: Remove memory write activity persistence**

Remove calls that create `memory_save`, `memory_extraction`, and `memory_proposals` transcript cards. Keep generic tool markers for `search_memory`.

- [ ] **Step 5: Run provider and daemon tests**

Run:

```bash
cargo test -p noema-core provider::contract -- --nocapture
cargo test -p noema-core daemon::tests::slash_remember_is_ordinary_chat_text -- --nocapture
```

Expected: pass.

- [ ] **Step 6: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider crates/noema-core/src/daemon crates/noema-core/src/memory
git commit -m "Remove Noema memory proposals"
```

## Task 8: Replace GraphQL Memory API With Settings API

**Files:**
- Modify: `crates/noema-core/src/graphql/memory.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/graphql/local_status.rs`
- Modify: `crates/noema-core/src/graphql/resolvers.rs`

**Interfaces:**
- Produces: `memorySettings: MemorySettings!`
- Produces: `saveMemoryServiceSettings(input: SaveMemoryServiceSettingsInput!): MemorySettings!`
- Produces: `checkMemoryService: MemoryServiceStatus!`
- Removes: old graph claim fields.

- [ ] **Step 1: Write GraphQL tests**

Add tests:

```rust
#[tokio::test]
async fn memory_settings_query_returns_defaults() {
    let store = crate::store::tests::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));

    let response = schema
        .execute(Request::new("{ memorySettings { mode baseUrl port status { status } } }"))
        .await
        .into_result()
        .expect("query");

    assert_eq!(
        response.data,
        Value::from_json(serde_json::json!({
            "memorySettings": {
                "mode": "MANAGED",
                "baseUrl": "http://127.0.0.1:6767",
                "port": 6767,
                "status": {"status": "NOT_CONFIGURED"}
            }
        }))
        .expect("json")
    );
}

#[tokio::test]
async fn old_memory_graph_field_is_not_in_schema() {
    let schema = build_schema(GraphqlState::for_tests());
    let response = schema
        .execute(Request::new("{ memoryGraph { nodes { nodeId } } }"))
        .await;

    assert!(!response.errors.is_empty());
    assert!(response.errors[0].message.contains("memoryGraph"));
}
```

Run:

```bash
cargo test -p noema-core graphql::schema::tests::memory_settings_query_returns_defaults graphql::schema::tests::old_memory_graph_field_is_not_in_schema -- --nocapture
```

Expected: fail before GraphQL update.

- [ ] **Step 2: Replace `graphql/memory.rs` types**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlMemoryServiceMode {
    Managed,
    External,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlMemoryServiceStatusKind {
    NotConfigured,
    Starting,
    Ready,
    Unavailable,
    AuthError,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemoryServiceStatus {
    pub status: GraphqlMemoryServiceStatusKind,
    pub checked_at: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMemorySettings {
    pub mode: GraphqlMemoryServiceMode,
    pub base_url: String,
    pub port: Option<i32>,
    pub status: GraphqlMemoryServiceStatus,
    pub model_preference: Option<super::agents::GraphqlAgentModelPreference>,
    pub model_options: Vec<super::agents::GraphqlModelProviderOption>,
}
```

Use existing model option builders from `graphql/agents.rs`; if private, move the shared builder to a small `graphql/model_options.rs`.

- [ ] **Step 3: Add query and mutations**

In `schema.rs`, remove old memory claim query methods and add:

```rust
async fn memory_settings(&self, ctx: &Context<'_>) -> Result<GraphqlMemorySettings> {
    memory::memory_settings(ctx.data::<GraphqlState>()?).await
}

async fn save_memory_service_settings(
    &self,
    ctx: &Context<'_>,
    input: GraphqlSaveMemoryServiceSettingsInput,
) -> Result<GraphqlMemorySettings> {
    memory::save_memory_service_settings(ctx.data::<GraphqlState>()?, input).await
}

async fn check_memory_service(&self, ctx: &Context<'_>) -> Result<GraphqlMemoryServiceStatus> {
    memory::check_memory_service(ctx.data::<GraphqlState>()?).await
}
```

- [ ] **Step 4: Run GraphQL tests**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::memory_settings_query_returns_defaults graphql::schema::tests::old_memory_graph_field_is_not_in_schema -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/graphql
git commit -m "Replace memory GraphQL API with settings"
```

## Task 9: Remove SurrealDB And Graph Memory Modules

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-core/src/store.rs`
- Delete: `crates/noema-core/src/store/claims.rs`
- Delete: `crates/noema-core/src/store/claims/`
- Delete: `crates/noema-core/src/store/ontology.rs`
- Delete: `crates/noema-core/src/store/retrieval.rs`
- Delete: `crates/noema-core/src/memory/consolidation.rs`
- Delete: `crates/noema-core/src/memory/extraction.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/memory.rs`

**Interfaces:**
- Produces: no `surrealdb` dependency in workspace or noema-core.
- Produces: no `MemoryClaim*`, `MemoryGraph*`, `PredicateProposal*`, `ClaimRetrieval*` public exports.

- [ ] **Step 1: Remove exports and modules**

In `store.rs`, remove:

```rust
mod claims;
mod ontology;
mod retrieval;
```

In `lib.rs`, remove public exports for claim, graph, predicate proposal, and memory extraction/consolidation types that no caller uses after Tasks 7 and 8.

- [ ] **Step 2: Delete files**

Run:

```bash
git rm crates/noema-core/src/store/claims.rs crates/noema-core/src/store/retrieval.rs crates/noema-core/src/store/ontology.rs
git rm -r crates/noema-core/src/store/claims
```

Remove the old memory extraction and consolidation modules:

```bash
git rm crates/noema-core/src/memory/extraction.rs crates/noema-core/src/memory/consolidation.rs crates/noema-core/src/memory/model.rs
```

Keep `memory/types.rs` and `memory/error.rs` only when the compiler shows a concrete remaining caller for `MemoryType`, `Sensitivity`, or `MemoryPersistenceError`; otherwise remove those files in the same commit and update `memory.rs` to an empty facade or delete the module.

- [ ] **Step 3: Remove SurrealDB dependency**

Delete from `Cargo.toml`:

```toml
surrealdb = { version = "3", default-features = false, features = ["kv-rocksdb"] }
```

Delete from `crates/noema-core/Cargo.toml`:

```toml
surrealdb.workspace = true
```

- [ ] **Step 4: Search for leftovers**

Run:

```bash
rg -n "surrealdb|Surreal|SurrealValue|MemoryClaim|MemoryGraph|PredicateProposal|ClaimRetrieval|graph claim|graph-memory|predicate proposal" crates/noema-core/src crates/noema-core/web/src
```

Expected: no code references except docs or deliberately retained historical text.

- [ ] **Step 5: Run check**

Run:

```bash
cargo check -p noema-core
```

Expected: pass.

- [ ] **Step 6: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add Cargo.toml Cargo.lock crates/noema-core/Cargo.toml crates/noema-core/src
git commit -m "Remove SurrealDB graph memory store"
```

## Task 10: Add Settings > Memory Frontend

**Files:**
- Create: `crates/noema-core/web/src/routes/settings/memory.tsx`
- Create: `crates/noema-core/web/src/components/settings/MemorySettingsPane.tsx`
- Create: `crates/noema-core/web/src/components/settings/MemorySettingsPaneContent.tsx`
- Create: `crates/noema-core/web/src/components/settings/memorySettingsModel.ts`
- Modify: `crates/noema-core/web/src/pages/SettingsPage.tsx`
- Modify: `crates/noema-core/web/src/app/routes.ts`
- Modify: `crates/noema-core/web/src/components/shell/shellNavigation.ts`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Regenerate: `crates/noema-core/web/src/generated/graphql.ts`
- Regenerate: `crates/noema-core/web/src/generated/schema.graphql`
- Regenerate: `crates/noema-core/web/src/routeTree.gen.ts`

**Interfaces:**
- Consumes: GraphQL `memorySettings`, `saveMemoryServiceSettings`, `checkMemoryService`.
- Produces: Settings section `"memory"`.
- Produces: `/settings/memory`.
- Produces: `/memory` redirect to `/settings/memory`.

- [ ] **Step 1: Add route model tests**

Modify `crates/noema-core/web/src/app/routes.test.ts`:

```ts
test("routes memory to settings memory", () => {
  expect(routeFromPathname("/memory")).toEqual({
    kind: "settings",
    section: "memory"
  });
  expect(pathForRoute({ kind: "settings", section: "memory" })).toBe("/settings/memory");
});
```

Run:

```bash
cd crates/noema-core/web
bun test src/app/routes.test.ts
```

Expected: fail before route update.

- [ ] **Step 2: Update route types**

In `routes.ts`, add `"memory"` to `SettingsSection`, remove `memory_home` and `memory_graph` route kinds, and add `"/settings/memory"` to `AppPath`.

Use:

```ts
if (pathname === "/memory" || pathname === "/settings/memory") {
  return { kind: "settings", section: "memory" };
}
```

In `pathForRoute`, add:

```ts
case "memory":
  return "/settings/memory";
```

- [ ] **Step 3: Update shell navigation**

In `shellNavigation.ts`, remove the L0 `memory` item and add a Settings section:

```ts
{
  kind: "section",
  item: { section: "memory", itemId: "settings.memory", label: "Memory", icon: Brain }
}
```

Update `ShellMenuItemId` to include `"settings.memory"` and remove the primary `"memory"` item if no longer used.

- [ ] **Step 4: Add GraphQL operations**

In `operations.ts`, add:

```ts
export const MEMORY_SETTINGS_QUERY = gql`
  query MemorySettings {
    memorySettings {
      mode
      baseUrl
      port
      status {
        status
        checkedAt
        lastErrorCode
        lastErrorMessage
      }
      modelPreference {
        providerKind
        providerAccountId
        modelProfile
        reasoningEffort
      }
      modelOptions {
        providerKind
        providerAccountId
        providerDisplayName
        status
        disabledReason
        defaultModelProfile
        profiles {
          id
          label
          disabledReason
          reasoningEfforts
          defaultReasoningEffort
        }
      }
    }
  }
`;

export const SAVE_MEMORY_SERVICE_SETTINGS_MUTATION = gql`
  mutation SaveMemoryServiceSettings($input: SaveMemoryServiceSettingsInput!) {
    saveMemoryServiceSettings(input: $input) {
      mode
      baseUrl
      port
      status { status checkedAt lastErrorCode lastErrorMessage }
    }
  }
`;

export const CHECK_MEMORY_SERVICE_MUTATION = gql`
  mutation CheckMemoryService {
    checkMemoryService {
      status
      checkedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;
```

- [ ] **Step 5: Create route**

Create `routes/settings/memory.tsx`:

```tsx
import { createFileRoute } from "@tanstack/react-router";
import { SettingsSurface } from "@/pages/SettingsPage";

export const Route = createFileRoute("/settings/memory")({
  component: MemorySettingsRoute
});

function MemorySettingsRoute() {
  return <SettingsSurface section="memory" />;
}
```

- [ ] **Step 6: Create pane container**

Create `MemorySettingsPane.tsx` using Apollo hooks:

```tsx
import { useMutation, useQuery } from "@apollo/client/react";
import {
  CHECK_MEMORY_SERVICE_MUTATION,
  MEMORY_SETTINGS_QUERY,
  SAVE_MEMORY_SERVICE_SETTINGS_MUTATION
} from "@/graphql/operations";
import { MemorySettingsPaneContent } from "./MemorySettingsPaneContent";

export function MemorySettingsPane() {
  const { data, loading, error, refetch } = useQuery(MEMORY_SETTINGS_QUERY);
  const [saveSettings, saveState] = useMutation(SAVE_MEMORY_SERVICE_SETTINGS_MUTATION);
  const [checkService, checkState] = useMutation(CHECK_MEMORY_SERVICE_MUTATION);

  return (
    <MemorySettingsPaneContent
      settings={data?.memorySettings ?? null}
      loading={loading}
      error={error ? "Memory settings could not be loaded." : null}
      saving={saveState.loading}
      checking={checkState.loading}
      saveError={saveState.error ? "Noema could not save memory settings." : null}
      onSave={async (input) => {
        await saveSettings({ variables: { input } });
        await refetch();
      }}
      onCheck={async () => {
        await checkService();
        await refetch();
      }}
    />
  );
}
```

- [ ] **Step 7: Create pane content**

Use Astryx controls already present in settings. Include `ModelPreferenceSelect`:

```tsx
export function MemorySettingsPaneContent({
  settings,
  loading,
  error,
  saving,
  checking,
  saveError,
  onSave,
  onCheck
}: MemorySettingsPaneContentProps) {
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading memory settings...</p>;
  }
  if (error || !settings) {
    return <p {...stylex.props(styles.mutedText)}>Memory settings could not be loaded.</p>;
  }
  return (
    <section {...stylex.props(styles.card)} aria-labelledby="memory-settings-title">
      <div {...stylex.props(styles.cardHeader)}>
        <div>
          <h2 id="memory-settings-title" {...stylex.props(styles.cardTitle)}>Supermemory</h2>
          <p {...stylex.props(styles.mutedText)}>{memoryStatusLabel(settings.status.status)}</p>
        </div>
        <button type="button" {...stylex.props(styles.button)} disabled={checking} onClick={() => void onCheck()}>
          Check connection
        </button>
      </div>
      <ModelPreferenceSelect
        options={settings.modelOptions}
        preference={settings.modelPreference}
        saving={saving}
        ariaLabel="Model settings for Supermemory extraction"
        onSave={(input) =>
          onSave({
            mode: settings.mode,
            baseUrl: settings.baseUrl,
            port: settings.port,
            providerAccountId: input.providerAccountId,
            modelProfile: input.modelProfile,
            reasoningEffort: input.reasoningEffort ?? null
          })
        }
      />
      {saveError ? <p {...stylex.props(styles.saveError)}>{saveError}</p> : null}
    </section>
  );
}
```

Define `memoryStatusLabel` in `memorySettingsModel.ts` with explicit cases for `NOT_CONFIGURED`, `STARTING`, `READY`, `UNAVAILABLE`, and `AUTH_ERROR`.

- [ ] **Step 8: Wire Settings page**

Add section copy in `SettingsPage.tsx`:

```ts
memory: {
  title: "Memory",
  description: "Configure the local Supermemory service Noema uses for recall."
}
```

Add switch case:

```tsx
case "memory":
  return <MemorySettingsPane />;
```

- [ ] **Step 9: Regenerate and validate web**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run gen:routes
bun test src/app/routes.test.ts
bun run lint
bun run build
```

Expected: all pass.

- [ ] **Step 10: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src
git commit -m "Add Memory settings surface"
```

## Task 11: Remove Memory Graph Frontend

**Files:**
- Delete: `crates/noema-core/web/src/routes/memory/graph.tsx`
- Delete: `crates/noema-core/web/src/pages/MemoryGraphPage.tsx`
- Delete: `crates/noema-core/web/src/components/memory/MemoryGraphFlow.tsx`
- Delete: `crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx`
- Delete: `crates/noema-core/web/src/components/memory/MemoryEntityNode.tsx`
- Delete: `crates/noema-core/web/src/components/memory/MemoryGraphDetailPanel.tsx`
- Delete: `crates/noema-core/web/src/components/memory/MemoryClaimEdge.tsx`
- Delete: `crates/noema-core/web/src/components/memory/MemoryGraphControls.tsx`
- Delete: `crates/noema-core/web/src/memory/graph.ts`
- Delete: `crates/noema-core/web/src/memory/graphLayout.ts`
- Modify: remaining memory imports and tests.

**Interfaces:**
- Produces: no React Flow graph page in first slice.
- Produces: no primary Memory sidebar item.

- [ ] **Step 1: Search references**

Run:

```bash
rg -n "MemoryGraph|memoryGraph|MemoryClaim|/memory/graph|components/memory|memory/graph" crates/noema-core/web/src
```

Expected: references remain before deletion.

- [ ] **Step 2: Delete graph files**

Run:

```bash
git rm crates/noema-core/web/src/routes/memory/graph.tsx
git rm crates/noema-core/web/src/pages/MemoryGraphPage.tsx
git rm crates/noema-core/web/src/components/memory/MemoryGraphFlow.tsx
git rm crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx
git rm crates/noema-core/web/src/components/memory/MemoryEntityNode.tsx
git rm crates/noema-core/web/src/components/memory/MemoryGraphDetailPanel.tsx
git rm crates/noema-core/web/src/components/memory/MemoryClaimEdge.tsx
git rm crates/noema-core/web/src/components/memory/MemoryGraphControls.tsx
git rm crates/noema-core/web/src/memory/graph.ts
git rm crates/noema-core/web/src/memory/graphLayout.ts
```

Replace `routes/memory.tsx` with a redirect route:

```tsx
import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/memory")({
  beforeLoad: () => {
    throw redirect({ to: "/settings/memory" });
  }
});
```

- [ ] **Step 3: Regenerate routes and verify no references**

Run:

```bash
cd crates/noema-core/web
bun run gen:routes
rg -n "MemoryGraph|memoryGraph|MemoryClaim|/memory/graph|components/memory|memory/graph" src
```

Expected: no references.

- [ ] **Step 4: Build**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src
git commit -m "Remove memory graph frontend"
```

## Task 12: Docs And Final Validation

**Files:**
- Modify: `docs/project.md`
- Modify: `docs/context/current.md`
- Modify: `docs/memory.md`
- Modify: `docs/sqlite.md`
- Modify: `docs/frontend/current-contract.md`
- Modify: `README.md`

**Interfaces:**
- Produces: docs aligned with SQLite and Supermemory.
- Produces: final validation evidence.

- [ ] **Step 1: Update project architecture docs**

Replace SurrealDB source-of-truth language in `docs/project.md` with:

```markdown
| Structured state: humans, agents, tools, conversations, transcript items, provider accounts, MCP setup, tasks, permissions, approvals, and audit events | SQLite |
| Memory truth, memory graph, memory extraction, memory updates, inferred memories, and memory search indexes | Local Supermemory |
```

Update filesystem layout:

```text
~/.noema/
  db/
    noema.sqlite3
  supermemory/
    data/
    secrets/
```

- [ ] **Step 2: Update current context**

In `docs/context/current.md`, record:

```markdown
Noema's approved storage direction is now SQLite for Noema-owned structured state and local Supermemory for memory truth and graph ownership. The implementation is a clean pre-V1 reset: no SurrealDB migration path, no Noema graph-claim tables in SQLite, no `/remember`, no memory transcript markers, and no `/memory/graph` in the first slice. `search_memory` remains explicit tool-only recall and maps trusted Noema scopes to Supermemory container tags.
```

Remove active-direction bullets that say SurrealDB is canonical or that graph claims are current target state.

- [ ] **Step 3: Update memory and frontend docs**

In `docs/memory.md`, make Supermemory the current authority:

```markdown
Current implementation direction:

- Durable memory truth and graph behavior belong to local Supermemory.
- Noema stores only memory service configuration, readiness, and ingest job diagnostics in SQLite.
- Noema uses `search_memory` as an explicit tool-only recall path.
- Memory markers, `/remember`, graph browsing, and automatic pre-turn recall are future work.
```

In `docs/frontend/current-contract.md`, remove `/memory/graph` from current routes and add `/settings/memory`.

- [ ] **Step 4: Run full validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: all pass.

- [ ] **Step 5: Final status and commit**

Run:

```bash
git status --short --branch
git diff --check
git add README.md docs/project.md docs/context/current.md docs/memory.md docs/sqlite.md docs/frontend/current-contract.md
git commit -m "Update docs for Supermemory SQLite reset"
```

## Final Ship Checklist

- [ ] Run `git status --short --branch`.
- [ ] Run `git diff --check`.
- [ ] Run `cargo fmt --all --check`.
- [ ] Run `cargo check --workspace`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run `cargo test --workspace --no-fail-fast`.
- [ ] Run `bun run gen:types`, `bun run lint`, and `bun run build` in `crates/noema-core/web`.
- [ ] Inspect `git diff --cached --stat`.
- [ ] Inspect `git diff --cached --name-status`.
- [ ] Report remaining untracked or unstaged files.
