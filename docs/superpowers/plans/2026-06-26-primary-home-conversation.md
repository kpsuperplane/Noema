# Primary Home Conversation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the web home chat load and append to one durable Noema-owned primary conversation for `human:local`, while removing provider-native thread ids from the current conversation contract.

**Architecture:** Store the home conversation pointer on `humans.primary_conversation_id`. The daemon exposes a `primary_conversation_start` web message that idempotently creates or loads the local human's primary conversation, starts a fresh provider session when needed, replays Noema-owned `conversation_items`, and appends later turns to the same durable conversation. Conversation continuity comes from Postgres, not provider-native thread state.

**Tech Stack:** Rust, Tokio, SQLx Postgres, serde/serde_json, ts-rs, Bun/Vite/React, current Noema daemon WebSocket protocol.

---

## Scope Check

This plan implements one cohesive feature: primary home conversation loading for the existing web chat. It also removes `provider_thread_id` because the approved design makes Noema-owned conversation state authoritative. Multiple-thread UI, provider switching UI, and provider optimization metadata remain out of scope.

## File Structure

- Modify `crates/noema-core/src/memory_persistence/postgres_schema.rs`: add `humans.primary_conversation_id`, remove `conversations.provider_thread_id`.
- Modify `crates/noema-core/src/memory_persistence/conversations.rs`: remove provider-thread fields and add primary-conversation repository methods.
- Modify `crates/noema-core/src/memory_persistence/repository.rs`: keep default actor upsert compatible with the new nullable column.
- Modify `crates/noema-core/src/memory_persistence/postgres_tests.rs`: add schema and primary-conversation repository tests.
- Modify `crates/noema-core/src/daemon/protocol.rs`: remove `provider_thread_id` from daemon start responses and `StartedConversation`.
- Modify `crates/noema-core/src/frontend_protocol.rs`: add `primary_conversation_start`, remove `provider_thread_id` from web start response, regenerate TypeScript.
- Modify `crates/noema-core/src/daemon/runtime.rs`: add primary-conversation start command and activate existing durable conversations without creating duplicates.
- Modify `crates/noema-core/src/daemon/web/mod.rs`: route `primary_conversation_start`, replay primary transcript, and tolerate malformed replay items with a visible warning item.
- Modify `crates/noema-core/src/daemon/client.rs`, `crates/noema-core/src/daemon/server.rs`, and `crates/noema-core/src/daemon/tests.rs`: align daemon client/server tests with the provider-thread removal and primary-start behavior.
- Modify `crates/noema-core/web/src/App.tsx`: request `primary_conversation_start` on page load.
- Modify `crates/noema-core/web/src/generated/noema.ts`: generated output from `bun run gen:types`.
- Modify docs under `docs/frontend/` and `docs/postgres.md`: describe Noema-owned continuity and primary home conversation behavior.

---

### Task 1: Schema And Repository Primary Conversation

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Modify: `crates/noema-core/src/memory_persistence/conversations.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Write failing schema tests**

Add these tests to `crates/noema-core/src/memory_persistence/postgres_tests.rs` near the existing schema/bootstrap tests:

```rust
#[tokio::test]
async fn postgres_bootstrap_adds_primary_conversation_to_humans() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let exists: bool = sqlx::query_scalar(
        r"
        SELECT EXISTS (
          SELECT 1
          FROM information_schema.columns
          WHERE table_schema = 'public'
            AND table_name = 'humans'
            AND column_name = 'primary_conversation_id'
        )
        ",
    )
    .fetch_one(repo.pool())
    .await
    .expect("primary conversation column exists query");

    assert!(exists);
}

#[tokio::test]
async fn postgres_bootstrap_removes_provider_thread_id_from_conversations() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let exists: bool = sqlx::query_scalar(
        r"
        SELECT EXISTS (
          SELECT 1
          FROM information_schema.columns
          WHERE table_schema = 'public'
            AND table_name = 'conversations'
            AND column_name = 'provider_thread_id'
        )
        ",
    )
    .fetch_one(repo.pool())
    .await
    .expect("provider thread column exists query");

    assert!(!exists);
}
```

- [ ] **Step 2: Run schema tests and verify they fail**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::postgres_bootstrap -- --nocapture
```

