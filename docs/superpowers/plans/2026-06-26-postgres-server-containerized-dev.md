# Postgres Server And Containerized Development Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Noema from SQLite-centered local persistence to a Postgres-backed always-on server architecture with an ergonomic Docker Compose development path.

**Architecture:** Postgres becomes the canonical structured store. SQLx is used inside explicit repository modules; daemon/runtime/web/CLI code calls repository APIs and does not embed raw persistence SQL. Docker Compose provides the default local development environment for Postgres, the Rust server, and the web build/watch loop.

**Tech Stack:** Rust, Tokio, SQLx Postgres, serde/serde_json, Docker Compose, Bun/Vite/React, existing Noema daemon/WebSocket protocol.

---

## Execution Notes

- Work on `main`; the user has explicitly approved ambitious pre-stable rewrites.
- Preserve unrelated dirty files unless a task explicitly owns them. Current unrelated dirty files before this plan: `.cargo/config.toml`, `README.md`, `crates/noema-cli/src/main.rs`, `crates/noema-core/web/package.json`, and untracked `crates/noema-cli/src/dev.rs`.
- No SQLite compatibility layer is required. SQLite code may exist only as temporary scaffolding inside an intermediate task and should be removed by the cleanup tasks.
- Prefer runtime-checked SQLx queries (`sqlx::query`, `sqlx::query_as`) during this migration. Do not introduce SQLx offline metadata in this pass.
- Database-backed tests should run against a real Postgres database. Use `NOEMA_TEST_DATABASE_URL` for tests and `NOEMA_DATABASE_URL` for app runtime.
- Commit after each task if the task reaches its verification checkpoint.

## File Structure

Create:

- `compose.yaml`: development stack for Postgres, Noema server, and web watcher.
- `Dockerfile`: development image with Rust and Bun tooling.
- `.dockerignore`: keep Docker context small.
- `.env.example`: documented local environment defaults.
- `crates/noema-core/src/database.rs`: resolved database configuration and Postgres pool creation.
- `crates/noema-core/src/memory_persistence/postgres_schema.rs`: Postgres bootstrap SQL.
- `crates/noema-core/src/memory_persistence/postgres_helpers.rs`: SQLx helpers for IDs, JSON, timestamps, and row mapping.
- `crates/noema-core/src/memory_persistence/postgres_tests.rs`: integration-style Postgres repository tests gated by `NOEMA_TEST_DATABASE_URL`.
- `docs/postgres.md`: canonical Postgres schema and local database notes.

Modify:

- `Cargo.toml`: add SQLx workspace dependency, keep `rusqlite` only until cleanup.
- `crates/noema-core/Cargo.toml`: add SQLx dependency.
- `crates/noema-core/src/lib.rs`: export database config and `PostgresMemoryRepository`.
- `crates/noema-core/src/config.rs`: load `NOEMA_DATABASE_URL` / storage config for daemon.
- `crates/noema-core/src/home.rs`: update default config text.
- `crates/noema-core/src/paths.rs`: remove SQLite-specific canonical database language.
- `crates/noema-core/src/daemon/protocol.rs`: replace `database_path` with `database_url`.
- `crates/noema-core/src/daemon/server.rs`: pass database URL to runtime and web state.
- `crates/noema-core/src/daemon/runtime.rs`: use async Postgres repository methods.
- `crates/noema-core/src/daemon/web/mod.rs`: replay from async Postgres repository.
- `crates/noema-core/src/memory_persistence.rs`: reframe as Postgres-backed persistence and export renamed repository.
- `crates/noema-core/src/memory_persistence/error.rs`: replace SQLite errors with Postgres/config errors.
- `crates/noema-core/src/memory_persistence/objects.rs`: make object-reference validation async SQLx.
- `crates/noema-core/src/memory_persistence/conversations.rs`: port conversation persistence to SQLx.
- `crates/noema-core/src/memory_persistence/provenance.rs`: port memory/provenance/delete behavior to SQLx.
- `crates/noema-core/src/memory_persistence/context_packets.rs`: port context packet writes to SQLx.
- `crates/noema-core/src/memory_persistence/repository.rs`: replace `SqliteMemoryRepository` with `PostgresMemoryRepository`.
- `crates/noema-core/src/memory_persistence/queries.rs`: replace SQLite query text with Postgres query text.
- `crates/noema-core/src/memory_persistence/helpers.rs`: remove rusqlite row helpers; keep pure parsing helpers and Postgres helpers.
- `crates/noema-core/src/sqlite_memory_retrieval.rs`: replace with Postgres retrieval or fold into `memory_persistence`.
- `crates/noema-cli/src/inspection.rs`: open Postgres repository from config/env.
- `docs/project.md`, `docs/context/current.md`, `docs/memory.md`, `docs/harness*.md`, `docs/frontend/*.md`: replace target-architecture SQLite/product-version framing.

Delete by final cleanup:

- `docs/sqlite.md` or replace it with `docs/postgres.md`.
- SQLite-only tests under `crates/noema-core/src/memory_persistence/tests/*` after equivalent Postgres tests exist.
- `rusqlite` workspace dependency if no remaining code uses it.

---

### Task 1: Add Postgres And Container Development Foundation

**Files:**
- Create: `compose.yaml`
- Create: `Dockerfile`
- Create: `.dockerignore`
- Create: `.env.example`
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Add workspace dependencies**

