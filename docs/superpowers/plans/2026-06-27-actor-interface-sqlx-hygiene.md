# Actor Interface And SQLx Hygiene Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce `actors` as the first explicit interface table and make Noema's SQLx persistence code safer with typed IDs and named row structs.

**Architecture:** Keep Postgres schema and explicit SQL as the source of truth. Add typed Rust ID wrappers first, then add an `actors` table whose `actor_id` is used by author/creator/requester/deleter/audit roles. Keep broader ownership, provenance source/target, and generic object links as `ObjectRef` polymorphic refs in this slice. Use SQLx derives for transparent ID wrappers and `FromRow` mapping, but do not introduce an ORM or SQLx offline query macros yet.

**Tech Stack:** Rust, SQLx 0.8, Postgres, Tokio tests, existing Noema memory persistence repository.

---

## Scope

This plan combines the prior actor-interface and SQLx typed-query plans into one execution path.

This plan covers:

- Enabling SQLx derive support with the `macros` feature.
- Adding typed ID wrappers such as `ActorId`, `ObjectId`, `MemoryItemId`, and `ConversationId`.
- Adding `actors` with discriminator-backed concrete profile FKs.
- Migrating true actor roles from polymorphic object pairs to `actor_id` FKs.
- Keeping non-actor object refs polymorphic.
- Updating retrieval, fingerprint, context graph, CLI inspection, daemon call sites, and docs.
- Adding named `FromRow` structs for tuple-heavy query shapes touched during the migration.

This plan does not:

- Add an ORM.
- Add SQLx offline metadata or `query!`/`query_as!` macros.
- Convert bootstrap schema to migrations.
- Migrate `owner_object_type/id`, `target_object_type/id`, `source_object_type/id`, generic object links, provenance source/target, context object refs, used-for refs, or entity linked-object refs.
- Implement `governable_scopes` or a universal `objects` table.

## File Structure

- Modify: `Cargo.toml`
  - Add SQLx `macros` feature.
- Create: `crates/noema-core/src/memory_persistence/ids.rs`
  - Define typed persistence ID wrappers.
- Modify: `crates/noema-core/src/memory_persistence.rs`
  - Register and export `ids`, `ActorKind`, and `ActorRef`.
- Modify: `crates/noema-core/src/memory_persistence/objects.rs`
  - Keep `ObjectType`/`ObjectRef`.
  - Add `ActorKind`/`ActorRef`.
  - Store typed IDs inside refs.
  - Add actor/object validation helpers.
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
  - Add `actors`.
  - Add `humans.actor_id` and `agents.actor_id`.
  - Replace actor-role object pairs with direct actor FKs.
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
  - Upsert default actors before profiles.
  - Use typed row structs for memory summaries.
  - Update retrieval policy extractor writes.
- Modify: `crates/noema-core/src/memory_persistence/models.rs`
  - Use `ActorRef` for participants, creators, and actor-only fields.
- Modify: `crates/noema-core/src/memory_persistence/conversations.rs`
  - Store author/deleter actors as actor IDs.
  - Add named row structs for repeated read shapes.
- Modify: `crates/noema-core/src/memory_persistence/provenance.rs`
  - Store creator/deleter/audit actors as actor IDs.
  - Keep provenance targets/sources as `ObjectRef`.
- Modify: `crates/noema-core/src/memory_persistence/context_packets.rs`
  - Store requester/agent actors as actor IDs.
- Modify: `crates/noema-core/src/postgres_memory_retrieval.rs`
  - Read actor ID fields and add row structs for retrieval queries.
- Modify: `crates/noema-core/src/postgres_retrieval_policy_fingerprint.rs`
  - Update fingerprint basis for actor IDs and row structs.
- Modify: `crates/noema-core/src/memory_persistence/postgres_context_graph.rs`
  - Read actor IDs and add row structs for graph projections.
- Modify: `crates/noema-core/src/context_graph.rs`
  - Replace actor object-type/id output fields with actor IDs.
- Modify: `crates/noema-cli/src/inspection/context_graph_text.rs`
  - Render actor IDs directly.
- Modify: `crates/noema-cli/src/inspection/context_graph_output.rs`
  - Render actor IDs directly.
- Modify: `crates/noema-cli/src/inspection_tests.rs`
  - Update expected inspection records.
- Modify: `crates/noema-core/src/daemon/runtime.rs`
  - Use `ActorRef` for authors, creators, and participants.
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`
  - Use `ActorRef` for memory actor roles.
- Modify: `crates/noema-core/src/memory_persistence/postgres_tests.rs`
  - Add typed ID tests, actor schema tests, and update persistence assertions.
- Modify: `docs/postgres.md`
  - Document actors and SQLx persistence conventions.
- Modify: `docs/memory.md`
  - Distinguish actors, owners, subjects, participants, and object refs.
- Modify: `docs/context/current.md`
  - Add durable context after implementation completes.

## Naming Rules

- `ActorId`: a typed ID for rows in `actors`.
- `ActorRef`: a typed reference to an actor row.
- `ObjectId`: a typed ID for concrete object rows.
- `ObjectRef`: a typed reference to a concrete object row and object type.
- `actor_id`: a direct FK to `actors(actor_id)`.
- `actor_kind`: actor discriminator. Initial values: `human`, `agent`, `system`.
- Actor columns must not be named `*_object_type/id`.
- Non-actor object refs must not be named `*_actor_id`.

## Task 1: Enable SQLx Derives And Add Typed ID Tests

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Enable SQLx derive macros**

Change the workspace SQLx dependency in `Cargo.toml` from:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls", "postgres", "json"] }
```

to:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls", "postgres", "json", "macros"] }
```

- [ ] **Step 2: Add failing typed ID tests**

Add imports near the top of `postgres_tests.rs`:

```rust
use super::{ActorId, ConversationId, MemoryItemId, ObjectId};
```

Add these tests near the schema bootstrap tests:

```rust
#[test]
fn persistence_ids_reject_empty_values() {
    assert!(ActorId::try_from("").is_err());
    assert!(ObjectId::try_from("   ").is_err());
    assert!(MemoryItemId::try_from("").is_err());
    assert!(ConversationId::try_from("").is_err());
}

#[test]
fn persistence_ids_display_inner_value() {
    let actor_id = ActorId::try_from("agent:primary").expect("valid actor id");
    let object_id = ObjectId::try_from("conversation:abc").expect("valid object id");

    assert_eq!(actor_id.as_str(), "agent:primary");
    assert_eq!(object_id.to_string(), "conversation:abc");
}
```

- [ ] **Step 3: Run test to verify it fails before implementation**

Run:

```bash
cargo test -p noema-core persistence_ids_ -- --nocapture
```

Expected: compile fails because `ActorId`, `ObjectId`, `MemoryItemId`, and `ConversationId` do not exist yet.

## Task 2: Add Typed ID Wrappers

**Files:**
- Create: `crates/noema-core/src/memory_persistence/ids.rs`
- Modify: `crates/noema-core/src/memory_persistence.rs`

- [ ] **Step 1: Create `ids.rs`**

Create `crates/noema-core/src/memory_persistence/ids.rs`:

```rust
use std::{fmt, ops::Deref};

use super::MemoryPersistenceError;

macro_rules! persistence_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, sqlx::Type)]
        #[sqlx(transparent)]
        pub struct $name(String);

        impl $name {
            /// Create a typed persistence id.
            ///
            /// # Errors
            ///
            /// Returns [`MemoryPersistenceError::EmptyObjectId`] when the id is empty.
            pub fn new(value: impl Into<String>) -> Result<Self, MemoryPersistenceError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(MemoryPersistenceError::EmptyObjectId {
                        object_type: stringify!($name).to_string(),
                    });
                }
                Ok(Self(value))
            }

            /// Borrow the id as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume the typed id and return its inner string.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = MemoryPersistenceError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = MemoryPersistenceError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