Expected: the first test fails because `primary_conversation_id` is absent, or the second fails because `provider_thread_id` still exists.

- [ ] **Step 3: Update schema**

In `crates/noema-core/src/memory_persistence/postgres_schema.rs`, change the `humans` table block to include:

```sql
  primary_conversation_id TEXT,
```

after `handle TEXT,`.

In the `conversations` table block, remove:

```sql
  provider_thread_id TEXT,
```

After the `conversations` table definition, add this guarded foreign key block because `humans` is created before `conversations`:

```sql
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'fk_humans_primary_conversation'
      AND conrelid = 'humans'::regclass
  ) THEN
    ALTER TABLE humans
      ADD CONSTRAINT fk_humans_primary_conversation
      FOREIGN KEY (primary_conversation_id)
      REFERENCES conversations(conversation_id)
      ON DELETE SET NULL;
  END IF;
END
$$;
```

- [ ] **Step 4: Run schema tests and verify they pass**

Run the same command as Step 2.

Expected: both tests pass.

- [ ] **Step 5: Write failing repository primary-conversation tests**

Add these tests to `crates/noema-core/src/memory_persistence/postgres_tests.rs`:

```rust
#[tokio::test]
async fn primary_conversation_is_created_and_reused_for_local_human() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let first = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("first primary conversation");
    let second = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("second primary conversation");

    assert_eq!(first.conversation_id, second.conversation_id);

    let stored: Option<String> =
        sqlx::query_scalar("SELECT primary_conversation_id FROM humans WHERE human_id = $1")
            .bind("human:local")
            .fetch_one(repo.pool())
            .await
            .expect("stored primary conversation");
    assert_eq!(stored.as_deref(), Some(first.conversation_id.as_str()));
}

#[tokio::test]
async fn primary_conversation_replaces_deleted_assignment() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let first = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("first primary conversation");

    sqlx::query("UPDATE conversations SET lifecycle_status = 'deleted' WHERE conversation_id = $1")
        .bind(first.conversation_id.as_str())
        .execute(repo.pool())
        .await
        .expect("mark conversation deleted");

    let replacement = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("replacement primary conversation");

    assert_ne!(first.conversation_id, replacement.conversation_id);
}
```

- [ ] **Step 6: Run repository tests and verify they fail**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::primary_conversation -- --nocapture
```

Expected: compile fails because `get_or_create_primary_conversation` does not exist.

- [ ] **Step 7: Remove provider-thread fields from conversation models**

In `crates/noema-core/src/memory_persistence/conversations.rs`, remove `provider_thread_id` from `NewConversation`, `ConversationRecord`, `NewConversation::local_chat`, and `create_conversation`.

The final `NewConversation` and `ConversationRecord` shapes should be:

```rust
pub struct NewConversation {
    pub title: Option<String>,
    pub owner: ObjectRef,
    pub primary_human_id: Option<String>,
    pub primary_agent_id: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub metadata: Value,
}