Modify `Cargo.toml` under `[workspace.dependencies]`:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls", "postgres", "json"] }
```

Keep `rusqlite` for now. It will be removed in the cleanup task after code is ported.

- [ ] **Step 2: Add core dependency**

Modify `crates/noema-core/Cargo.toml`:

```toml
sqlx.workspace = true
```

- [ ] **Step 3: Add development Dockerfile**

Create `Dockerfile`:

```dockerfile
FROM rust:1.96-bookworm AS dev

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates unzip pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

WORKDIR /workspace

CMD ["bash"]
```

- [ ] **Step 4: Add Compose stack**

Create `compose.yaml`:

```yaml
services:
  postgres:
    image: postgres:17
    environment:
      POSTGRES_USER: noema
      POSTGRES_PASSWORD: noema
      POSTGRES_DB: noema
      PGDATA: /noema/db/postgres
    ports:
      - "127.0.0.1:5432:5432"
    volumes:
      - ${NOEMA_HOME:-${HOME}/.noema}:/noema
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U noema -d noema"]
      interval: 2s
      timeout: 3s
      retries: 20

  noema-server:
    build:
      context: .
      target: dev
    working_dir: /workspace
    command: ["cargo", "run", "-p", "noema-cli", "--", "start"]
    environment:
      NOEMA_DATABASE_URL: postgres://noema:noema@postgres:5432/noema
      NOEMA_HOME: /noema
      NOEMA_OPENAI__API_KEY: ${NOEMA_OPENAI__API_KEY:-}
    volumes:
      - .:/workspace
      - cargo-registry:/usr/local/cargo/registry
      - cargo-git:/usr/local/cargo/git
      - target:/workspace/target
      - ${NOEMA_HOME:-${HOME}/.noema}:/noema
    ports:
      - "127.0.0.1:3737:3737"
    depends_on:
      postgres:
        condition: service_healthy

  web:
    build:
      context: .
      target: dev
    working_dir: /workspace/crates/noema-core/web
    command: ["sh", "-lc", "bun install && bun run dev"]
    volumes:
      - .:/workspace
      - target:/workspace/target
      - bun-cache:/root/.bun/install/cache
      - web-node-modules:/workspace/crates/noema-core/web/node_modules
    depends_on:
      - noema-server

volumes:
  cargo-registry:
  cargo-git:
  target:
  bun-cache:
  web-node-modules:
```

- [ ] **Step 5: Add Docker ignore**

Create `.dockerignore`:

```text
.git
.env
.env.*
!.env.example
target
crates/noema-core/web/node_modules
crates/noema-core/web/dist
.noema
.noema-dev
```

- [ ] **Step 6: Add environment example**

Create `.env.example`:

```dotenv
NOEMA_DATABASE_URL=postgres://noema:noema@localhost:5432/noema
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test
NOEMA_HOME=${HOME}/.noema
NOEMA_OPENAI__API_KEY=
```

`NOEMA_HOME` is a host path for the complete local Noema home. Compose mounts
it into containers at `/noema`; Postgres stores its physical database cluster at
`/noema/db/postgres`, and the Noema server stores object-owned files under the
same mounted home. The default host path is `${HOME}/.noema`, so the physical
Postgres files live at `${HOME}/.noema/db/postgres` unless `NOEMA_HOME` is
overridden.

- [ ] **Step 7: Verify dependency resolution**

Run:

```bash
cargo check -p noema-core
```

Expected: it may still compile against SQLite code, but dependency resolution must succeed.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock crates/noema-core/Cargo.toml compose.yaml Dockerfile .dockerignore .env.example
git commit -m "chore: add postgres container development foundation"
```

---

### Task 2: Add Database Configuration And Postgres Pool

**Files:**
- Create: `crates/noema-core/src/database.rs`
- Modify: `crates/noema-core/src/config.rs`
- Modify: `crates/noema-core/src/home.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Test: `crates/noema-core/src/config.rs`

- [ ] **Step 1: Create database module**

Create `crates/noema-core/src/database.rs`:

```rust
//! Database configuration and Postgres connection helpers.

use sqlx::{PgPool, postgres::PgPoolOptions};
use thiserror::Error;

/// Environment variable that provides the canonical Postgres connection URL.
pub const NOEMA_DATABASE_URL_ENV: &str = "NOEMA_DATABASE_URL";

/// Resolved canonical database configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    /// Postgres connection URL.
    pub url: String,
}

impl DatabaseConfig {
    /// Build a database config from a URL string.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseConfigError::MissingUrl`] when the URL is empty.
    pub fn new(url: impl Into<String>) -> Result<Self, DatabaseConfigError> {
        let url = url.into();
        if url.trim().is_empty() {
            return Err(DatabaseConfigError::MissingUrl);
        }
        Ok(Self { url })
    }

    /// Connect to Postgres.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseConfigError::Connect`] when SQLx cannot connect.
    pub async fn connect(&self) -> Result<PgPool, DatabaseConfigError> {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(&self.url)
            .await
            .map_err(DatabaseConfigError::Connect)
    }
}