persistence_id!(ActorId, "A typed id for rows in `actors`.");
persistence_id!(ObjectId, "A typed id for concrete object rows.");
persistence_id!(MemoryItemId, "A typed id for rows in `memory_items`.");
persistence_id!(ConversationId, "A typed id for rows in `conversations`.");
persistence_id!(ConversationItemId, "A typed id for rows in `conversation_items`.");
persistence_id!(ContextPacketId, "A typed id for rows in `context_packets`.");
```

- [ ] **Step 2: Register and export ID types**

In `crates/noema-core/src/memory_persistence.rs`, add:

```rust
mod ids;
```

near the other module declarations.

Add exports:

```rust
pub use ids::{
    ActorId, ContextPacketId, ConversationId, ConversationItemId, MemoryItemId, ObjectId,
};
```

- [ ] **Step 3: Run typed ID tests**

Run:

```bash
cargo test -p noema-core persistence_ids_ -- --nocapture
```

Expected: the typed ID tests pass.

## Task 3: Add Actor Schema Tests

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Update bootstrap table expectation**

In `bootstrap_creates_core_tables`, add `"actors"` as the first table in the sorted expected list:

```rust
assert_eq!(
    tables,
    [
        "actors",
        "agents",
        "context_packet_memory_edges",
        "context_packet_omissions",
        "context_packets",
        "conversation_items",
        "conversation_turns",
        "conversations",
        "entities",
        "humans",
        "memory_items",
        "memory_participants",
        "memory_retrieval_object_links",
        "memory_retrieval_purpose_rules",
        "memory_subjects",
        "memory_use_records",
        "object_access_grants",
        "object_events",
        "object_links",
        "object_provenance_edges",
        "provider_accounts",
        "relationships",
        "schema_migrations",
        "tools",
    ]
);
```

- [ ] **Step 2: Add failing profile-kind integrity test**

Add near existing schema tests:

```rust
#[tokio::test]
async fn actors_enforce_concrete_profile_kind() {
    let Some(repo) = test_repo().await else {
        return;
    };

    sqlx::query(
        r"
        INSERT INTO actors (actor_id, actor_kind, display_name)
        VALUES ('actor:test-human', 'human', 'Test Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("insert human actor");

    sqlx::query(
        r"
        INSERT INTO humans (human_id, actor_id, display_name)
        VALUES ('human:test', 'actor:test-human', 'Test Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("human profile may point at human actor");

    let invalid_agent_profile = sqlx::query(
        r"
        INSERT INTO agents (agent_id, actor_id, display_name)
        VALUES ('agent:wrong-kind', 'actor:test-human', 'Wrong Kind')
        ",
    )
    .execute(repo.pool())
    .await;

    assert_sqlstate(invalid_agent_profile, "23503");
}
```

- [ ] **Step 3: Add failing actor reuse test**

Add after the previous test:

```rust
#[tokio::test]
async fn actors_cannot_be_reused_by_two_human_profiles() {
    let Some(repo) = test_repo().await else {
        return;
    };

    sqlx::query(
        r"
        INSERT INTO actors (actor_id, actor_kind, display_name)
        VALUES ('actor:shared-human', 'human', 'Shared Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("insert actor");

    sqlx::query(
        r"
        INSERT INTO humans (human_id, actor_id, display_name)
        VALUES ('human:first', 'actor:shared-human', 'First Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("first profile may use actor");

    let duplicate_actor = sqlx::query(
        r"
        INSERT INTO humans (human_id, actor_id, display_name)
        VALUES ('human:second', 'actor:shared-human', 'Second Human')
        ",
    )
    .execute(repo.pool())
    .await;

    assert_sqlstate(duplicate_actor, "23505");
}
```

- [ ] **Step 4: Run test to verify it fails before schema implementation**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core actors_ -- --nocapture
```

Expected: fail because `actors` and concrete `actor_id` profile columns do not exist yet.

## Task 4: Add `actors` And Concrete Profile FKs

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`

- [ ] **Step 1: Add `actors` table before `humans`**

Add before `CREATE TABLE IF NOT EXISTS humans`:

```sql
CREATE TABLE IF NOT EXISTS actors (
  actor_id TEXT PRIMARY KEY,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('human','agent','system')),
  display_name TEXT NOT NULL,
  handle TEXT,
  is_active BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  UNIQUE (actor_id, actor_kind)
);
```

- [ ] **Step 2: Add concrete actor FK columns**

Change the start of `humans` to:

```sql
CREATE TABLE IF NOT EXISTS humans (
  human_id TEXT PRIMARY KEY,
  actor_id TEXT NOT NULL UNIQUE,
  actor_kind TEXT GENERATED ALWAYS AS ('human') STORED,
  display_name TEXT NOT NULL,
  handle TEXT,
  primary_conversation_id TEXT,
  is_active BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  FOREIGN KEY (actor_id, actor_kind)
    REFERENCES actors(actor_id, actor_kind)
    ON DELETE RESTRICT
);
```

Change the start of `agents` to:

```sql
CREATE TABLE IF NOT EXISTS agents (
  agent_id TEXT PRIMARY KEY,
  actor_id TEXT NOT NULL UNIQUE,
  actor_kind TEXT GENERATED ALWAYS AS ('agent') STORED,
  display_name TEXT NOT NULL,
  handle TEXT,
  model_default TEXT,
  is_active BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  FOREIGN KEY (actor_id, actor_kind)
    REFERENCES actors(actor_id, actor_kind)
    ON DELETE RESTRICT
);
```

- [ ] **Step 3: Add actor indexes**

Add near the existing handle indexes:

```sql
CREATE INDEX IF NOT EXISTS idx_actors_kind_active ON actors(actor_kind, is_active);
CREATE INDEX IF NOT EXISTS idx_actors_handle ON actors(handle);
```

- [ ] **Step 4: Upsert default actors before concrete profiles**

In `ensure_default_actors`, insert actor rows before inserting `humans` and `agents`:

```rust
sqlx::query(
    r"
    INSERT INTO actors (actor_id, actor_kind, display_name, handle)
    VALUES ($1, 'human', $2, $3)
    ON CONFLICT (actor_id) DO UPDATE SET
      display_name = EXCLUDED.display_name,
      handle = EXCLUDED.handle,
      updated_at = now()
    ",
)
.bind("human:local")
.bind("Local human")
.bind("local")
.execute(&mut *tx)
.await
.map_err(MemoryPersistenceError::Database)?;

sqlx::query(
    r"
    INSERT INTO actors (actor_id, actor_kind, display_name, handle)
    VALUES ($1, 'agent', $2, $3)
    ON CONFLICT (actor_id) DO UPDATE SET
      display_name = EXCLUDED.display_name,
      handle = EXCLUDED.handle,
      updated_at = now()
    ",
)
.bind("agent:primary")
.bind("Noema")
.bind("primary")
.execute(&mut *tx)
.await
.map_err(MemoryPersistenceError::Database)?;
```

- [ ] **Step 5: Set profile `actor_id` values**

Change the human insert to:

```rust
sqlx::query(
    r"
    INSERT INTO humans (human_id, actor_id, display_name, handle)
    VALUES ($1, $1, $2, $3)
    ON CONFLICT (human_id) DO UPDATE SET
      actor_id = EXCLUDED.actor_id,
      updated_at = now()
    ",
)
.bind("human:local")
.bind("Local human")
.bind("local")
.execute(&mut *tx)
.await
.map_err(MemoryPersistenceError::Database)?;
```

Change the agent insert to:

```rust
sqlx::query(
    r"
    INSERT INTO agents (agent_id, actor_id, display_name, handle)
    VALUES ($1, $1, $2, $3)
    ON CONFLICT (agent_id) DO UPDATE SET
      actor_id = EXCLUDED.actor_id,
      updated_at = now()
    ",
)
.bind("agent:primary")
.bind("Noema")
.bind("primary")
.execute(&mut *tx)
.await
.map_err(MemoryPersistenceError::Database)?;
```

- [ ] **Step 6: Run actor schema tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core actors_ -- --nocapture
```

Expected: actor schema tests pass.

## Task 5: Add Typed `ObjectRef`, `ActorKind`, And `ActorRef`

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/objects.rs`
- Modify: `crates/noema-core/src/memory_persistence.rs`

- [ ] **Step 1: Update imports in `objects.rs`**

Change the import to:

```rust
use super::{ActorId, MemoryPersistenceError, ObjectId};
```

- [ ] **Step 2: Store typed object IDs**

Change `ObjectRef` to:

```rust
pub struct ObjectRef {
    /// Concrete object type.
    pub object_type: ObjectType,
    /// Concrete object id.
    pub object_id: ObjectId,
}
```

Update `ObjectRef::new`:

```rust
pub fn new(
    object_type: ObjectType,
    object_id: impl Into<String>,
) -> Result<Self, MemoryPersistenceError> {
    Ok(Self {
        object_type,
        object_id: ObjectId::new(object_id)?,
    })
}
```

Update convenience constructors:

```rust
pub fn human(object_id: impl Into<String>) -> Self {
    Self {
        object_type: ObjectType::Human,
        object_id: ObjectId::new(object_id).expect("object id must not be empty"),
    }
}
```

Repeat the same pattern for `agent` and `conversation_item`.

- [ ] **Step 3: Add `ActorKind`**

Add below `ObjectRef`:

```rust
/// Actor kinds that may perform actions in Noema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    /// A human user or collaborator.
    Human,
    /// A Noema agent.
    Agent,
    /// System-owned automatic behavior.
    System,
}

impl ActorKind {
    /// Return the stable storage string for this actor kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::System => "system",
        }
    }

    /// Parse a storage string into an actor kind.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidObjectType`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "system" => Ok(Self::System),
            _ => Err(MemoryPersistenceError::InvalidObjectType {
                value: value.to_string(),
            }),
        }
    }
}
```

- [ ] **Step 4: Add `ActorRef`**

Add:

```rust
/// A typed reference to an actor row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorRef {
    /// Actor kind.
    pub actor_kind: ActorKind,
    /// Actor id.
    pub actor_id: ActorId,
}

impl ActorRef {
    /// Construct a typed actor reference.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::EmptyObjectId`] when the id is empty.
    pub fn new(
        actor_kind: ActorKind,
        actor_id: impl Into<String>,
    ) -> Result<Self, MemoryPersistenceError> {
        Ok(Self {
            actor_kind,
            actor_id: ActorId::new(actor_id)?,
        })
    }

    /// Reference a human actor.
    #[must_use]
    pub fn human(actor_id: impl Into<String>) -> Self {
        Self {
            actor_kind: ActorKind::Human,
            actor_id: ActorId::new(actor_id).expect("actor id must not be empty"),
        }
    }

    /// Reference an agent actor.
    #[must_use]
    pub fn agent(actor_id: impl Into<String>) -> Self {
        Self {
            actor_kind: ActorKind::Agent,
            actor_id: ActorId::new(actor_id).expect("actor id must not be empty"),
        }
    }

    /// Reference a system actor.
    #[must_use]
    pub fn system(actor_id: impl Into<String>) -> Self {
        Self {
            actor_kind: ActorKind::System,
            actor_id: ActorId::new(actor_id).expect("actor id must not be empty"),
        }
    }
}
```

- [ ] **Step 5: Add display implementations**

Update `ObjectRef` display to use `.as_str()`:

```rust
impl fmt::Display for ObjectRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}",
            self.object_type.as_str(),
            self.object_id.as_str()
        )
    }
}
```

Add:

```rust
impl fmt::Display for ActorRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.actor_id.as_str())
    }
}
```

- [ ] **Step 6: Update validation helpers**

Update object validation binds:

```rust
.bind(object_ref.object_id.as_str())
```

Add actor validation:

```rust
pub(super) async fn validate_actor_ref_for_pool(
    pool: &sqlx::PgPool,
    actor_ref: &ActorRef,
) -> Result<(), MemoryPersistenceError> {
    let exists = sqlx::query_scalar::<_, i32>(
        r"
        SELECT 1
        FROM actors
        WHERE actor_id = $1
          AND actor_kind = $2
        LIMIT 1
        ",
    )
    .bind(actor_ref.actor_id.as_str())
    .bind(actor_ref.actor_kind.as_str())
    .fetch_optional(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?
    .is_some();

    if exists {
        Ok(())
    } else {
        Err(MemoryPersistenceError::ObjectRefNotFound {
            object_type: actor_ref.actor_kind.as_str().to_string(),
            object_id: actor_ref.actor_id.to_string(),
        })
    }
}
```

- [ ] **Step 7: Export actor types**

In `memory_persistence.rs`, change:

```rust
pub use objects::{ObjectRef, ObjectType};
```

to:

```rust
pub use objects::{ActorKind, ActorRef, ObjectRef, ObjectType};
```

- [ ] **Step 8: Run compile check**

Run:

```bash
cargo check -p noema-core
```

Expected: compile errors may remain at call sites that directly use `object_id` as `String`. Fix those by using `.as_str()`, `.to_string()`, or `.clone().into_string()` according to ownership needs.

## Task 6: Migrate Conversation Actor Roles

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Modify: `crates/noema-core/src/memory_persistence/conversations.rs`
- Modify: `crates/noema-core/src/memory_persistence/provenance.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Replace conversation actor columns**

In `conversations`, replace:

```sql
  deleted_by_object_type TEXT,
  deleted_by_object_id TEXT,
```

with:

```sql
  deleted_by_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
```

Remove the deleted-by object-pair check.

In `conversation_items`, replace:

```sql
  author_object_type TEXT NOT NULL,
  author_object_id TEXT NOT NULL,
```

with:

```sql
  author_actor_id TEXT NOT NULL REFERENCES actors(actor_id) ON DELETE RESTRICT,
```

Replace deleted-by object pair columns with:

```sql
  deleted_by_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
```

Replace the author index with:

```sql
CREATE INDEX IF NOT EXISTS idx_conversation_items_author_actor
  ON conversation_items(author_actor_id, created_at DESC);
```

- [ ] **Step 2: Change `NewConversationItem` author type**

In `conversations.rs`, import `ActorRef` and `validate_actor_ref_for_pool`.

Change:

```rust
pub author: ObjectRef,
```

to:

```rust
pub author: ActorRef,
```

- [ ] **Step 3: Validate and write author actor IDs**

Before inserting a conversation item, validate:

```rust
validate_actor_ref_for_pool(self.pool(), &item.author).await?;
```

In the insert SQL, replace `author_object_type, author_object_id` with `author_actor_id`, and bind:

```rust
.bind(item.author.actor_id.as_str())
```

- [ ] **Step 4: Change deletion actor type**

In `provenance.rs`, change `DeleteConversationItem`:

```rust
pub deleted_by: ActorRef,
```

Validate with `validate_actor_ref_for_pool`, and update deletion SQL to set `deleted_by_actor_id`.

- [ ] **Step 5: Update runtime and tests**

Replace actor-role calls like:

```rust
author: ObjectRef::human("human:local"),
```

with:

```rust
author: ActorRef::human("human:local"),
```

Keep non-actor refs such as:

```rust
ObjectRef::new(ObjectType::Conversation, conversation_id)?
ObjectRef::conversation_item(user_item_id)
```

- [ ] **Step 6: Run conversation tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core conversation -- --nocapture
```

Expected: conversation persistence tests pass.

## Task 7: Migrate Memory Actor Roles

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Modify: `crates/noema-core/src/memory_persistence/models.rs`
- Modify: `crates/noema-core/src/memory_persistence/provenance.rs`
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Replace memory actor columns**

In `memory_items`, replace:

```sql
  retrieval_policy_extractor_object_type TEXT,
  retrieval_policy_extractor_object_id TEXT,
  created_by_object_type TEXT NOT NULL,
  created_by_object_id TEXT NOT NULL,
```

with:

```sql
  retrieval_policy_extractor_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
  created_by_actor_id TEXT NOT NULL REFERENCES actors(actor_id) ON DELETE RESTRICT,
```

Update the valid policy check to require `retrieval_policy_extractor_actor_id IS NOT NULL`.

In `memory_participants`, replace participant object pair columns with:

```sql
  participant_actor_id TEXT NOT NULL REFERENCES actors(actor_id) ON DELETE CASCADE,
```

Change the primary key to:

```sql
PRIMARY KEY (memory_id, participant_actor_id, role)
```

In `memory_retrieval_purpose_rules`, replace creator object pair columns with:

```sql
created_by_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
```

In `memory_retrieval_object_links`, replace resolver/authorized/creator object pairs with:

```sql
resolver_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
authorized_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
created_by_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
```

- [ ] **Step 2: Change memory model actor fields**

In `models.rs`, use `ActorRef` for:

```rust
pub participant: ActorRef,
pub created_by: ActorRef,
pub owner_actor: Option<ActorRef>,
```

Remove the string-to-object participant inference helper if it only exists to infer human/agent `ObjectRef`s.

- [ ] **Step 3: Validate actor refs when appending memory**

In `validate_postgres_memory_candidate_refs`, add:

```rust
validate_actor_ref_for_pool(pool, &candidate.created_by).await?;
for participant in &candidate.participants {
    validate_actor_ref_for_pool(pool, &participant.participant).await?;
}
```

Keep `validate_object_ref_for_pool` for memory owner, subject linked objects, and provenance sources.

- [ ] **Step 4: Write actor IDs**

In `append_memory_candidate`, replace creator object columns with `created_by_actor_id`, and bind:

```rust
.bind(candidate.created_by.actor_id.as_str())
```

Replace the memory participant insert with:

```sql
INSERT INTO memory_participants (
  memory_id,
  participant_actor_id,
  role,
  metadata
)
VALUES ($1, $2, $3, $4)
ON CONFLICT (memory_id, participant_actor_id, role)
DO NOTHING
```

Bind:

```rust
.bind(participant.participant.actor_id.as_str())
```

- [ ] **Step 5: Update retrieval policy extractor**

Change `refresh_retrieval_policy_fingerprint` to accept:

```rust
extractor: ActorRef,
```

Validate with `validate_actor_ref_for_pool`, set `retrieval_policy_extractor_actor_id`, and bind:

```rust
.bind(extractor.actor_id.as_str())
```

- [ ] **Step 6: Update daemon call sites**

In `daemon/memory_pipeline.rs` and `daemon/runtime.rs`, use:

```rust
ActorRef::agent("agent:primary")
ActorRef::human("human:local")
```

for participants, creators, and `owner_actor`. Keep ownership/source refs as `ObjectRef`.

- [ ] **Step 7: Run memory tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core memory_persistence::postgres_tests:: -- --nocapture
```

Expected: memory persistence tests pass after graph/retrieval readers are updated in later tasks.

## Task 8: Migrate Provenance, Events, Context Packets, And Use Records

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Modify: `crates/noema-core/src/memory_persistence/provenance.rs`
- Modify: `crates/noema-core/src/memory_persistence/context_packets.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Replace actor-role schema fields**

In `object_access_grants`, `object_provenance_edges`, and `object_links`, replace `created_by_object_type/id` with:

```sql
created_by_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL
```

In `object_events`, replace actor object pair columns with:

```sql
actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
```

In `context_packets`, replace requester object pair columns with:

```sql
requesting_actor_id TEXT NOT NULL REFERENCES actors(actor_id) ON DELETE RESTRICT,
```

In `memory_use_records`, replace agent object pair columns with:

```sql
agent_actor_id TEXT REFERENCES actors(actor_id) ON DELETE SET NULL,
```

- [ ] **Step 2: Update indexes**

Replace:

```sql
CREATE INDEX IF NOT EXISTS idx_object_events_actor_time ON object_events(actor_object_type, actor_object_id, created_at DESC);
```

with:

```sql
CREATE INDEX IF NOT EXISTS idx_object_events_actor_time ON object_events(actor_id, created_at DESC);
```

Add:

```sql
CREATE INDEX IF NOT EXISTS idx_context_packets_requesting_actor
  ON context_packets(requesting_actor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_use_records_agent_actor
  ON memory_use_records(agent_actor_id, created_at DESC);
```

- [ ] **Step 3: Change provenance edge creator**

In `NewObjectProvenanceEdge`, change:

```rust
pub created_by: ActorRef,
```

Keep `target` and `source` as `ObjectRef`.

- [ ] **Step 4: Write actor IDs for creators and events**

In provenance insert SQL, replace `created_by_object_type`, `created_by_object_id` with `created_by_actor_id`.

In object event inserts, replace actor pair columns with `actor_id`, and bind:

```rust
.bind(candidate.created_by.actor_id.as_str())
```

or:

```rust
.bind(edge.created_by.actor_id.as_str())
```

- [ ] **Step 5: Update context packet writes**

In `context_packets.rs`, delete `actor_object_parts`. Use the request principal id directly as the actor id:

```rust
let requesting_actor_id = request.requesting_principal_id.as_str();
```

When building `MemoryUseRecord`, write:

```rust
agent_actor_id: Some(requesting_actor_id.to_string()),
```

- [ ] **Step 6: Run context/provenance tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core context -- --nocapture
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core provenance -- --nocapture
```

Expected: context and provenance tests pass after graph/retrieval readers are updated.

## Task 9: Add Named `FromRow` Structs For Touched Queries

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
- Modify: `crates/noema-core/src/memory_persistence/conversations.rs`
- Modify: `crates/noema-core/src/postgres_memory_retrieval.rs`
- Modify: `crates/noema-core/src/postgres_retrieval_policy_fingerprint.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_context_graph.rs`

- [ ] **Step 1: Derive `FromRow` for memory summary rows**

In `repository.rs`, change `MemorySummaryRow` to:

```rust
#[derive(sqlx::FromRow)]
struct MemorySummaryRow {
    memory_id: String,
    status: String,
    memory_type: String,
    owner_object_type: String,
    owner_object_id: String,
    sensitivity: String,
    title: String,
    content: String,
    created_at: String,
    source_object_type: Option<String>,
    source_object_id: Option<String>,
    conversation_id: Option<String>,
}
```

Replace tuple decoding in `list_recent_memories` with:

```rust
let rows = sqlx::query_as::<_, MemorySummaryRow>(POSTGRES_RECENT_MEMORY_SQL)
    .bind(i64::from(limit))
    .fetch_all(&self.pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

rows.into_iter()
    .map(|row| postgres_row_to_memory_summary(row).map(redact_for_list))
    .collect()
```

Replace tuple decoding in `get_memory` with:

```rust
let row = sqlx::query_as::<_, MemorySummaryRow>(POSTGRES_MEMORY_SUMMARY_BY_ID_SQL)
    .bind(memory_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

row.map(postgres_row_to_memory_summary).transpose()
```

- [ ] **Step 2: Add conversation row structs**

In `conversations.rs`, add a row struct for long conversation item reads:

```rust
#[derive(sqlx::FromRow)]
struct ConversationItemRow {
    item_id: String,
    conversation_id: String,
    turn_id: Option<String>,
    parent_item_id: Option<String>,
    kind: String,
    status: String,
    author_actor_id: String,
    content_text: Option<String>,
    payload_json: Value,
    created_at: String,
    updated_at: String,
    metadata: Value,
}
```

Use it with:

```rust
let rows = sqlx::query_as::<_, ConversationItemRow>(sql)
    .bind(conversation_id)
    .fetch_all(&self.pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;
```

- [ ] **Step 3: Add retrieval row structs**

In `postgres_memory_retrieval.rs`, replace tuple-heavy query outputs with row structs such as:

```rust
#[derive(sqlx::FromRow)]
struct PurposeRuleRow {
    memory_id: String,
    purpose: String,
    effect: String,
}

#[derive(sqlx::FromRow)]
struct ParticipantRow {
    memory_id: String,
    participant_actor_id: String,
    actor_kind: String,
    role: String,
}

#[derive(sqlx::FromRow)]
struct ProvenanceRow {
    target_object_type: String,
    target_object_id: String,
    source_object_type: String,
    source_object_id: String,
    relation: String,
}
```

For participants, join `actors` to recover kind when needed:

```sql
SELECT participant.memory_id,
       participant.participant_actor_id,
       actor.actor_kind,
       participant.role
FROM memory_participants participant
JOIN actors actor ON actor.actor_id = participant.participant_actor_id
```

- [ ] **Step 4: Add fingerprint row structs**

In `postgres_retrieval_policy_fingerprint.rs`, add structs for basis rows:

```rust
#[derive(sqlx::FromRow)]
struct ProvenanceBasisRow {
    source_object_type: String,
    source_object_id: String,
    relation: String,
    evidence_excerpt: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ParticipantBasisRow {
    participant_actor_id: String,
    actor_kind: String,
    role: String,
}
```

Update JSON basis fields to use `created_by_actor_id`, `participant_actor_id`, and `actor_kind` for actor roles.

- [ ] **Step 5: Add graph projection row structs**

In `postgres_context_graph.rs`, replace manual `try_get` blocks for actor-touched projections with named rows. For events:

```rust
#[derive(sqlx::FromRow)]
struct GraphObjectEventRow {
    event_id: String,
    event_type: String,
    actor_id: Option<String>,
    target_object_type: Option<String>,
    target_object_id: Option<String>,
    reason: Option<String>,
    created_at: String,
    details: serde_json::Value,
}
```

Use:

```rust
let rows = sqlx::query_as::<_, GraphObjectEventRow>(sql)
    .fetch_all(self.pool())
    .await
    .map_err(MemoryPersistenceError::Database)?;
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core memory_persistence::postgres_tests:: -- --nocapture
cargo test -p noema-cli inspection
```

Expected: persistence and CLI inspection tests pass.

## Task 10: Update Context Graph And CLI Output Models

**Files:**
- Modify: `crates/noema-core/src/context_graph.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_context_graph.rs`
- Modify: `crates/noema-cli/src/inspection/context_graph_text.rs`
- Modify: `crates/noema-cli/src/inspection/context_graph_output.rs`
- Modify: `crates/noema-cli/src/inspection_tests.rs`

- [ ] **Step 1: Replace actor object pairs in graph structs**

In `context_graph.rs`, replace actor-role pairs with actor ID fields:

```rust
pub retrieval_policy_extractor_actor_id: Option<String>,
pub created_by_actor_id: Option<String>,
pub requesting_actor_id: String,
pub agent_actor_id: Option<String>,
pub actor_id: Option<String>,
```

Keep generic source/target/context/used-for object pairs unchanged.

- [ ] **Step 2: Update graph SQL and mapping**

In `postgres_context_graph.rs`, select actor fields:

```sql
memory.retrieval_policy_extractor_actor_id
rule.created_by_actor_id
grant.created_by_actor_id
packet.requesting_actor_id
use_record.agent_actor_id
event.actor_id
```

Remove reads of actor-role object pair columns.

- [ ] **Step 3: Update CLI text rendering**

Replace calls such as:

```rust
object_ref(&packet.requesting_object_type, &packet.requesting_object_id)
```

with:

```rust
packet.requesting_actor_id.as_str()
```

- [ ] **Step 4: Update CLI JSON/output rendering**

In `context_graph_output.rs`, serialize actor ID fields directly instead of actor object pairs.

- [ ] **Step 5: Update inspection tests**

Replace expected values like:

```rust
created_by_object_type: Some("agent".to_string()),
created_by_object_id: Some("agent:primary".to_string()),
```

with:

```rust
created_by_actor_id: Some("agent:primary".to_string()),
```

Apply the same pattern for requester, agent, extractor, and event actors.

- [ ] **Step 6: Run graph and CLI tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core postgres_context_graph -- --nocapture
cargo test -p noema-cli inspection
```

Expected: graph and CLI inspection tests pass.

## Task 11: Update Docs

**Files:**
- Modify: `docs/postgres.md`
- Modify: `docs/memory.md`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Document actor/interface model**

In `docs/postgres.md`, add:

```markdown
Noema distinguishes concrete objects from actor interfaces. Concrete object
tables remain the canonical source of domain truth. The `actors` table is a
narrow interface table for rows that can perform actions. Actor-role columns
use direct `actor_id` foreign keys; broader ownership, provenance source/target,
and generic object links continue to use typed object references until those
interfaces are explicitly modeled.
```

Update implemented tables:

```markdown
- `actors`: actor interface rows for humans, agents, and system actors.
- `humans`, `agents`: concrete actor profiles. Each profile owns one unique
  `actor_id` whose `actor_kind` must match the profile table.
```

- [ ] **Step 2: Document SQLx conventions**

In `docs/postgres.md`, add:

```markdown
## SQLx Persistence Conventions

Noema uses SQLx with explicit SQL rather than an ORM. Schema shape remains the
architecture source of truth.

Use typed ID wrappers such as `ActorId`, `ObjectId`, `MemoryItemId`, and
`ConversationId` at persistence API boundaries. Use `ObjectRef` for generic
concrete object references and `ActorRef` for action-capable identities.

Use named `FromRow` structs for query results with more than three selected
fields or rows reused across functions. Small local tuple queries are acceptable
when the tuple shape is obvious. Use dynamic SQL only for whitelisted table or
column names, such as `ObjectType::table_name()` and `ObjectType::id_column()`.

SQLx `query!` and `query_as!` macros are deferred until Noema adopts an offline
SQLx metadata workflow or a stable development database contract.
```

- [ ] **Step 3: Update memory docs**

In `docs/memory.md`, add:

```markdown
Actors and owners are intentionally different. An actor can create, confirm,
delete, request, or use memory. An owner is the governable object where memory
is managed. Actor roles use `actor_id`; owner, source, and target roles remain
object references unless a narrower interface exists.
```

- [ ] **Step 4: Update current context after implementation**

In `docs/context/current.md`, add under "Settled Decisions":

```markdown
- Actors are the first explicit interface table. Humans and agents are concrete
  actor profiles with unique `actor_id` foreign keys; actor roles such as
  author, creator, requester, deleter, and audit actor use direct actor foreign
  keys instead of polymorphic object refs.
- Persistence uses SQLx with explicit SQL plus typed ID wrappers and named
  `FromRow` structs. Noema is not adopting an ORM; SQL and Postgres constraints
  remain the source of schema truth.
```

- [ ] **Step 5: Run wording search**

Run:

```bash
rg -n "author_object_type|created_by_object_type|actor_object_type|requesting_object_type|agent_object_type|participant_object_type|retrieval_policy_extractor_object_type" crates/noema-core/src crates/noema-cli/src docs/postgres.md docs/memory.md docs/context/current.md
```

Expected: no active actor-role matches. Generic non-actor object references may remain if they are intentionally source/target/owner/context refs.

## Task 12: Full Validation

**Files:**
- No direct file edits.

- [ ] **Step 1: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: pass.

- [ ] **Step 2: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: pass.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: pass.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: pass. If Postgres tests are skipped because `NOEMA_TEST_DATABASE_URL` is unset, report that clearly.

- [ ] **Step 5: Run Postgres-backed tests when database is available**

Run:

```bash
NOEMA_TEST_DATABASE_URL="$NOEMA_TEST_DATABASE_URL" cargo test -p noema-core memory_persistence::postgres_tests -- --nocapture
```

Expected: pass when `NOEMA_TEST_DATABASE_URL` points at a disposable Postgres database.

- [ ] **Step 6: Inspect final diff**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: no whitespace errors. Report unrelated pre-existing dirty files separately.

## Self-Review Notes

- Spec coverage: this combined plan covers typed IDs, SQLx derive support, the `actors` interface table, actor-role migration, row-mapping cleanup, context graph/CLI updates, docs, and validation.
- Placeholder scan: no placeholder or future-filled implementation steps are required for this slice.
- Type consistency: typed IDs are introduced before refs use them; `ActorRef` is used only for actor roles; `ObjectRef` remains for non-actor concrete object refs.
- Scope check: this is one coherent persistence/data-model slice. It intentionally avoids ORM adoption, SQLx offline macros, universal object registry work, and governable scope modeling.