pub struct ConversationRecord {
    pub conversation_id: String,
}
```

Update the insert in `create_conversation` to:

```rust
let record = sqlx::query_as::<_, (String,)>(
    r"
    INSERT INTO conversations (
      conversation_id, title, owner_object_type, owner_object_id,
      primary_human_id, primary_agent_id, provider, model,
      cwd, metadata
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
    RETURNING conversation_id
    ",
)
.bind(conversation_id)
.bind(title.as_deref())
.bind(owner.object_type.as_str())
.bind(owner.object_id.as_str())
.bind(primary_human_id.as_deref())
.bind(primary_agent_id.as_deref())
.bind(provider.as_str())
.bind(model.as_deref())
.bind(cwd.as_deref())
.bind(json_value(metadata))
.fetch_one(self.pool())
.await
.map_err(MemoryPersistenceError::Database)?;

Ok(ConversationRecord {
    conversation_id: record.0,
})
```

- [ ] **Step 8: Implement primary-conversation repository methods**

Add this method to the `impl PostgresMemoryRepository` block in `crates/noema-core/src/memory_persistence/conversations.rs` after `create_conversation`:

```rust
pub async fn get_or_create_primary_conversation(
    &self,
    human_id: &str,
    model: Option<String>,
    cwd: Option<String>,
) -> Result<ConversationRecord, MemoryPersistenceError> {
    validate_object_ref_for_pool(self.pool(), &ObjectRef::human(human_id)).await?;

    let mut tx = self
        .pool()
        .begin()
        .await
        .map_err(MemoryPersistenceError::Database)?;

    let primary_conversation_id = sqlx::query_scalar::<_, Option<String>>(
        r"
        SELECT primary_conversation_id
        FROM humans
        WHERE human_id = $1
        FOR UPDATE
        ",
    )
    .bind(human_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    if let Some(conversation_id) = primary_conversation_id {
        let active = sqlx::query_scalar::<_, i32>(
            r"
            SELECT 1
            FROM conversations
            WHERE conversation_id = $1
              AND lifecycle_status = 'active'
              AND deleted_at IS NULL
            LIMIT 1
            ",
        )
        .bind(conversation_id.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?
        .is_some();

        if active {
            tx.commit().await.map_err(MemoryPersistenceError::Database)?;
            return Ok(ConversationRecord { conversation_id });
        }
    }

    let conversation_id = allocate_postgres_id(self.pool(), "conversation").await?;
    sqlx::query(
        r"
        INSERT INTO conversations (
          conversation_id, title, owner_object_type, owner_object_id,
          primary_human_id, primary_agent_id, provider, model, cwd, metadata
        )
        VALUES ($1, $2, 'human', $3, $3, 'agent:primary', 'codex', $4, $5, '{}'::jsonb)
        ",
    )
    .bind(conversation_id.as_str())
    .bind("Home")
    .bind(human_id)
    .bind(model.as_deref())
    .bind(cwd.as_deref())
    .execute(&mut *tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    sqlx::query(
        r"
        UPDATE humans
        SET primary_conversation_id = $2,
            updated_at = now()
        WHERE human_id = $1
        ",
    )
    .bind(human_id)
    .bind(conversation_id.as_str())
    .execute(&mut *tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    tx.commit().await.map_err(MemoryPersistenceError::Database)?;
    Ok(ConversationRecord { conversation_id })
}
```

- [ ] **Step 9: Run repository tests and fix compile fallout**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::primary_conversation -- --nocapture
```

Expected: tests pass after all `provider_thread_id` references in repository tests are removed.

- [ ] **Step 10: Commit schema and repository changes**

```bash
git add crates/noema-core/src/memory_persistence/postgres_schema.rs crates/noema-core/src/memory_persistence/conversations.rs crates/noema-core/src/memory_persistence/postgres_tests.rs
git commit -m "feat: store primary conversation on humans"
```

---

### Task 2: Daemon And Web Protocol Primary Start

**Files:**
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Modify: `crates/noema-core/src/frontend_protocol.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/daemon/client.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Modify: `crates/noema-core/web/src/generated/noema.ts`

- [ ] **Step 1: Update failing protocol tests first**

In `crates/noema-core/src/daemon/tests.rs`, update `protocol_round_trips_requests_and_responses` so `ConversationStarted` has no provider thread field:

```rust
let response = DaemonResponse::ConversationStarted {
    conversation_id: "conversation_1".to_string(),
    provider: "codex".to_string(),
};
let encoded = serde_json::to_string(&response).expect("encode");
let decoded: DaemonResponse = serde_json::from_str(&encoded).expect("decode");
assert_eq!(decoded, response);
assert!(!encoded.contains("provider_thread_id"));
```

Add a web protocol serialization test in `crates/noema-core/src/frontend_protocol.rs` tests:

```rust
#[test]
fn web_client_message_supports_primary_conversation_start() {
    let message = serde_json::from_value::<WebClientMessage>(serde_json::json!({
        "type": "primary_conversation_start"
    }))
    .expect("primary start message");

    assert!(matches!(
        message,
        WebClientMessage::StartPrimary { model: None, cwd: None }
    ));
}
```

- [ ] **Step 2: Run protocol tests and verify they fail**

Run:

```bash
cargo test -p noema-core daemon::tests::protocol_round_trips_requests_and_responses
cargo test -p noema-core frontend_protocol::tests::web_client_message_supports_primary_conversation_start
```

Expected: compile fails because protocol structs still require `provider_thread_id` and `StartPrimary` does not exist.

- [ ] **Step 3: Remove provider-thread fields from daemon and web protocol**

In `crates/noema-core/src/daemon/protocol.rs`, change `DaemonResponse::ConversationStarted` and `StartedConversation` to:

```rust
ConversationStarted {
    conversation_id: String,
    provider: String,
},
```

```rust
pub struct StartedConversation {
    pub conversation_id: String,
}
```

In `crates/noema-core/src/frontend_protocol.rs`, add a new web client variant and remove the web server provider-thread field:

```rust
#[serde(rename = "primary_conversation_start")]
#[ts(rename = "primary_conversation_start")]
StartPrimary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    cwd: Option<String>,
},
```

`WebServerMessage::ConversationStarted` should contain only:

```rust
ConversationStarted {
    conversation_id: String,
    provider: String,
},
```

Update `WebServerMessage::conversation_started` to return `provider: "codex".to_string()` without `provider_thread_id`.

- [ ] **Step 4: Add runtime primary start command**

In `crates/noema-core/src/daemon/runtime.rs`, add this public handle method:

```rust
pub(super) async fn start_primary_conversation(
    &self,
    model: Option<String>,
    cwd: Option<String>,
) -> Result<StartedConversation, DaemonError> {
    let (reply, reply_rx) = oneshot::channel();
    self.sender
        .send(CodexRuntimeCommand::StartPrimaryConversation { model, cwd, reply })
        .await
        .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
    reply_rx
        .await
        .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
}
```

Add the enum variant:

```rust
StartPrimaryConversation {
    model: Option<String>,
    cwd: Option<String>,
    reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
},
```

Handle it in `CodexRuntimeActor::run`:

```rust
CodexRuntimeCommand::StartPrimaryConversation { model, cwd, reply } => {
    let _ = reply.send(self.start_primary_conversation(model, cwd).await);
}
```

Add this actor method:

```rust
async fn start_primary_conversation(
    &mut self,
    model: Option<String>,
    cwd: Option<String>,
) -> Result<StartedConversation, DaemonError> {
    self.memory_repository.ensure_default_actors().await?;
    let durable_conversation = self
        .memory_repository
        .get_or_create_primary_conversation("human:local", model.clone(), cwd.clone())
        .await?;

    if !self.conversations.contains_key(&durable_conversation.conversation_id) {
        let provider = self.runtime.start_conversation(model, cwd.clone()).await?;
        let next_turn_index = self
            .memory_repository
            .next_conversation_turn_index(&durable_conversation.conversation_id)
            .await?;
        self.conversations.insert(
            durable_conversation.conversation_id.clone(),
            ActiveConversation {
                provider,
                cwd,
                next_turn_index,
            },
        );
    }

    Ok(StartedConversation {
        conversation_id: durable_conversation.conversation_id,
    })
}
```

The `next_conversation_turn_index` method is defined in Task 2 Step 5.

- [ ] **Step 5: Add repository helper for resumed turn indexes**

In `crates/noema-core/src/memory_persistence/conversations.rs`, add:

```rust
pub async fn next_conversation_turn_index(
    &self,
    conversation_id: &str,
) -> Result<u64, MemoryPersistenceError> {
    validate_object_ref_for_pool(
        self.pool(),
        &ObjectRef::new(ObjectType::Conversation, conversation_id)?,
    )
    .await?;

    let next = sqlx::query_scalar::<_, Option<i64>>(
        r"
        SELECT COALESCE(MAX((metadata->>'turn_index')::bigint), 0) + 1
        FROM conversation_turns
        WHERE conversation_id = $1
          AND metadata ? 'turn_index'
        ",
    )
    .bind(conversation_id)
    .fetch_one(self.pool())
    .await
    .map_err(MemoryPersistenceError::Database)?
    .unwrap_or(1);

    Ok(u64::try_from(next).unwrap_or(1))
}
```

- [ ] **Step 6: Keep explicit new conversation start available**

In `CodexRuntimeActor::start_conversation`, remove provider-thread persistence but keep explicit new-thread behavior:

```rust
pub(super) async fn start_conversation(
    &mut self,
    model: Option<String>,
    cwd: Option<String>,
) -> Result<StartedConversation, DaemonError> {
    self.memory_repository.ensure_default_actors().await?;
    let provider = self.runtime.start_conversation(model.clone(), cwd.clone()).await?;
    let durable_conversation = self
        .memory_repository
        .create_conversation(NewConversation::local_chat(model, cwd.clone()))
        .await?;
    let conversation_id = durable_conversation.conversation_id;
    self.conversations.insert(
        conversation_id.clone(),
        ActiveConversation {
            provider,
            cwd,
            next_turn_index: 1,
        },
    );

    Ok(StartedConversation { conversation_id })
}
```

- [ ] **Step 7: Route `primary_conversation_start` in web server**

In `crates/noema-core/src/daemon/web/mod.rs`, add a match arm:

```rust
WebClientMessage::StartPrimary { model, cwd } => {
    match state.runtime.start_primary_conversation(model, cwd).await {
        Ok(started) => {
            let replay_records =
                visible_conversation_replay(&state.memory_repository, &started.conversation_id)
                    .await?;
            for message in conversation_start_messages(started, replay_records)? {
                send_ws_json(stream, &message).await?;
            }
        }
        Err(error) => send_ws_error(stream, error.to_string()).await?,
    }
}
```

- [ ] **Step 8: Update daemon client/server provider-thread fallout**

In `crates/noema-core/src/daemon/client.rs`, update the `DaemonResponse::ConversationStarted` match to construct:

```rust
StartedConversation { conversation_id }
```

In `crates/noema-core/src/daemon/server.rs`, update the `ConversationStart` response to omit `provider_thread_id`:

```rust
Ok(started) => DaemonResponse::ConversationStarted {
    conversation_id: started.conversation_id,
    provider: "codex".to_string(),
},
```

- [ ] **Step 9: Regenerate frontend types**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: `src/generated/noema.ts` includes `primary_conversation_start` and no `provider_thread_id`.

- [ ] **Step 10: Run protocol-focused tests**

Run:

```bash
cargo test -p noema-core daemon::tests::protocol_round_trips_requests_and_responses
cargo test -p noema-core frontend_protocol::tests::web_client_message_supports_primary_conversation_start
cargo test -p noema-core frontend_protocol::tests::generated_frontend_types_are_current
```

Expected: all pass.

- [ ] **Step 11: Commit protocol and runtime changes**

```bash
git add crates/noema-core/src/daemon/protocol.rs crates/noema-core/src/frontend_protocol.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/web/mod.rs crates/noema-core/src/daemon/client.rs crates/noema-core/src/daemon/server.rs crates/noema-core/src/daemon/tests.rs crates/noema-core/src/memory_persistence/conversations.rs crates/noema-core/web/src/generated/noema.ts
git commit -m "feat: start primary home conversation"
```

---

### Task 3: Replay Robustness And Frontend Startup

**Files:**
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/transcript.ts`

- [ ] **Step 1: Write replay warning test**

In `crates/noema-core/src/daemon/web/mod.rs`, add this unit test near existing replay tests:

```rust
#[test]
fn conversation_replay_skips_malformed_item_and_adds_warning() {
    let message = conversation_replay_message(
        "conversation_1".to_string(),
        vec![
            ConversationItemRecord {
                item_id: "item_bad".to_string(),
                conversation_id: "conversation_1".to_string(),
                turn_id: Some("turn_1".to_string()),
                kind: ConversationItemKind::Activity,
                status: ConversationItemStatus::Completed,
                content_text: None,
                payload_json: json!({ "not": "an activity payload" }),
            },
            ConversationItemRecord {
                item_id: "item_good".to_string(),
                conversation_id: "conversation_1".to_string(),
                turn_id: Some("turn_1".to_string()),
                kind: ConversationItemKind::AssistantText,
                status: ConversationItemStatus::Completed,
                content_text: Some("still visible".to_string()),
                payload_json: json!({}),
            },
        ],
    )
    .expect("replay message");

    let WebServerMessage::ConversationReplay { items, .. } = message else {
        panic!("expected replay message");
    };

    assert_eq!(items.len(), 2);
    assert!(matches!(
        &items[0].item,
        TurnTranscriptItem::ErrorNotice { message, recoverable: true }
            if message.contains("could not replay one saved item")
    ));
    assert!(matches!(
        &items[1].item,
        TurnTranscriptItem::AssistantText { text } if text == "still visible"
    ));
}
```

- [ ] **Step 2: Run replay warning test and verify it fails**

Run:

```bash
cargo test -p noema-core daemon::web::tests::conversation_replay_skips_malformed_item_and_adds_warning
```

Expected: fails because replay currently returns an error for malformed items.

- [ ] **Step 3: Make replay skip malformed rows with warning item**

In `conversation_replay_message`, replace the loop with:

```rust
let mut items = Vec::new();
for record in replay_records {
    match web_conversation_item_from_record(record) {
        Ok(Some(item)) => items.push(item),
        Ok(None) => {}
        Err(error) => items.push(WebConversationItem::new(
            format!("replay_warning_{}", items.len() + 1),
            None,
            TurnTranscriptItem::ErrorNotice {
                message: format!("Noema could not replay one saved item: {error}"),
                recoverable: true,
            },
        )),
    }
}
Ok(WebServerMessage::ConversationReplay {
    conversation_id,
    items,
})
```

- [ ] **Step 4: Change frontend startup message**

In `crates/noema-core/web/src/App.tsx`, change:

```ts
const message: WebClientMessage = { type: "conversation_start" };
```

to:

```ts
const message: WebClientMessage = { type: "primary_conversation_start" };
```

- [ ] **Step 5: Keep transcript replay behavior unchanged**

Confirm `crates/noema-core/web/src/transcript.ts` still handles `conversation_replay` by replacing transcript entries:

```ts
if (message.type === "conversation_replay") {
  setters.setConversationId(message.conversation_id);
  setters.setTranscript(message.items.map(entryFromReplayItem).filter((entry) => entry !== null));
  setters.setPending(false);
  return;
}
```

If this block has drifted, restore it exactly as shown.

- [ ] **Step 6: Run frontend checks**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: both commands pass.

- [ ] **Step 7: Commit replay and frontend startup**

```bash
git add crates/noema-core/src/daemon/web/mod.rs crates/noema-core/web/src/App.tsx crates/noema-core/web/src/transcript.ts
git commit -m "feat: load primary conversation in web chat"
```

---

### Task 4: Runtime Restart Flow Tests

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write restart-style runtime test**

Add this test to `crates/noema-core/src/daemon/tests.rs` near the existing runtime actor tests:

```rust
#[tokio::test]
async fn runtime_primary_conversation_reuses_durable_conversation_after_restart() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script();

    let first_handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("first runtime");

    let first = first_handle
        .start_primary_conversation(None, None)
        .await
        .expect("first primary conversation");
    collect_turn(&first_handle, first.conversation_id.clone(), "hello".to_string())
        .await
        .expect("first turn");
    first_handle.shutdown().await;

    let second_handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("second runtime");

    let second = second_handle
        .start_primary_conversation(None, None)
        .await
        .expect("second primary conversation");
    assert_eq!(first.conversation_id, second.conversation_id);

    collect_turn(&second_handle, second.conversation_id.clone(), "again".to_string())
        .await
        .expect("second turn");
    second_handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let replay = repo
        .list_conversation_items(&first.conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let user_text_count = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::UserText)
        .count();
    assert_eq!(user_text_count, 2);
}
```

- [ ] **Step 2: Run restart test and verify it passes**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core daemon::tests::runtime_primary_conversation_reuses_durable_conversation_after_restart -- --nocapture
```

Expected: pass.

- [ ] **Step 3: Update existing runtime test expectations**

In `runtime_actor_allocates_distinct_conversation_ids`, remove this assertion:

```rust
assert_ne!(first.provider_thread_id, second.provider_thread_id);
```

Keep:

```rust
assert_ne!(first.conversation_id, second.conversation_id);
```

- [ ] **Step 4: Run daemon runtime tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core daemon::tests -- --nocapture
```

Expected: all daemon tests pass.

- [ ] **Step 5: Commit restart-flow tests**

```bash
git add crates/noema-core/src/daemon/tests.rs
git commit -m "test: cover primary conversation restart flow"
```

---

### Task 5: Documentation And Full Validation

**Files:**
- Modify: `docs/frontend/current-contract.md`
- Modify: `docs/frontend/navigation-workflows.md`
- Modify: `docs/frontend/object-model.md`
- Modify: `docs/postgres.md`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update frontend contract docs**

In `docs/frontend/current-contract.md`, replace the current behavior bullets that say daemon restarts remove active runtime conversation state and provider thread resume is uncertain with:

```markdown
- The web home chat loads `human:local.primary_conversation_id`.
- Durable chat history is reconstructed from `conversation_items`.
- Daemon runtime state is live coordination state only; after restart, Noema
  reactivates the durable conversation and assembles context from Postgres.
- Provider-native thread ids are not part of the current product contract.
```

- [ ] **Step 2: Update navigation and object-model docs**

In `docs/frontend/navigation-workflows.md`, under `Current conversation contract`, include:

```markdown
- The home chat is a single durable primary conversation for `human:local`.
- Future thread switching changes the human's primary conversation pointer.
```

In `docs/frontend/object-model.md`, under `Conversation`, include:

```markdown
- `human:local.primary_conversation_id` identifies the current home
  conversation in the initial slice.
- Provider changes should preserve the Noema conversation id.
```

- [ ] **Step 3: Update Postgres docs**

In `docs/postgres.md`, update the `humans` and `conversations` table descriptions so:

```markdown
humans.primary_conversation_id references conversations.conversation_id and stores
the default home chat for that human.
```

and no table description includes `provider_thread_id`.

- [ ] **Step 4: Update durable current context**

In `docs/context/current.md`, add a settled decision:

```markdown
- The web home chat should load `human:local.primary_conversation_id`; Noema
  conversation continuity is owned by Postgres, not provider-native thread
  state.
```

- [ ] **Step 5: Search for stale provider-thread wording**

Run:

```bash
rg -n "provider_thread_id|provider thread|provider-native thread" crates docs
```

Expected: no matches, except historical design docs under `docs/superpowers/specs/` or `docs/superpowers/plans/` if you intentionally leave already-committed history untouched. Current product docs and Rust/TypeScript code should have no stale active contract references.

- [ ] **Step 6: Run Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all pass. If tests needing Postgres are skipped because `NOEMA_TEST_DATABASE_URL` is unset, run the targeted Postgres tests with:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests -- --nocapture
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core daemon::tests -- --nocapture
```

- [ ] **Step 7: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: all pass.

- [ ] **Step 8: Run final ship checks without staging unrelated files**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: no whitespace errors. Dirty files unrelated to this feature, if any, remain unstaged and are reported.

- [ ] **Step 9: Commit docs and validation cleanup**

```bash
git add docs/frontend/current-contract.md docs/frontend/navigation-workflows.md docs/frontend/object-model.md docs/postgres.md docs/context/current.md crates/noema-core/web/src/generated/noema.ts
git commit -m "docs: document noema-owned conversation continuity"
```