/// Database configuration and connection errors.
#[derive(Debug, Error)]
pub enum DatabaseConfigError {
    /// Database URL was not configured.
    #[error("{NOEMA_DATABASE_URL_ENV} is required for the Noema server")]
    MissingUrl,
    /// Postgres connection failed.
    #[error("failed to connect to Postgres: {0}")]
    Connect(sqlx::Error),
}
```

- [ ] **Step 2: Export database module**

Modify `crates/noema-core/src/lib.rs`:

```rust
/// Database configuration and Postgres connection helpers.
pub mod database;
```

and export:

```rust
pub use database::{DatabaseConfig, DatabaseConfigError, NOEMA_DATABASE_URL_ENV};
```

- [ ] **Step 3: Add config fields**

Modify `crates/noema-core/src/config.rs`:

```rust
use crate::{DatabaseConfig, NOEMA_DATABASE_URL_ENV};
```

Add `"database.url"` to `CONFIG_ENV_KEYS`.

Add to `ResolvedConfig`:

```rust
/// Canonical database configuration.
pub database: DatabaseConfig,
```

Add to `DaemonResolvedConfig`:

```rust
/// Canonical database configuration.
pub database: DatabaseConfig,
```

Add to the raw config struct:

```rust
database: RawDatabaseConfig,
```

Add:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
struct RawDatabaseConfig {
    #[serde(default)]
    url: Option<String>,
}

impl Default for RawDatabaseConfig {
    fn default() -> Self {
        Self {
            url: std::env::var(NOEMA_DATABASE_URL_ENV).ok(),
        }
    }
}
```

In raw config resolution, construct:

```rust
let database = DatabaseConfig::new(
    self.database
        .url
        .or_else(|| std::env::var(NOEMA_DATABASE_URL_ENV).ok())
        .ok_or(ConfigError::MissingDatabaseUrl)?,
)?;
```

Add to `ConfigError`:

```rust
#[error("{NOEMA_DATABASE_URL_ENV} is required for daemon storage")]
MissingDatabaseUrl,

#[error(transparent)]
Database(#[from] crate::DatabaseConfigError),
```

- [ ] **Step 4: Update default config template**

Modify `crates/noema-core/src/home.rs` default YAML by documenting the
environment variable rather than embedding a literal placeholder:

```yaml
# Set NOEMA_DATABASE_URL=postgres://noema:noema@localhost:5432/noema
```

Do not write secrets into the generated config.

- [ ] **Step 5: Update daemon server config**

Modify `DaemonServerConfig` in `crates/noema-core/src/daemon/protocol.rs`:

```rust
/// Canonical Postgres database URL.
pub database_url: String,
```

Update `DaemonServerConfig::new` to accept `database_url: String` instead of `database_path: PathBuf`.

- [ ] **Step 6: Update config tests**

Add a config test in `crates/noema-core/src/config.rs`:

```rust
#[test]
fn daemon_config_requires_database_url() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_path = dir.path().join("config.yaml");
    std::fs::write(
        &config_path,
        r#"
provider: codex
codex:
  command: codex
web:
  host: 127.0.0.1
  port: 3737
"#,
    )
    .expect("config");

    let error = Config::load_daemon(Some(config_path), CliOverrides::default()).unwrap_err();
    assert!(matches!(error, ConfigError::MissingDatabaseUrl));
}
```

Add a positive test with a database URL in YAML.

- [ ] **Step 7: Verify**

Run:

```bash
cargo test -p noema-core config:: --no-fail-fast
```

Expected: config tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/database.rs crates/noema-core/src/config.rs crates/noema-core/src/home.rs crates/noema-core/src/lib.rs crates/noema-core/src/daemon/protocol.rs
git commit -m "feat: add postgres database configuration"
```

---

### Task 3: Create Postgres Schema Bootstrap

**Files:**
- Create: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Modify: `crates/noema-core/src/memory_persistence.rs`
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
- Modify: `crates/noema-core/src/memory_persistence/error.rs`
- Test: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Add Postgres schema SQL**

Create `crates/noema-core/src/memory_persistence/postgres_schema.rs` with `POSTGRES_SCHEMA_SQL`. Port the current concrete object schema from `schema.rs` with these conversions:

```text
TEXT timestamp columns -> TIMESTAMPTZ NOT NULL DEFAULT now()
TEXT nullable timestamp columns -> TIMESTAMPTZ
metadata TEXT CHECK json_valid(metadata) -> metadata JSONB NOT NULL DEFAULT '{}'::jsonb
payload_json TEXT -> payload_json JSONB NOT NULL DEFAULT '{}'::jsonb
INTEGER booleans -> BOOLEAN
STRICT tables -> omit; Postgres is typed
SQLite FTS table -> `memory_items.search_vector TSVECTOR GENERATED ALWAYS AS (to_tsvector('simple', title || ' ' || content || ' ' || coalesce(retrieval_hints::text, ''))) STORED` plus `idx_memory_items_search_vector` GIN index
CURRENT_TIMESTAMP -> now()
```

Include at minimum:

```sql
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS humans (
  human_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  handle TEXT,
  is_active BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
```

The schema file must define every table in this exact list: `schema_migrations`,
`humans`, `agents`, `tools`, `conversations`, `conversation_turns`,
`conversation_items`, `memory_items`, `entities`, `relationships`,
`memory_subjects`, `memory_participants`, `memory_retrieval_purpose_rules`,
`memory_retrieval_object_links`, `object_access_grants`,
`object_provenance_edges`, `object_events`, `object_links`, `context_packets`,
`context_packet_memory_edges`, `context_packet_omissions`, and
`memory_use_records`. Convert each table from the current SQLite schema using
the conversion rules above, preserve every current enum `CHECK` constraint,
preserve relevant indexes, add `memory_items.search_vector` as the generated
stored `tsvector` column described above with the `idx_memory_items_search_vector`
GIN index, and add an index on `conversation_items(conversation_id, created_at)`.

- [ ] **Step 2: Add bootstrap function**

In `repository.rs`, replace the SQLite constructor with:

```rust
#[derive(Debug, Clone)]
pub struct PostgresMemoryRepository {
    pool: sqlx::PgPool,
}

impl PostgresMemoryRepository {
    /// Connect and bootstrap the Postgres-backed repository.
    pub async fn connect(database: &crate::DatabaseConfig) -> Result<Self, MemoryPersistenceError> {
        let pool = database.connect().await?;
        Self::from_pool(pool).await
    }

    /// Build a repository from an existing pool.
    pub async fn from_pool(pool: sqlx::PgPool) -> Result<Self, MemoryPersistenceError> {
        sqlx::query(super::postgres_schema::POSTGRES_SCHEMA_SQL)
            .execute(&pool)
            .await
            .map_err(MemoryPersistenceError::Database)?;
        sqlx::query(
            "INSERT INTO schema_migrations (version, name) VALUES ($1, $2)
             ON CONFLICT (version) DO NOTHING",
        )
        .bind(0_i32)
        .bind("postgres_bootstrap_v0")
        .execute(&pool)
        .await
        .map_err(MemoryPersistenceError::Database)?;
        Ok(Self { pool })
    }

    #[must_use]
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}
```

- [ ] **Step 3: Update module exports**

Modify `memory_persistence.rs`:

```rust
mod postgres_schema;
pub use repository::PostgresMemoryRepository;
```

Keep `pub use repository::SqliteMemoryRepository;` only until call sites are ported. Remove it in the cleanup task.

- [ ] **Step 4: Update error type**

Modify `error.rs`:

```rust
/// Errors produced by Postgres-backed memory persistence.
#[derive(Debug, Error)]
pub enum MemoryPersistenceError {
    #[error(transparent)]
    DatabaseConfig(#[from] crate::DatabaseConfigError),

    #[error("Postgres memory persistence failed: {0}")]
    Database(#[from] sqlx::Error),

    #[error("failed to serialize JSON metadata: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid {kind} value in Postgres: {value}")]
    InvalidEnum {
        kind: &'static str,
        value: String,
    },

    #[error("invalid object type: {value}")]
    InvalidObjectType {
        value: String,
    },

    #[error("object id cannot be empty for type {object_type}")]
    EmptyObjectId {
        object_type: String,
    },

    #[error("object reference not found: {object_type}:{object_id}")]
    ObjectRefNotFound {
        object_type: String,
        object_id: String,
    },

    #[error("turn {turn_id} does not belong to conversation {conversation_id}")]
    TurnConversationMismatch {
        turn_id: String,
        conversation_id: String,
    },

    #[error("memory not found: {memory_id}")]
    MemoryNotFound {
        memory_id: crate::memory::MemoryId,
    },

    #[error("relationship not found: {relationship_id}")]
    RelationshipNotFound {
        relationship_id: String,
    },

    #[error("active or confirmed relationship {relationship_id} requires supporting memory")]
    RelationshipRequiresSupportingMemory {
        relationship_id: String,
    },

    #[error("relationship {relationship_id} requires provenance on supporting memory {memory_id}")]
    RelationshipSupportingMemoryMissingProvenance {
        relationship_id: String,
        memory_id: crate::memory::MemoryId,
    },

    #[error(transparent)]
    MemoryStore(#[from] crate::memory::MemoryStoreError),
}
```

Keep `Sqlite(#[from] rusqlite::Error)` only while SQLite code remains.

- [ ] **Step 5: Add Postgres test helper**

Create `crates/noema-core/src/memory_persistence/postgres_tests.rs`:

```rust
use super::PostgresMemoryRepository;

async fn test_repo() -> Option<PostgresMemoryRepository> {
    let url = std::env::var("NOEMA_TEST_DATABASE_URL").ok()?;
    let pool = sqlx::PgPool::connect(&url).await.expect("connect test db");
    sqlx::query("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(&pool)
        .await
        .expect("reset schema");
    Some(PostgresMemoryRepository::from_pool(pool).await.expect("repo"))
}

#[tokio::test]
async fn bootstrap_creates_core_tables() {
    let Some(repo) = test_repo().await else {
        eprintln!("skipping postgres test; NOEMA_TEST_DATABASE_URL is not set");
        return;
    };
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'conversation_items'",
    )
    .fetch_one(repo.pool())
    .await
    .expect("table count");
    assert_eq!(count, 1);
}
```

- [ ] **Step 6: Wire test module**

In `memory_persistence.rs`:

```rust
#[cfg(test)]
mod postgres_tests;
```

- [ ] **Step 7: Verify without and with Postgres**

Run without database:

```bash
cargo test -p noema-core memory_persistence::postgres_tests::bootstrap_creates_core_tables -- --nocapture
```

Expected: PASS with a skip message if `NOEMA_TEST_DATABASE_URL` is unset.

Run with Compose Postgres:

```bash
docker compose up -d postgres
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::bootstrap_creates_core_tables -- --nocapture
```

Expected: PASS and creates the Postgres schema.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/memory_persistence.rs crates/noema-core/src/memory_persistence/error.rs crates/noema-core/src/memory_persistence/repository.rs crates/noema-core/src/memory_persistence/postgres_schema.rs crates/noema-core/src/memory_persistence/postgres_tests.rs
git commit -m "feat: bootstrap postgres memory schema"
```

---

### Task 4: Port Object References And Conversation Repositories

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/objects.rs`
- Modify: `crates/noema-core/src/memory_persistence/conversations.rs`
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_helpers.rs`
- Test: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Create Postgres helpers**

Create `postgres_helpers.rs`:

```rust
use serde_json::Value;

use super::MemoryPersistenceError;

pub(super) async fn allocate_id(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    prefix: &str,
) -> Result<String, MemoryPersistenceError> {
    let hex: String = sqlx::query_scalar("SELECT encode(gen_random_bytes(16), 'hex')")
        .fetch_one(executor)
        .await
        .map_err(MemoryPersistenceError::Database)?;
    Ok(format!("{prefix}_{hex}"))
}

pub(super) fn json_value(value: Value) -> Value {
    value
}
```

Add `CREATE EXTENSION IF NOT EXISTS pgcrypto;` to `POSTGRES_SCHEMA_SQL` before ID allocation tests run.

- [ ] **Step 2: Port object reference validation**

Replace `validate_object_ref_for_conn` with an async Postgres version:

```rust
pub(super) async fn validate_object_ref_for_pool(
    pool: &sqlx::PgPool,
    object_ref: &ObjectRef,
) -> Result<(), MemoryPersistenceError> {
    let sql = format!(
        "SELECT 1 FROM {} WHERE {} = $1 LIMIT 1",
        object_ref.object_type.table_name(),
        object_ref.object_type.id_column()
    );
    let exists = sqlx::query_scalar::<_, i32>(&sql)
        .bind(object_ref.object_id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(MemoryPersistenceError::Database)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(MemoryPersistenceError::ObjectRefNotFound {
            object_type: object_ref.object_type.as_str().to_string(),
            object_id: object_ref.object_id.clone(),
        })
    }
}
```

- [ ] **Step 3: Add default actor method**

In `repository.rs`:

```rust
pub async fn ensure_default_actors(&self) -> Result<(), MemoryPersistenceError> {
    let mut tx = self.pool.begin().await?;
    sqlx::query(
        "INSERT INTO humans (human_id, display_name, handle)
         VALUES ($1, $2, $3)
         ON CONFLICT (human_id) DO UPDATE SET updated_at = now()",
    )
    .bind("human:local")
    .bind("Local human")
    .bind("local")
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO agents (agent_id, display_name, handle)
         VALUES ($1, $2, $3)
         ON CONFLICT (agent_id) DO UPDATE SET updated_at = now()",
    )
    .bind("agent:primary")
    .bind("Noema")
    .bind("primary")
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
```

- [ ] **Step 4: Port conversation methods**

Move these methods onto `impl PostgresMemoryRepository` and make them `async`:

```rust
pub async fn create_conversation(&self, input: NewConversation) -> Result<ConversationRecord, MemoryPersistenceError>;
pub async fn create_conversation_turn(&self, input: NewConversationTurn) -> Result<ConversationTurnRecord, MemoryPersistenceError>;
pub async fn update_conversation_agent_status(&self, conversation_id: &str, status: AgentStatus) -> Result<(), MemoryPersistenceError>;
pub async fn append_conversation_item(&self, input: NewConversationItem) -> Result<ConversationItemRecord, MemoryPersistenceError>;
pub async fn complete_conversation_turn(&self, turn_id: &str) -> Result<(), MemoryPersistenceError>;
pub async fn fail_conversation_turn(&self, turn_id: &str) -> Result<(), MemoryPersistenceError>;
pub async fn list_conversation_items(&self, conversation_id: &str, mode: ReplayMode) -> Result<Vec<ConversationItemRecord>, MemoryPersistenceError>;
```

Use `$1`, `$2`, etc. placeholders and `RETURNING` to map rows in one round trip. Preserve current enum strings and record structs.

- [ ] **Step 5: Add conversation tests**

Add to `postgres_tests.rs`:

```rust
#[tokio::test]
async fn conversation_items_replay_in_created_order() {
    let Some(repo) = test_repo().await else {
        eprintln!("skipping postgres test; NOEMA_TEST_DATABASE_URL is not set");
        return;
    };
    repo.ensure_default_actors().await.expect("actors");
    let conversation = repo
        .create_conversation(super::NewConversation::local_chat(
            Some("test-model".to_string()),
            Some("/tmp/noema".to_string()),
        ))
        .await
        .expect("conversation");
    let turn = repo
        .create_conversation_turn(super::NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");
    repo.append_conversation_item(super::NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: Some(turn.turn_id.clone()),
        parent_item_id: None,
        kind: super::ConversationItemKind::UserText,
        status: super::ConversationItemStatus::Completed,
        author: super::ObjectRef::human("human:local"),
        content_text: Some("hello".to_string()),
        payload_json: serde_json::json!({}),
        metadata: serde_json::json!({}),
    })
    .await
    .expect("user item");
    let items = repo
        .list_conversation_items(&conversation.conversation_id, super::ReplayMode::Visible)
        .await
        .expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].content_text.as_deref(), Some("hello"));
}
```

- [ ] **Step 6: Verify**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::conversation_items_replay_in_created_order -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/memory_persistence/objects.rs crates/noema-core/src/memory_persistence/conversations.rs crates/noema-core/src/memory_persistence/repository.rs crates/noema-core/src/memory_persistence/postgres_helpers.rs crates/noema-core/src/memory_persistence/postgres_schema.rs crates/noema-core/src/memory_persistence/postgres_tests.rs
git commit -m "feat: port conversation persistence to postgres"
```

---

### Task 5: Port Memory, Provenance, Retrieval, And Context Packets

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/provenance.rs`
- Modify: `crates/noema-core/src/memory_persistence/context_packets.rs`
- Modify: `crates/noema-core/src/memory_persistence/queries.rs`
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
- Modify: `crates/noema-core/src/sqlite_memory_retrieval.rs`
- Test: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Port memory candidate insertion**

Move `append_memory_candidate` to async SQLx. Preserve:

```rust
pub async fn append_memory_candidate(
    &self,
    candidate: NewMemoryCandidate,
) -> Result<MemorySummary, MemoryPersistenceError>
```

Use one transaction. Validate owner, creator, subjects, participants, and source references before insert. Insert `memory_items`, `memory_subjects`, `memory_participants`, purpose rules, object links, and provenance edges in the same transaction.

- [ ] **Step 2: Port provenance edge insert**

Implement:

```rust
pub async fn add_object_provenance_edge(
    &self,
    edge: NewObjectProvenanceEdge,
) -> Result<String, MemoryPersistenceError>
```

Use:

```sql
INSERT INTO object_provenance_edges (
  provenance_edge_id,
  source_object_type,
  source_object_id,
  target_object_type,
  target_object_id,
  relation,
  confidence,
  metadata
)
VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
RETURNING provenance_edge_id
```

- [ ] **Step 3: Port soft-delete propagation**

Implement:

```rust
pub async fn soft_delete_conversation_item(
    &self,
    deletion: DeleteConversationItem,
) -> Result<(), MemoryPersistenceError>
```

The transaction must:

1. Validate item and deleting actor.
2. Set `conversation_items.deleted_at`, deleting actor, `redacted_at`, and `redaction_reason`.
3. Mark provenance edges from the item as deleted.
4. Find memories whose only non-deleted provenance edge was the deleted item.
5. Set those memory rows to `deleted` and redact their content.

- [ ] **Step 4: Port memory list/show queries**

Update `queries.rs` to Postgres SQL using `LEFT JOIN LATERAL` or grouped joins for source conversation item references. Keep `list_recent_memories` and `get_memory` behavior unchanged from the CLI perspective.

- [ ] **Step 5: Port retrieval**

Replace `sqlite_memory_retrieval` with a Postgres retrieval module or fold it into repository methods. Use Postgres FTS:

```sql
to_tsvector('simple', title || ' ' || content || ' ' || coalesce(retrieval_hints::text, ''))
@@ plainto_tsquery('simple', $1)
```

Keep policy filtering in Rust so behavior stays close to the current implementation.

- [ ] **Step 6: Port context packets**

Make `record_context_packet` async SQLx. Insert `context_packets`, `context_packet_memory_edges`, omissions, and memory use records in one transaction.

- [ ] **Step 7: Add Postgres provenance tests**

Add this test to `postgres_tests.rs` after Task 4's conversation helper test:

```rust
#[tokio::test]
async fn deleting_source_item_deletes_sole_provenance_memory() {
    let Some(repo) = test_repo().await else {
        eprintln!("skipping postgres test; NOEMA_TEST_DATABASE_URL is not set");
        return;
    };
    repo.ensure_default_actors().await.expect("actors");
    let conversation = repo
        .create_conversation(super::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let source = repo
        .append_conversation_item(super::NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: super::ConversationItemKind::UserText,
            status: super::ConversationItemStatus::Completed,
            author: super::ObjectRef::human("human:local"),
            content_text: Some("remember that I prefer early trains".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source");
    let memory = repo
        .append_memory_candidate(super::NewMemoryCandidate::confirmed_note(
            super::ObjectRef::human("human:local"),
            super::ObjectRef::agent("agent:primary"),
            "Travel preference",
            "The user prefers early trains.",
            super::ObjectRef::conversation_item(source.item_id.clone()),
        ))
        .await
        .expect("memory");
    repo.soft_delete_conversation_item(super::DeleteConversationItem {
        item: super::ObjectRef::conversation_item(source.item_id),
        deleted_by: super::ObjectRef::human("human:local"),
        reason: "user deleted source message".to_string(),
    })
    .await
    .expect("delete");
    let deleted = repo
        .get_memory(&memory.id)
        .await
        .expect("memory lookup")
        .expect("memory exists");
    assert_eq!(deleted.status, crate::memory::MemoryStatus::Deleted);
    assert!(deleted.content.contains("redacted"));
}
```

- [ ] **Step 8: Verify**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests -- --nocapture
```

Expected: all Postgres repository tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/noema-core/src/memory_persistence/provenance.rs crates/noema-core/src/memory_persistence/context_packets.rs crates/noema-core/src/memory_persistence/queries.rs crates/noema-core/src/memory_persistence/repository.rs crates/noema-core/src/sqlite_memory_retrieval.rs crates/noema-core/src/memory_persistence/postgres_tests.rs
git commit -m "feat: port memory provenance to postgres"
```

---

### Task 6: Update Daemon, Web Replay, And CLI Inspection

**Status:** Complete as of the Task 6 implementation slice. Daemon runtime,
memory extraction, web replay, CLI memory inspection, and CLI context graph
inspection now use `PostgresMemoryRepository`. Daemon DB-backed tests skip
cleanly when `NOEMA_TEST_DATABASE_URL` is unset, and a Postgres context graph
regression covers relationship-only omissions resolving back to their backing
memory rows.

**Files:**
- Modify: `crates/noema-core/src/daemon/server.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Modify: `crates/noema-cli/src/main.rs`
- Modify: `crates/noema-cli/src/inspection.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Pass database URL to daemon**

In `crates/noema-cli/src/main.rs`, update `run_start`:

```rust
run_daemon(DaemonServerConfig::new(
    socket_path,
    daemon_config.codex,
    daemon_config.database.url,
    daemon_config.web,
))
.await?;
```

- [ ] **Step 2: Connect repositories in server/runtime**

Change `CodexRuntimeHandle::spawn` and `MemoryExtractionWorkerHandle::spawn` to accept `DatabaseConfig` or `String` database URL. Inside async actor startup, connect:

```rust
let database = DatabaseConfig::new(database_url)?;
let memory_repository = PostgresMemoryRepository::connect(&database).await?;
```

Make `CodexRuntimeActor::new` async. Update `CodexRuntimeHandle::spawn` to
create the actor before spawning the command loop:

```rust
let actor = CodexRuntimeActor::new(config, database_url).await?;
tokio::spawn(actor.run(receiver));
```

If the compiler reports that startup must happen inside the spawned task, use a
oneshot channel named `startup_tx` / `startup_rx` so `spawn` returns database
connection errors before the handle is exposed.

- [ ] **Step 3: Await repository calls**

In `runtime.rs`, update every repository call:

```rust
self.memory_repository.ensure_default_actors().await?;
self.memory_repository.create_conversation(new_conversation).await?;
self.memory_repository.update_conversation_agent_status(&conversation_id, AgentStatus::Thinking).await?;
self.memory_repository.append_conversation_item(item).await?;
self.memory_repository.complete_conversation_turn(&turn_id).await?;
```

Keep stream event order unchanged.

- [ ] **Step 4: Update web replay**

In `web/mod.rs`, replace `database_path: PathBuf` with `database_url: String`. In replay handling:

```rust
let database = DatabaseConfig::new(state.database_url.clone())?;
let repo = PostgresMemoryRepository::connect(&database).await?;
let items = repo
    .list_conversation_items(&conversation_id, ReplayMode::Visible)
    .await?;
```

- [ ] **Step 5: Update CLI inspection**

In `inspection.rs`, open the repository from resolved daemon config or `NOEMA_DATABASE_URL`:

```rust
let config = Config::load_daemon(None, CliOverrides::default())?;
let repo = PostgresMemoryRepository::connect(&config.database).await?;
```

If `run_memory` and `run_context` are currently synchronous, make those command paths async and update call sites in `main.rs`.

- [ ] **Step 6: Update daemon tests**

Replace raw `rusqlite::Connection` assertions in `daemon/tests.rs` with Postgres repository or SQLx assertions. Use `NOEMA_TEST_DATABASE_URL`; skip with a message when unset.

- [ ] **Step 7: Verify**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core daemon::tests -- --nocapture
cargo check --workspace
```

Expected: daemon tests pass when Postgres is available; workspace checks compile.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/daemon crates/noema-cli/src/main.rs crates/noema-cli/src/inspection.rs
git commit -m "feat: use postgres persistence in daemon"
```

---

### Task 7: Remove SQLite Code Paths And Rename Persistence Surface

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/memory_persistence.rs`
- Modify/Delete: `crates/noema-core/src/memory_persistence/schema.rs`
- Modify/Delete: SQLite-only tests under `crates/noema-core/src/memory_persistence/tests/`
- Modify/Delete: `crates/noema-core/src/sqlite_memory_retrieval.rs`

- [ ] **Step 1: Remove `SqliteMemoryRepository` export**

In `memory_persistence.rs` and `lib.rs`, remove `SqliteMemoryRepository` and export:

```rust
pub use repository::PostgresMemoryRepository;
```

- [ ] **Step 2: Remove rusqlite imports**

Run:

```bash
rg "rusqlite|SqliteMemoryRepository|sqlite_memory_retrieval|SQLite|Sqlite" crates/noema-core crates/noema-cli Cargo.toml
```

Remove or rename every remaining runtime use. Test names may mention historical behavior only if they are testing docs strings, but prefer Postgres names.

- [ ] **Step 3: Remove rusqlite dependency**

Remove from root `Cargo.toml`:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```

Remove from `crates/noema-core/Cargo.toml` if present.

- [ ] **Step 4: Replace SQLite schema tests**

Delete SQLite-specific schema contract tests after Postgres equivalents exist. Add equivalent Postgres tests for:

- JSONB object payload validity.
- enum check constraints.
- object reference pair constraints.
- conversation turn/item mismatch validation.
- soft-delete deleted-by pair constraints.

- [ ] **Step 5: Verify cleanup**

Run:

```bash
rg "<stale sqlite/runtime persistence regex>" crates docs Cargo.toml
cargo check --workspace
```

Expected: no runtime SQLite references remain. Remaining docs references, if any, must be historical and intentional.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/noema-core crates/noema-cli
git commit -m "refactor: remove sqlite persistence paths"
```

---

### Task 8: Update Documentation And Product Language

**Files:**
- Create: `docs/postgres.md`
- Modify/Delete: `docs/sqlite.md`
- Modify: `docs/project.md`
- Modify: `docs/context/current.md`
- Modify: `docs/memory.md`
- Modify: `docs/harness.md`
- Modify: `docs/harness/*.md`
- Modify: `docs/frontend/*.md`
- Modify: `README.md`

- [ ] **Step 1: Add Postgres schema doc**

Create `docs/postgres.md`:

```markdown
# Canonical Postgres Schema

Postgres is Noema's canonical structured store for the always-on personal
server architecture.

Concrete object rows are canonical structured state. Actor/principal,
governable scope, provenance source, and transcript item are interfaces
implemented by concrete objects rather than universal parent tables.

The schema is bootstrapped by
`crates/noema-core/src/memory_persistence/postgres_schema.rs` during the
pre-stable phase. Durable user-data migrations will be introduced only when
Noema adopts a stable compatibility policy.
```

Then summarize the implemented tables and link to the schema file.

- [ ] **Step 2: Replace storage architecture in `docs/project.md`**

Update the canonical store diagram to:

```text
~/.noema/
  config.yaml
  humans/
  agents/
  conversations/
  workspaces/
  system/

Postgres
  canonical structured state
```

Replace SQLite source-of-truth rows with Postgres.

- [ ] **Step 3: Update current context**

In `docs/context/current.md`, set:

```markdown
Noema is an always-on, self-hosted personal agent operating system. The current
build path is chat-led and object-backed: clients connect to the Noema server,
the server records durable state in Postgres, and inspection/control surfaces
appear when they are useful.
```

Replace product-version bullets with current-slice bullets.

- [ ] **Step 4: Replace stale product-version and storage language**

Run:

```bash
rg -n "<stale product-version/storage regex>" docs README.md
```

For each product/architecture mention:

- Product-version labels -> `current slice`, `initial slice`, or `future slice`.
- Old compatibility labels -> `pre-stable`.
- `SQLite canonical` -> `Postgres canonical`.

Do not change OpenAI API URLs containing `/v1`.

- [ ] **Step 5: Verify docs**

Run:

```bash
rg -n "<stale product-version/storage regex>" docs README.md
```

Expected: no stale product-version architecture labels or legacy-store references remain. OpenAI `/v1` URLs are allowed.

- [ ] **Step 6: Commit**

```bash
git add docs README.md
git commit -m "docs: align project around postgres server architecture"
```

---

### Task 9: Container Smoke Test And Developer Workflow Polish

**Files:**
- Modify: `README.md`
- Modify: `compose.yaml`
- Modify: `crates/noema-cli/src/dev.rs` if this file remains part of the intended dev workflow.
- Test: container startup.

- [ ] **Step 1: Document local development**

Add to `README.md`:

```markdown
## Development

Copy environment defaults:

```bash
cp .env.example .env
```

Start the local development stack:

```bash
docker compose up
```

The web chat is available at <http://localhost:3737/>.
Postgres is available at `postgres://noema:noema@localhost:5432/noema`.
```
```

- [ ] **Step 2: Add database setup note**

Document test database creation:

```bash
docker compose exec postgres createdb -U noema noema_test
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests -- --nocapture
```

- [ ] **Step 3: Run Compose smoke test**

Run:

```bash
docker compose up -d postgres
docker compose ps
```

Expected: `postgres` is healthy.

Run:

```bash
NOEMA_DATABASE_URL=postgres://noema:noema@localhost:5432/noema cargo run -p noema-cli -- start
```

Expected: daemon starts and web chat binds on `127.0.0.1:3737`.

- [ ] **Step 4: Verify web status**

Run:

```bash
curl -s http://127.0.0.1:3737/api/status
```

Expected: JSON status response with the configured database/server state.

- [ ] **Step 5: Commit**

```bash
git add README.md compose.yaml crates/noema-cli/src/dev.rs
git commit -m "docs: document containerized development workflow"
```

---

### Task 10: Final Validation

**Files:**
- All changed files.

- [ ] **Step 1: Check status**

Run:

```bash
git status --short --branch
```

Expected: only unrelated pre-existing dirty files remain, or no dirty files remain if intentionally included in prior commits.

- [ ] **Step 2: Whitespace check**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 3: Rust format**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 4: Rust check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Unit and integration tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS, except daemon socket tests may need sandbox escalation.

Run with Postgres:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test --workspace --no-fail-fast
```

Expected: PASS with Postgres-backed repository tests exercising real database behavior.

- [ ] **Step 7: Frontend validation**

Run in `crates/noema-core/web`:

```bash
bun run gen:types
bun run lint
bun run build
```

Expected: all pass and embedded web assets are current.

- [ ] **Step 8: Container validation**

Run:

```bash
docker compose up --build
```

Expected: Postgres becomes healthy, Noema server starts, and web chat is reachable at `http://localhost:3737/`.

- [ ] **Step 9: Final commit for generated or formatting changes**

If validation caused generated asset or formatting changes:

```bash
git add crates/noema-core/web/src crates/noema-core/src/frontend_protocol.rs crates/noema-core/src/daemon/web/assets Cargo.lock
git commit -m "chore: finalize postgres container migration"
```

---

## Self-Review Notes

- Spec coverage: Postgres canonical store, always-on server posture, SQLx repositories, no ORM, no SQLite compatibility, Docker Compose development, config, documentation language, data flow, tests, and direct migration are all covered.
- Known execution risk: SQLx async migration touches many synchronous call sites. Task 6 explicitly allows actor startup to become async and requires call-site updates rather than blocking wrappers.
- Known test risk: Postgres tests need a real database. The plan includes Compose setup and `NOEMA_TEST_DATABASE_URL`; repository tests skip with a message only when the environment is absent.
- Known docs risk: `README.md` is currently dirty before this plan. Workers must inspect it before editing and preserve unrelated user changes.
