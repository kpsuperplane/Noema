# Context Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add durable rolling context compaction checkpoints so Noema respects each selected model's context window without repeatedly trimming transcript context.

**Architecture:** Add store-backed `conversation_context_summaries`, provider context metadata, a prompt-context planner, and a runtime compaction service. The runtime will assemble prompts from stable instructions, the latest active summary, and post-checkpoint verbatim turns; it will compact synchronously only when the selected model would exceed its context window, and opportunistically after turns once the projected next prompt crosses a threshold.

**Tech Stack:** Rust, Tokio, SurrealDB embedded store, existing Noema daemon runtime, Swift FoundationModels bridge, React web chat error rendering through existing `ErrorNotice` transcript items.

---

## File Structure

- Create `crates/noema-core/src/store/context_summaries.rs`
  - Owns `NewConversationContextSummary`, `ConversationContextSummaryRecord`, `ConversationContextSummaryStatus`, and store helpers for inserting, activating, failing, superseding, and reading summaries.
- Modify `crates/noema-core/src/store.rs`
  - Adds `mod context_summaries;` and public re-exports for summary types.
- Modify `crates/noema-core/src/store/schema.rs`
  - Defines the strict `conversation_context_summaries` table and indexes.
- Modify `crates/noema-core/src/conversation.rs`
  - Adds `sequence_index` to `ConversationItemRecord` because compaction checkpoints need exact covered ranges.
- Modify `crates/noema-core/src/store/conversations.rs`
  - Preserves `sequence_index` when converting rows into `ConversationItemRecord`.
  - Adds a query for visible text transcript items after a sequence index.
- Modify `crates/noema-core/src/store/tests.rs`
  - Adds store tests for summary lifecycle and sequence-index replay.
- Modify `crates/noema-core/src/provider/contract.rs`
  - Adds provider context metadata and optional token counting to `ModelProvider`.
- Modify `crates/noema-core/src/daemon/runtime/handle.rs`
  - Adds object-safe runtime provider methods for metadata and token counting.
- Modify `crates/noema-core/src/provider/adapters/foundation_local.rs`
  - Advertises Foundation Models context metadata, passes max output tokens to the bridge, and exposes token counting through the bridge when available.
- Modify `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs`
  - Adds `CountTokens` and `max_output_tokens` to `Generate`.
- Modify `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`
  - Sends token-count requests and generation options through JSON lines.
- Modify `crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift`
  - Implements token counting and `GenerationOptions(maximumResponseTokens:)`.
- Create `crates/noema-core/src/daemon/runtime/context_window.rs`
  - Owns prompt budget math, token estimates, and context-window fit decisions.
- Create `crates/noema-core/src/daemon/runtime/prompt_context.rs`
  - Owns context assembly from active summaries and post-checkpoint transcript items.
- Create `crates/noema-core/src/daemon/runtime/context_compaction.rs`
  - Owns compaction prompts, structured parsing, summary insertion, retry policy, and background scheduling entrypoints.
- Modify `crates/noema-core/src/daemon/runtime.rs`
  - Registers new runtime modules.
- Modify `crates/noema-core/src/daemon/runtime/turn.rs`
  - Delegates context assembly and compaction preflight to new modules.
  - Triggers background compaction after successful turns.
- Modify `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
  - Adds a recoverable turn-failure helper for compaction preflight errors.
- Modify `crates/noema-core/src/daemon/tests.rs`
  - Adds runtime tests for synchronous compaction, background compaction, restart reuse, and failure behavior.
- Web dashboard:
  - No new files and no new controls for the first slice.
  - Existing `ErrorNotice` rendering in `crates/noema-core/web/src/components/transcript/ErrorNotice.tsx` is sufficient.

## Suggested Subagent Ownership

- Swift subagent, high effort: bridge protocol Swift support and bridge tests.
- Rust/backend subagent, high effort: store, provider metadata, runtime planner, compaction, daemon tests.
- Frontend/UI subagent, high effort: verify current GraphQL operations and transcript rendering cover compaction errors; report whether any web changes are needed.
- Review agents, high effort: one Rust/backend reviewer and one Swift/UI reviewer after implementation.

## Task 1: Store Checkpoints And Sequence Ranges

**Files:**
- Create: `crates/noema-core/src/store/context_summaries.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/conversation.rs`
- Modify: `crates/noema-core/src/store/conversations.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add failing store tests**

Add these tests to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn conversation_items_replay_exposes_sequence_index() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("turn");

    let first = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("first".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("first item");
    let second = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: Some(first.item_id.clone()),
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary"),
            content_text: Some("second".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("second item");

    assert_eq!(first.sequence_index, 1);
    assert_eq!(second.sequence_index, 2);
    let replay = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");
    assert_eq!(
        replay
            .iter()
            .map(|item| item.sequence_index)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[tokio::test]
async fn context_summary_lifecycle_supersedes_previous_active_checkpoint() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(NewConversation::local_chat_for_provider(
            "foundation_local",
            Some("default".to_string()),
            None,
        ))
        .await
        .expect("conversation");

    let first = store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: conversation.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "The user is exploring local model support.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 4,
            source_item_ids: vec!["item:1".to_string(), "item:4".to_string()],
            input_token_estimate: 900,
            summary_token_estimate: 64,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("first summary");
    let second = store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: conversation.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "The user wants durable compaction checkpoints.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 8,
            source_item_ids: vec!["item:1".to_string(), "item:8".to_string()],
            input_token_estimate: 1_800,
            summary_token_estimate: 82,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("second summary");

    let active = store
        .latest_active_context_summary(
            &conversation.conversation_id,
            "foundation_local",
            Some("default"),
        )
        .await
        .expect("active summary")
        .expect("active summary exists");

    assert_eq!(active.summary_id, second.summary_id);
    let first_reloaded = store
        .get_conversation_context_summary(&first.summary_id)
        .await
        .expect("first reload")
        .expect("first summary exists");
    assert_eq!(
        first_reloaded.status,
        crate::ConversationContextSummaryStatus::Superseded
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core conversation_items_replay_exposes_sequence_index context_summary_lifecycle_supersedes_previous_active_checkpoint --no-fail-fast
```

Expected: fail because `ConversationItemRecord::sequence_index`, `NewConversationContextSummary`, `ConversationContextSummaryStatus`, and summary store helpers do not exist.

- [ ] **Step 3: Add summary schema**

In `crates/noema-core/src/store/schema.rs`, after `conversation_items` indexes, add:

```rust
DEFINE TABLE IF NOT EXISTS conversation_context_summaries SCHEMAFULL;
DEFINE FIELD OVERWRITE summary_id ON TABLE conversation_context_summaries TYPE string;
DEFINE FIELD OVERWRITE conversation_id ON TABLE conversation_context_summaries TYPE string;
DEFINE FIELD OVERWRITE provider_kind ON TABLE conversation_context_summaries TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE model_profile ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE summary_text ON TABLE conversation_context_summaries TYPE string;
DEFINE FIELD OVERWRITE covered_item_start_sequence ON TABLE conversation_context_summaries TYPE int;
DEFINE FIELD OVERWRITE covered_item_end_sequence ON TABLE conversation_context_summaries TYPE int;
DEFINE FIELD OVERWRITE source_item_ids ON TABLE conversation_context_summaries TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE input_token_estimate ON TABLE conversation_context_summaries TYPE int ASSERT $value >= 0;
DEFINE FIELD OVERWRITE summary_token_estimate ON TABLE conversation_context_summaries TYPE int ASSERT $value >= 0;
DEFINE FIELD OVERWRITE compaction_provider_kind ON TABLE conversation_context_summaries TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE compaction_model_profile ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE status ON TABLE conversation_context_summaries TYPE string ASSERT $value INSIDE ['pending', 'active', 'failed', 'superseded'];
DEFINE FIELD OVERWRITE error_code ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE error_message ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE conversation_context_summaries TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE conversation_context_summaries TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversation_context_summaries_summary_id ON TABLE conversation_context_summaries COLUMNS summary_id UNIQUE;
DEFINE INDEX IF NOT EXISTS conversation_context_summaries_profile ON TABLE conversation_context_summaries COLUMNS conversation_id, provider_kind, model_profile, status, covered_item_end_sequence;
```

- [ ] **Step 4: Add sequence index to `ConversationItemRecord`**

In `crates/noema-core/src/conversation.rs`, update the record:

```rust
pub struct ConversationItemRecord {
    /// Durable Noema item id.
    pub item_id: String,
    /// Conversation that owns the item.
    pub conversation_id: String,
    /// Optional turn that owns the item.
    pub turn_id: Option<String>,
    /// Append-order sequence inside the conversation.
    pub sequence_index: i64,
    /// Semantic item kind.
    pub kind: ConversationItemKind,
    /// Item execution status.
    pub status: ConversationItemStatus,
    /// Readable item text, when any.
    pub content_text: Option<String>,
    /// Structured item payload.
    pub payload_json: Value,
}
```

In `crates/noema-core/src/store/conversations.rs`, include `sequence_index` in `ConversationItemRow` and `conversation_item_from_row`:

```rust
#[derive(Debug, Deserialize, SurrealValue)]
struct ConversationItemRow {
    item_id: String,
    conversation_id: String,
    turn_id: Option<String>,
    sequence_index: i64,
    kind: String,
    status: String,
    content_text: Option<String>,
    payload_json: Value,
}

fn conversation_item_from_row(
    row: ConversationItemRow,
) -> Result<ConversationItemRecord, StoreError> {
    Ok(ConversationItemRecord {
        item_id: row.item_id,
        conversation_id: row.conversation_id,
        turn_id: row.turn_id,
        sequence_index: row.sequence_index,
        kind: ConversationItemKind::parse(&row.kind).map_err(memory_enum_error)?,
        status: ConversationItemStatus::parse(&row.status).map_err(memory_enum_error)?,
        content_text: row.content_text,
        payload_json: row.payload_json,
    })
}
```

Also set `sequence_index` in `append_conversation_item`'s returned record.

- [ ] **Step 5: Add summary model and store helpers**

In `crates/noema-core/src/store/conversations.rs`, make the existing conversation guard visible to sibling store modules:

```rust
pub(crate) async fn require_conversation(&self, conversation_id: &str) -> Result<(), StoreError> {
    if self.conversation_exists(conversation_id).await? {
        Ok(())
    } else {
        Err(StoreError::ConversationNotFound {
            conversation_id: conversation_id.to_string(),
        })
    }
}
```

Create `crates/noema-core/src/store/context_summaries.rs`:

```rust
use serde::Deserialize;
use surrealdb::types::SurrealValue;

use crate::ConversationContextSummaryStatus;

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, record_fragment},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConversationContextSummary {
    pub conversation_id: String,
    pub provider_kind: String,
    pub model_profile: Option<String>,
    pub summary_text: String,
    pub covered_item_start_sequence: i64,
    pub covered_item_end_sequence: i64,
    pub source_item_ids: Vec<String>,
    pub input_token_estimate: u64,
    pub summary_token_estimate: u64,
    pub compaction_provider_kind: String,
    pub compaction_model_profile: Option<String>,
    pub status: ConversationContextSummaryStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationContextSummaryRecord {
    pub summary_id: String,
    pub conversation_id: String,
    pub provider_kind: String,
    pub model_profile: Option<String>,
    pub summary_text: String,
    pub covered_item_start_sequence: i64,
    pub covered_item_end_sequence: i64,
    pub source_item_ids: Vec<String>,
    pub input_token_estimate: u64,
    pub summary_token_estimate: u64,
    pub compaction_provider_kind: String,
    pub compaction_model_profile: Option<String>,
    pub status: ConversationContextSummaryStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl NoemaStore {
    pub async fn insert_conversation_context_summary(
        &self,
        summary: NewConversationContextSummary,
    ) -> Result<ConversationContextSummaryRecord, StoreError> {
        self.require_conversation(&summary.conversation_id).await?;
        if summary.status == ConversationContextSummaryStatus::Active {
            self.supersede_active_context_summaries(
                &summary.conversation_id,
                &summary.provider_kind,
                summary.model_profile.as_deref(),
            )
            .await?;
        }
        let summary_id = allocate_id("context-summary");
        self.db
            .query(
                r#"
                CREATE type::record('conversation_context_summaries', $record_id) SET
                  summary_id = $summary_id,
                  conversation_id = $conversation_id,
                  provider_kind = $provider_kind,
                  model_profile = $model_profile,
                  summary_text = $summary_text,
                  covered_item_start_sequence = $covered_item_start_sequence,
                  covered_item_end_sequence = $covered_item_end_sequence,
                  source_item_ids = $source_item_ids,
                  input_token_estimate = $input_token_estimate,
                  summary_token_estimate = $summary_token_estimate,
                  compaction_provider_kind = $compaction_provider_kind,
                  compaction_model_profile = $compaction_model_profile,
                  status = $status,
                  error_code = $error_code,
                  error_message = $error_message,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&summary_id)))
            .bind(("summary_id", summary_id.clone()))
            .bind(("conversation_id", summary.conversation_id.clone()))
            .bind(("provider_kind", summary.provider_kind.clone()))
            .bind(("model_profile", summary.model_profile.clone()))
            .bind(("summary_text", summary.summary_text.clone()))
            .bind(("covered_item_start_sequence", summary.covered_item_start_sequence))
            .bind(("covered_item_end_sequence", summary.covered_item_end_sequence))
            .bind(("source_item_ids", summary.source_item_ids.clone()))
            .bind(("input_token_estimate", summary.input_token_estimate as i64))
            .bind(("summary_token_estimate", summary.summary_token_estimate as i64))
            .bind(("compaction_provider_kind", summary.compaction_provider_kind.clone()))
            .bind(("compaction_model_profile", summary.compaction_model_profile.clone()))
            .bind(("status", summary.status.as_str().to_string()))
            .bind(("error_code", summary.error_code.clone()))
            .bind(("error_message", summary.error_message.clone()))
            .await?
            .check()?;
        Ok(ConversationContextSummaryRecord {
            summary_id,
            conversation_id: summary.conversation_id,
            provider_kind: summary.provider_kind,
            model_profile: summary.model_profile,
            summary_text: summary.summary_text,
            covered_item_start_sequence: summary.covered_item_start_sequence,
            covered_item_end_sequence: summary.covered_item_end_sequence,
            source_item_ids: summary.source_item_ids,
            input_token_estimate: summary.input_token_estimate,
            summary_token_estimate: summary.summary_token_estimate,
            compaction_provider_kind: summary.compaction_provider_kind,
            compaction_model_profile: summary.compaction_model_profile,
            status: summary.status,
            error_code: summary.error_code,
            error_message: summary.error_message,
        })
    }

    pub async fn latest_active_context_summary(
        &self,
        conversation_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
    ) -> Result<Option<ConversationContextSummaryRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let mut response = self
            .db
            .query(
                r#"
                SELECT summary_id, conversation_id, provider_kind, model_profile, summary_text,
                       covered_item_start_sequence, covered_item_end_sequence, source_item_ids,
                       input_token_estimate, summary_token_estimate,
                       compaction_provider_kind, compaction_model_profile,
                       status, error_code, error_message
                FROM conversation_context_summaries
                WHERE conversation_id = $conversation_id
                  AND provider_kind = $provider_kind
                  AND model_profile = $model_profile
                  AND status = 'active'
                ORDER BY covered_item_end_sequence DESC
                LIMIT 1;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("provider_kind", provider_kind.to_string()))
            .bind(("model_profile", model_profile.map(str::to_string)))
            .await?;
        let rows: Vec<ContextSummaryRow> = response.take(0)?;
        rows.into_iter().next().map(context_summary_from_row).transpose()
    }

    pub async fn get_conversation_context_summary(
        &self,
        summary_id: &str,
    ) -> Result<Option<ConversationContextSummaryRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT summary_id, conversation_id, provider_kind, model_profile, summary_text,
                       covered_item_start_sequence, covered_item_end_sequence, source_item_ids,
                       input_token_estimate, summary_token_estimate,
                       compaction_provider_kind, compaction_model_profile,
                       status, error_code, error_message
                FROM conversation_context_summaries
                WHERE summary_id = $summary_id
                LIMIT 1;
                "#,
            )
            .bind(("summary_id", summary_id.to_string()))
            .await?;
        let rows: Vec<ContextSummaryRow> = response.take(0)?;
        rows.into_iter().next().map(context_summary_from_row).transpose()
    }

    async fn supersede_active_context_summaries(
        &self,
        conversation_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
    ) -> Result<(), StoreError> {
        self.db
            .query(
                r#"
                UPDATE conversation_context_summaries SET
                  status = 'superseded',
                  updated_at = time::now()
                WHERE conversation_id = $conversation_id
                  AND provider_kind = $provider_kind
                  AND model_profile = $model_profile
                  AND status = 'active';
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("provider_kind", provider_kind.to_string()))
            .bind(("model_profile", model_profile.map(str::to_string)))
            .await?
            .check()?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ContextSummaryRow {
    summary_id: String,
    conversation_id: String,
    provider_kind: String,
    model_profile: Option<String>,
    summary_text: String,
    covered_item_start_sequence: i64,
    covered_item_end_sequence: i64,
    source_item_ids: Vec<String>,
    input_token_estimate: i64,
    summary_token_estimate: i64,
    compaction_provider_kind: String,
    compaction_model_profile: Option<String>,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
}

fn context_summary_from_row(
    row: ContextSummaryRow,
) -> Result<ConversationContextSummaryRecord, StoreError> {
    Ok(ConversationContextSummaryRecord {
        summary_id: row.summary_id,
        conversation_id: row.conversation_id,
        provider_kind: row.provider_kind,
        model_profile: row.model_profile,
        summary_text: row.summary_text,
        covered_item_start_sequence: row.covered_item_start_sequence,
        covered_item_end_sequence: row.covered_item_end_sequence,
        source_item_ids: row.source_item_ids,
        input_token_estimate: row.input_token_estimate.max(0) as u64,
        summary_token_estimate: row.summary_token_estimate.max(0) as u64,
        compaction_provider_kind: row.compaction_provider_kind,
        compaction_model_profile: row.compaction_model_profile,
        status: ConversationContextSummaryStatus::parse(&row.status)
            .map_err(|error| StoreError::Schema(error.to_string()))?,
        error_code: row.error_code,
        error_message: row.error_message,
    })
}
```

- [ ] **Step 6: Add summary status type and re-exports**

Add to `crates/noema-core/src/conversation.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationContextSummaryStatus {
    Pending,
    Active,
    Failed,
    Superseded,
}

impl ConversationContextSummaryStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "failed" => Ok(Self::Failed),
            "superseded" => Ok(Self::Superseded),
            _ => invalid_enum("conversation_context_summary_status", value),
        }
    }
}
```

Update `crates/noema-core/src/lib.rs` exports to include `ConversationContextSummaryStatus`.

Update `crates/noema-core/src/store.rs`:

```rust
mod context_summaries;

pub use context_summaries::{
    ConversationContextSummaryRecord, NewConversationContextSummary,
};
```

- [ ] **Step 7: Add post-checkpoint transcript query**

Add to `crates/noema-core/src/store/conversations.rs`:

```rust
pub async fn list_conversation_items_after_sequence_for_context(
    &self,
    conversation_id: &str,
    after_sequence_index: i64,
    limit: i64,
) -> Result<Vec<ConversationItemRecord>, StoreError> {
    self.require_conversation(conversation_id).await?;
    let limit = limit.clamp(1, 80);
    let mut response = self
        .db
        .query(
            r#"
            SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
            FROM conversation_items
            WHERE conversation_id = $conversation_id
              AND deleted_at = NONE
              AND sequence_index > $after_sequence_index
              AND kind IN ['user_text', 'assistant_text']
            ORDER BY sequence_index ASC
            LIMIT $limit;
            "#,
        )
        .bind(("conversation_id", conversation_id.to_string()))
        .bind(("after_sequence_index", after_sequence_index))
        .bind(("limit", limit))
        .await?;
    let rows: Vec<ConversationItemRow> = response.take(0)?;
    rows.into_iter().map(conversation_item_from_row).collect()
}
```

- [ ] **Step 8: Run tests to verify store task passes**

Run:

```bash
cargo test -p noema-core conversation_items_replay_exposes_sequence_index context_summary_lifecycle_supersedes_previous_active_checkpoint --no-fail-fast
```

Expected: both tests pass.

- [ ] **Step 9: Commit store checkpoint work**

Run:

```bash
git add crates/noema-core/src/conversation.rs crates/noema-core/src/lib.rs crates/noema-core/src/store.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/conversations.rs crates/noema-core/src/store/context_summaries.rs crates/noema-core/src/store/tests.rs
git commit -m "Add durable context summary checkpoints"
```

## Task 2: Provider Context Metadata And Foundation Token Hooks

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`
- Modify: `crates/noema-core/src/provider/adapters/foundation_local.rs`
- Modify: `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs`
- Modify: `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`
- Modify: `crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift`
- Test: provider adapter tests in the same Rust files

- [ ] **Step 1: Add failing provider contract tests**

In `crates/noema-core/src/provider/contract.rs` tests, add:

```rust
#[test]
fn default_provider_context_metadata_is_unknown() {
    let provider = EchoProvider;
    assert_eq!(provider.context_metadata(None).context_window_tokens, None);
    assert_eq!(provider.context_metadata(Some("mock")).default_output_reserve_tokens, None);
}
```

In `crates/noema-core/src/provider/adapters/foundation_local.rs` tests, add:

```rust
#[test]
fn foundation_local_advertises_context_window_metadata() {
    let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
        default_profile: "default".to_string(),
        bridge_path: None,
    })
    .expect("provider");

    let metadata = provider.context_metadata(Some("default"));
    assert_eq!(metadata.context_window_tokens, Some(4_096));
    assert_eq!(metadata.default_output_reserve_tokens, Some(512));
}
```

In `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs` tests, add:

```rust
#[test]
fn count_tokens_request_serializes() {
    let message = BridgeRequest {
        id: "request-count".to_string(),
        payload: BridgeRequestPayload::CountTokens {
            instructions: Some("system".to_string()),
            input: "hello".to_string(),
        },
    };

    let value = serde_json::to_value(&message).expect("json");

    assert_eq!(value["payload"]["type"], "count_tokens");
    assert_eq!(value["payload"]["instructions"], "system");
    assert_eq!(value["payload"]["input"], "hello");
}

#[test]
fn generate_request_includes_optional_max_output_tokens() {
    let message = BridgeRequest {
        id: "request-generate".to_string(),
        payload: BridgeRequestPayload::Generate {
            session_id: "session:1".to_string(),
            input: "hello".to_string(),
            max_output_tokens: Some(256),
        },
    };

    let value = serde_json::to_value(&message).expect("json");

    assert_eq!(value["payload"]["type"], "generate");
    assert_eq!(value["payload"]["max_output_tokens"], 256);
}
```

- [ ] **Step 2: Run provider tests to verify they fail**

Run:

```bash
cargo test -p noema-core default_provider_context_metadata_is_unknown foundation_local_advertises_context_window_metadata count_tokens_request_serializes generate_request_includes_optional_max_output_tokens --no-fail-fast
```

Expected: fail because metadata and protocol fields do not exist.

- [ ] **Step 3: Add provider metadata types**

In `crates/noema-core/src/provider/contract.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderContextMetadata {
    pub context_window_tokens: Option<u32>,
    pub default_output_reserve_tokens: Option<u32>,
    pub compact_summary_target_tokens: Option<u32>,
}

pub trait ModelProvider: Send + Sync {
    fn generate(
        &self,
        request: GenerateRequest,
    ) -> impl Future<Output = Result<GenerateResponse, ProviderError>> + Send;

    fn default_tool_classification_model(&self) -> Option<String> {
        Some(DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string())
    }

    fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata::default()
    }

    fn count_tokens(
        &self,
        _instructions: Option<&str>,
        _input: &str,
        _model: Option<&str>,
    ) -> impl Future<Output = Result<Option<u32>, ProviderError>> + Send {
        async { Ok(None) }
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> impl Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a {
        async move {
            let _ = on_event;
            self.generate(request).await
        }
    }
}
```

Update `crates/noema-core/src/provider.rs` and `crates/noema-core/src/lib.rs` re-exports to include `ProviderContextMetadata`.

- [ ] **Step 4: Add runtime object-safe metadata methods**

In `crates/noema-core/src/daemon/runtime/handle.rs`, add methods to `RuntimeModelProvider`:

```rust
fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
    ProviderContextMetadata::default()
}

fn count_tokens<'a>(
    &'a self,
    instructions: Option<&'a str>,
    input: &'a str,
    model: Option<&'a str>,
) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>>;
```

In the blanket impl:

```rust
fn context_metadata(&self, model: Option<&str>) -> ProviderContextMetadata {
    ModelProvider::context_metadata(self, model)
}

fn count_tokens<'a>(
    &'a self,
    instructions: Option<&'a str>,
    input: &'a str,
    model: Option<&'a str>,
) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
    Box::pin(async move { ModelProvider::count_tokens(self, instructions, input, model).await })
}
```

Add `ProviderContextMetadata` to the imports.

- [ ] **Step 5: Add Foundation metadata and max-output forwarding**

In `crates/noema-core/src/provider/adapters/foundation_local.rs`, add:

```rust
pub const FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS: u32 = 4_096;
pub const FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS: u32 = 512;
pub const FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS: u32 = 512;
```

Implement:

```rust
fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
    ProviderContextMetadata {
        context_window_tokens: Some(FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS),
        default_output_reserve_tokens: Some(FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS),
        compact_summary_target_tokens: Some(FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS),
    }
}
```

When calling `bridge.generate`, pass `request.options.max_output_tokens`.

- [ ] **Step 6: Extend Rust bridge protocol**

In `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs`, change `Generate` and add `CountTokens`:

```rust
Generate {
    session_id: String,
    input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
},
CountTokens {
    instructions: Option<String>,
    input: String,
},
```

Add a response payload:

```rust
TokenCount {
    tokens: u32,
},
```

- [ ] **Step 7: Extend Rust bridge process**

In `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`, update generation signatures:

```rust
pub async fn generate(
    &mut self,
    conversation_id: String,
    model_profile: String,
    instructions: Option<String>,
    input: String,
    max_output_tokens: Option<u32>,
    on_delta: &mut (dyn FnMut(String) + Send),
) -> Result<String, FoundationBridgeError>
```

Pass `max_output_tokens` into `BridgeRequestPayload::Generate`.

Add:

```rust
pub async fn count_tokens(
    &mut self,
    instructions: Option<String>,
    input: String,
) -> Result<u32, FoundationBridgeError> {
    let response = self
        .send_request(BridgeRequest {
            id: "count_tokens".to_string(),
            payload: BridgeRequestPayload::CountTokens {
                instructions,
                input,
            },
        })
        .await?;
    match response.payload {
        BridgeResponsePayload::TokenCount { tokens } => Ok(tokens),
        payload => Err(FoundationBridgeError::BridgeProtocol(format!(
            "unexpected token count response {payload:?}"
        ))),
    }
}
```

- [ ] **Step 8: Extend Swift bridge**

In `crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift`, update payload decoding:

```swift
case generate(sessionID: String, input: String, maxOutputTokens: Int?)
case countTokens(instructions: String?, input: String)
```

Add coding key:

```swift
case maxOutputTokens = "max_output_tokens"
```

In the `"generate"` branch:

```swift
let maxOutputTokens = try container.decodeIfPresent(Int.self, forKey: .maxOutputTokens)
self = .generate(sessionID: sessionID, input: input, maxOutputTokens: maxOutputTokens)
```

Add protocol method:

```swift
func countTokens(instructions: String?, input: String) -> [String: Any]
```

In `UnavailableHandler`:

```swift
func countTokens(instructions: String?, input: String) -> [String: Any] {
    errorPayload(code: "foundation_unavailable", message: reason)
}
```

In `FoundationModelsHandler.generate`:

```swift
let options = GenerationOptions(maximumResponseTokens: maxOutputTokens)
let response = try await session.respond(to: input, options: options)
```

In `FoundationModelsHandler`, add:

```swift
func countTokens(instructions: String?, input: String) -> [String: Any] {
    do {
        var total = try SystemLanguageModel.default.tokenCount(for: Prompt(input))
        if let instructions {
            total += try SystemLanguageModel.default.tokenCount(for: Instructions(instructions))
        }
        return [
            "type": "token_count",
            "tokens": total
        ]
    } catch {
        return errorPayload(
            code: "token_count_failed",
            message: "Foundation Models token count failed: \(error.localizedDescription)"
        )
    }
}
```

If the exact FoundationModels initializer names differ in the installed SDK, use the local Swift interface as authority and keep the JSON response shape unchanged.

In `runBridge`, route `.countTokens` and pass `maxOutputTokens` into `handler.generate`.

- [ ] **Step 9: Run provider and Swift validation**

Run:

```bash
cargo test -p noema-core default_provider_context_metadata_is_unknown foundation_local_advertises_context_window_metadata count_tokens_request_serializes generate_request_includes_optional_max_output_tokens --no-fail-fast
swift build --package-path crates/noema-core/apple-foundation-bridge
```

Expected: Rust tests pass. Swift build passes on macOS with Xcode that includes FoundationModels, or reports SDK availability problems that should be fixed against the installed interface.

- [ ] **Step 10: Commit provider metadata work**

Run:

```bash
git add crates/noema-core/src/provider/contract.rs crates/noema-core/src/provider.rs crates/noema-core/src/lib.rs crates/noema-core/src/daemon/runtime/handle.rs crates/noema-core/src/provider/adapters/foundation_local.rs crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs crates/noema-core/src/provider/adapters/foundation_bridge_process.rs crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift
git commit -m "Add provider context metadata"
```

## Task 3: Prompt Context Planner

**Files:**
- Create: `crates/noema-core/src/daemon/runtime/context_window.rs`
- Create: `crates/noema-core/src/daemon/runtime/prompt_context.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/prompts.rs`
- Test: module unit tests and `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write failing context-window unit tests**

Create `crates/noema-core/src/daemon/runtime/context_window.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderContextMetadata;

    #[test]
    fn unknown_context_window_fits_without_budgeting() {
        let budget = ContextBudget::from_metadata(ProviderContextMetadata::default());
        assert!(budget.fits(10_000));
        assert_eq!(budget.available_input_tokens(), None);
    }

    #[test]
    fn context_window_reserves_output_and_safety_tokens() {
        let budget = ContextBudget::from_metadata(ProviderContextMetadata {
            context_window_tokens: Some(4_096),
            default_output_reserve_tokens: Some(512),
            compact_summary_target_tokens: Some(512),
        });

        assert_eq!(budget.available_input_tokens(), Some(3_456));
        assert!(budget.fits(3_456));
        assert!(!budget.fits(3_457));
    }

    #[test]
    fn estimated_tokens_are_conservative_for_text() {
        assert_eq!(estimate_text_tokens("abc"), 1);
        assert_eq!(estimate_text_tokens("abcd"), 2);
        assert_eq!(estimate_text_tokens(""), 0);
    }
}
```

- [ ] **Step 2: Run context-window tests to verify they fail**

Run:

```bash
cargo test -p noema-core context_window --no-fail-fast
```

Expected: fail because the module is not registered and implementation is missing.

- [ ] **Step 3: Implement context-window module**

Implement `crates/noema-core/src/daemon/runtime/context_window.rs`:

```rust
use crate::provider::ProviderContextMetadata;

const DEFAULT_CONTEXT_SAFETY_TOKENS: u32 = 128;
const FALLBACK_CHARS_PER_TOKEN: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContextBudget {
    context_window_tokens: Option<u32>,
    output_reserve_tokens: Option<u32>,
    safety_tokens: u32,
    compact_summary_target_tokens: Option<u32>,
}

impl ContextBudget {
    pub(super) fn from_metadata(metadata: ProviderContextMetadata) -> Self {
        Self {
            context_window_tokens: metadata.context_window_tokens,
            output_reserve_tokens: metadata.default_output_reserve_tokens,
            safety_tokens: DEFAULT_CONTEXT_SAFETY_TOKENS,
            compact_summary_target_tokens: metadata.compact_summary_target_tokens,
        }
    }

    pub(super) fn output_reserve_tokens(self) -> Option<u32> {
        self.output_reserve_tokens
    }

    pub(super) fn compact_summary_target_tokens(self) -> Option<u32> {
        self.compact_summary_target_tokens
    }

    pub(super) fn available_input_tokens(self) -> Option<u32> {
        let window = self.context_window_tokens?;
        let output = self.output_reserve_tokens.unwrap_or(0);
        Some(window.saturating_sub(output).saturating_sub(self.safety_tokens))
    }

    pub(super) fn fits(self, input_tokens: u32) -> bool {
        self.available_input_tokens()
            .is_none_or(|available| input_tokens <= available)
    }
}

pub(super) fn estimate_text_tokens(value: &str) -> u32 {
    let chars = value.chars().count();
    if chars == 0 {
        0
    } else {
        chars.div_ceil(FALLBACK_CHARS_PER_TOKEN) as u32
    }
}
```

Register it in `crates/noema-core/src/daemon/runtime.rs`:

```rust
mod context_window;
```

- [ ] **Step 4: Add failing prompt-context assembly test**

In `crates/noema-core/src/daemon/tests.rs`, add a capturing provider with metadata:

```rust
#[derive(Debug, Default)]
struct MetadataCapturingProvider {
    requests: Mutex<Vec<GenerateRequest>>,
}

impl super::runtime::RuntimeModelProvider for MetadataCapturingProvider {
    fn context_metadata(&self, _model: Option<&str>) -> crate::ProviderContextMetadata {
        crate::ProviderContextMetadata {
            context_window_tokens: Some(4_096),
            default_output_reserve_tokens: Some(512),
            compact_summary_target_tokens: Some(512),
        }
    }

    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        _model: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let total = instructions.map_or(0, estimated_test_tokens)
                + estimated_test_tokens(input);
            Ok(Some(total))
        })
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests.lock().expect("requests").push(request.clone());
            Ok(GenerateResponse {
                output: assistant_with_no_memories("fake answer"),
                provider: "test".to_string(),
                model: request.model.unwrap_or_else(|| "fake-model".to_string()),
                response_id: None,
                usage: None,
            })
        })
    }
}

fn estimated_test_tokens(value: &str) -> u32 {
    value.chars().count().div_ceil(3) as u32
}
```

Then add:

```rust
#[tokio::test]
async fn prompt_context_uses_active_summary_and_post_checkpoint_items() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(Some("default".to_string()), None)
        .await
        .expect("conversation");

    store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: started.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "Summary: the user approved rolling durable compaction.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 2,
            source_item_ids: vec!["item:1".to_string(), "item:2".to_string()],
            input_token_estimate: 400,
            summary_token_estimate: 16,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("summary");

    append_test_text_item(&store, &started.conversation_id, "post checkpoint user").await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "current turn".to_string(), tx)
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let instructions = requests
        .last()
        .and_then(|request| request.instructions.as_deref())
        .expect("instructions");
    assert!(instructions.contains("Compacted conversation context:"));
    assert!(instructions.contains("rolling durable compaction"));
    assert!(instructions.contains("post checkpoint user"));
}
```

Add helper:

```rust
async fn append_test_text_item(store: &crate::NoemaStore, conversation_id: &str, text: &str) {
    store
        .append_conversation_item(crate::NewConversationItem {
            conversation_id: conversation_id.to_string(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some(text.to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("test item");
}
```

- [ ] **Step 5: Run prompt-context test to verify it fails**

Run:

```bash
cargo test -p noema-core prompt_context_uses_active_summary_and_post_checkpoint_items --no-fail-fast
```

Expected: fail because runtime still uses fixed recent transcript rendering.

- [ ] **Step 6: Add prompt context module**

Create `crates/noema-core/src/daemon/runtime/prompt_context.rs`:

```rust
use crate::{
    ConversationContextSummaryRecord, ConversationItemRecord, NoemaStore,
    daemon::prompts::render_recent_transcript_for_prompt,
    store::StoreError,
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PromptContext {
    pub(super) active_summary: Option<ConversationContextSummaryRecord>,
    pub(super) transcript_items: Vec<ConversationItemRecord>,
    pub(super) rendered_context: String,
}

pub(super) async fn load_prompt_context(
    store: &NoemaStore,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
) -> Result<PromptContext, StoreError> {
    let active_summary = store
        .latest_active_context_summary(conversation_id, provider_kind, model_profile)
        .await?;
    let after_sequence = active_summary
        .as_ref()
        .map_or(0, |summary| summary.covered_item_end_sequence);
    let transcript_items = store
        .list_conversation_items_after_sequence_for_context(conversation_id, after_sequence, 40)
        .await?;
    let rendered_context = render_prompt_context(active_summary.as_ref(), &transcript_items);
    Ok(PromptContext {
        active_summary,
        transcript_items,
        rendered_context,
    })
}

fn render_prompt_context(
    summary: Option<&ConversationContextSummaryRecord>,
    transcript_items: &[ConversationItemRecord],
) -> String {
    let transcript = render_recent_transcript_for_prompt(transcript_items);
    match summary {
        Some(summary) => format!(
            "Compacted conversation context:\n{}\n\nRecent transcript after compacted checkpoint:\n{}",
            summary.summary_text, transcript
        ),
        None => transcript,
    }
}
```

Register in `crates/noema-core/src/daemon/runtime.rs`:

```rust
mod prompt_context;
```

- [ ] **Step 7: Use prompt context from `turn.rs`**

In `crates/noema-core/src/daemon/runtime/turn.rs`, replace:

```rust
let recent_context_items = self
    .store
    .list_recent_conversation_items_for_context(&conversation_id, 24)
    .await?;
let recent_transcript = render_recent_transcript_for_prompt(&recent_context_items);
```

with:

```rust
let provider = self.provider_for_kind(&conversation.provider_kind)?;
let prompt_context = super::prompt_context::load_prompt_context(
    &self.store,
    &conversation_id,
    &conversation.provider_kind,
    conversation.model.as_deref(),
)
.await?;
```

Pass `&prompt_context.rendered_context` into `build_structured_turn_system_prompt`.

Remove the second duplicate `let provider = self.provider_for_kind(...)` before `generate_streaming`.

- [ ] **Step 8: Run context planner tests**

Run:

```bash
cargo test -p noema-core context_window prompt_context_uses_active_summary_and_post_checkpoint_items --no-fail-fast
```

Expected: pass.

- [ ] **Step 9: Commit prompt context planner**

Run:

```bash
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/runtime/context_window.rs crates/noema-core/src/daemon/runtime/prompt_context.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Assemble prompts from compacted context"
```

## Task 4: Compaction Service

**Files:**
- Create: `crates/noema-core/src/daemon/runtime/context_compaction.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/runtime/prompt_context.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing synchronous compaction test**

In `crates/noema-core/src/daemon/tests.rs`, add a provider that compacts when it sees the compaction prompt:

```rust
#[derive(Debug, Default)]
struct CompactingProvider {
    requests: Mutex<Vec<GenerateRequest>>,
}

impl super::runtime::RuntimeModelProvider for CompactingProvider {
    fn context_metadata(&self, _model: Option<&str>) -> crate::ProviderContextMetadata {
        crate::ProviderContextMetadata {
            context_window_tokens: Some(1_200),
            default_output_reserve_tokens: Some(200),
            compact_summary_target_tokens: Some(160),
        }
    }

    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        _model: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(Some(
                instructions.map_or(0, estimated_test_tokens) + estimated_test_tokens(input),
            ))
        })
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let input_text = match &request.input {
                GenerateInput::Text(text) => text.as_str(),
            };
            self.requests.lock().expect("requests").push(request.clone());
            let text = if input_text.contains("NOEMA_CONTEXT_COMPACTION") {
                r#"{"type":"noema_context_summary","summary":"The user discussed context compaction and wants stable prompt caching.","open_loops":["Implement rolling checkpoints"],"durable_decisions":["Use durable summaries"],"uncertainty":["No UI controls in first slice"]}"#.to_string()
            } else {
                r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"after compaction"},{"kind":"memory_proposals","proposals":[]}]}"#.to_string()
            };
            Ok(GenerateResponse {
                output: vec![GenerateOutputItem::AssistantText { text }],
                provider: "test".to_string(),
                model: request.model.unwrap_or_else(|| "fake-model".to_string()),
                response_id: None,
                usage: None,
            })
        })
    }
}
```

Add:

```rust
#[tokio::test]
async fn over_limit_turn_compacts_before_agent_generation() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CompactingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let conversation_id = runtime
        .start_conversation(Some("default".to_string()), None)
        .await
        .expect("conversation")
        .conversation_id;

    for index in 0..10 {
        append_test_text_item(
            &store,
            &conversation_id,
            &format!("large history {index} {}", "context ".repeat(90)),
        )
        .await;
    }

    let items = collect_turn(&runtime, conversation_id.clone(), "continue".to_string())
        .await
        .expect("turn");
    runtime.shutdown().await;

    assert_eq!(assistant_text(&items), "after compaction");
    let active = store
        .latest_active_context_summary(&conversation_id, "foundation_local", Some("default"))
        .await
        .expect("active")
        .expect("summary");
    assert!(active.summary_text.contains("stable prompt caching"));
    let requests = provider.requests.lock().expect("requests");
    assert!(
        requests.iter().any(|request| match &request.input {
            GenerateInput::Text(text) => text.contains("NOEMA_CONTEXT_COMPACTION"),
        })
    );
}
```

- [ ] **Step 2: Run synchronous compaction test to verify it fails**

Run:

```bash
cargo test -p noema-core over_limit_turn_compacts_before_agent_generation --no-fail-fast
```

Expected: fail because compaction service does not exist.

- [ ] **Step 3: Implement compaction prompt and parser**

Create `crates/noema-core/src/daemon/runtime/context_compaction.rs`:

```rust
use serde::Deserialize;
use serde_json::json;

use crate::{
    ConversationContextSummaryStatus, ConversationItemRecord, GenerateInput, GenerateOptions,
    GenerateOutputItem, GenerateRequest, NewConversationContextSummary, NoemaStore, ProviderError,
};

use super::{context_window::ContextBudget, handle::RuntimeModelProvider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ContextCompactionError {
    Provider(String),
    MalformedSummary(String),
    EmptySource,
}

impl std::fmt::Display for ContextCompactionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider(message) => write!(formatter, "{message}"),
            Self::MalformedSummary(message) => write!(formatter, "{message}"),
            Self::EmptySource => write!(formatter, "no conversation context was available to compact"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ContextSummaryEnvelope {
    #[serde(rename = "type")]
    envelope_type: String,
    summary: String,
    #[serde(default)]
    open_loops: Vec<String>,
    #[serde(default)]
    durable_decisions: Vec<String>,
    #[serde(default)]
    uncertainty: Vec<String>,
}

pub(super) async fn compact_context(
    store: &NoemaStore,
    provider: &dyn RuntimeModelProvider,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
    previous_summary: Option<&crate::ConversationContextSummaryRecord>,
    source_items: &[ConversationItemRecord],
    budget: ContextBudget,
) -> Result<crate::ConversationContextSummaryRecord, ContextCompactionError> {
    if previous_summary.is_none() && source_items.is_empty() {
        return Err(ContextCompactionError::EmptySource);
    }

    let prompt = compaction_prompt(previous_summary, source_items, budget);
    let response = provider
        .generate_streaming(
            GenerateRequest {
                model: model_profile.map(str::to_string),
                input: GenerateInput::Text(prompt.clone()),
                instructions: Some(context_compaction_instructions()),
                options: GenerateOptions {
                    max_output_tokens: budget.compact_summary_target_tokens(),
                    temperature: Some(0.0),
                    require_noema_response: false,
                },
            },
            &mut |_| {},
        )
        .await
        .map_err(|error| ContextCompactionError::Provider(error.to_string()))?;
    let summary_text = parse_summary_response(response)?;
    let first_sequence = previous_summary
        .map(|summary| summary.covered_item_start_sequence)
        .or_else(|| source_items.first().map(|item| item.sequence_index))
        .ok_or(ContextCompactionError::EmptySource)?;
    let last_sequence = source_items
        .last()
        .map(|item| item.sequence_index)
        .or_else(|| previous_summary.map(|summary| summary.covered_item_end_sequence))
        .ok_or(ContextCompactionError::EmptySource)?;
    let source_item_ids = sampled_source_item_ids(source_items);
    let input_token_estimate = super::context_window::estimate_text_tokens(&prompt) as u64;
    let summary_token_estimate = super::context_window::estimate_text_tokens(&summary_text) as u64;

    store
        .insert_conversation_context_summary(NewConversationContextSummary {
            conversation_id: conversation_id.to_string(),
            provider_kind: provider_kind.to_string(),
            model_profile: model_profile.map(str::to_string),
            summary_text,
            covered_item_start_sequence: first_sequence,
            covered_item_end_sequence: last_sequence,
            source_item_ids,
            input_token_estimate,
            summary_token_estimate,
            compaction_provider_kind: provider_kind.to_string(),
            compaction_model_profile: model_profile.map(str::to_string),
            status: ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .map_err(|error| ContextCompactionError::Provider(error.to_string()))
}

fn context_compaction_instructions() -> String {
    "Return strict JSON: {\"type\":\"noema_context_summary\",\"summary\":\"summary text\",\"open_loops\":[],\"durable_decisions\":[],\"uncertainty\":[]}. Summarize only the supplied Noema transcript context. Preserve uncertainty. Do not invent memories.".to_string()
}

fn compaction_prompt(
    previous_summary: Option<&crate::ConversationContextSummaryRecord>,
    source_items: &[ConversationItemRecord],
    budget: ContextBudget,
) -> String {
    let previous = previous_summary
        .map(|summary| summary.summary_text.as_str())
        .unwrap_or("none");
    let transcript = source_items
        .iter()
        .filter_map(|item| {
            let role = match item.kind {
                crate::ConversationItemKind::UserText => "User",
                crate::ConversationItemKind::AssistantText => "Noema",
                _ => return None,
            };
            item.content_text
                .as_deref()
                .map(|text| format!("{role}: {text}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "NOEMA_CONTEXT_COMPACTION\nTarget summary tokens: {:?}\nPrevious summary:\n{}\n\nTranscript to compact:\n{}",
        budget.compact_summary_target_tokens(),
        previous,
        transcript
    )
}

fn parse_summary_response(response: crate::GenerateResponse) -> Result<String, ContextCompactionError> {
    let text = response
        .output
        .into_iter()
        .find_map(|item| match item {
            GenerateOutputItem::AssistantText { text } => Some(text),
            _ => None,
        })
        .ok_or_else(|| ContextCompactionError::MalformedSummary("compaction returned no assistant text".to_string()))?;
    let envelope: ContextSummaryEnvelope = serde_json::from_str(&text)
        .map_err(|error| ContextCompactionError::MalformedSummary(error.to_string()))?;
    if envelope.envelope_type != "noema_context_summary" || envelope.summary.trim().is_empty() {
        return Err(ContextCompactionError::MalformedSummary(
            "compaction response did not contain a noema_context_summary summary".to_string(),
        ));
    }
    Ok(format!(
        "{}\n\nOpen loops: {}\nDurable decisions: {}\nUncertainty: {}",
        envelope.summary.trim(),
        json!(envelope.open_loops),
        json!(envelope.durable_decisions),
        json!(envelope.uncertainty)
    ))
}

fn sampled_source_item_ids(items: &[ConversationItemRecord]) -> Vec<String> {
    if items.len() <= 16 {
        return items.iter().map(|item| item.item_id.clone()).collect();
    }
    items
        .iter()
        .take(8)
        .chain(items.iter().rev().take(8).collect::<Vec<_>>().into_iter().rev())
        .map(|item| item.item_id.clone())
        .collect()
}
```

Register `mod context_compaction;` in `crates/noema-core/src/daemon/runtime.rs`.

- [ ] **Step 4: Add prompt-fit preflight**

In `prompt_context.rs`, add a planner result:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlannedPromptContext {
    pub(super) context: PromptContext,
    pub(super) estimated_input_tokens: u32,
    pub(super) fits: bool,
}
```

Add a helper that builds instructions, asks provider for token count, and falls back to `estimate_text_tokens`.

Use this signature:

```rust
pub(super) async fn plan_prompt_context(
    store: &NoemaStore,
    provider: &dyn RuntimeModelProvider,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
    turn_index: u64,
    cwd: Option<&str>,
    agent_identity: &crate::daemon::agent_onboarding::AgentPromptIdentity,
    rendered_tools: &str,
    current_input: &str,
) -> Result<PlannedPromptContext, DaemonError>
```

Build the candidate instructions with `build_structured_turn_system_prompt`, count `instructions + current_input`, and compare with `ContextBudget::from_metadata(provider.context_metadata(model_profile))`.

- [ ] **Step 5: Invoke synchronous compaction from turn preflight**

In `turn.rs`, before calling the agent provider, use `plan_prompt_context`. If it does not fit, call `compact_context`, reload prompt context, and re-plan once. If the second plan does not fit, run the smaller-summary retry once by using half the target summary tokens in the `ContextBudget`.

The final provider `GenerateRequest` must set:

```rust
options: GenerateOptions {
    max_output_tokens: budget.output_reserve_tokens(),
    require_noema_response: true,
    ..GenerateOptions::default()
}
```

- [ ] **Step 6: Run synchronous compaction test**

Run:

```bash
cargo test -p noema-core over_limit_turn_compacts_before_agent_generation --no-fail-fast
```

Expected: pass.

- [ ] **Step 7: Commit compaction service**

Run:

```bash
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/runtime/context_compaction.rs crates/noema-core/src/daemon/runtime/prompt_context.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Compact context before over-limit turns"
```

## Task 5: Failure Handling And Background Compaction

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/context_compaction.rs`
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing preflight failure test**

Add to `crates/noema-core/src/daemon/tests.rs`:

```rust
#[derive(Debug, Default)]
struct FailingCompactionProvider;

impl super::runtime::RuntimeModelProvider for FailingCompactionProvider {
    fn context_metadata(&self, _model: Option<&str>) -> crate::ProviderContextMetadata {
        crate::ProviderContextMetadata {
            context_window_tokens: Some(1_200),
            default_output_reserve_tokens: Some(200),
            compact_summary_target_tokens: Some(160),
        }
    }

    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        _model: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(Some(
                instructions.map_or(0, estimated_test_tokens) + estimated_test_tokens(input),
            ))
        })
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let input_text = match &request.input {
                GenerateInput::Text(text) => text.as_str(),
            };
            if input_text.contains("NOEMA_CONTEXT_COMPACTION") {
                return Err(ProviderError::ProviderUnavailable {
                    provider: "test".to_string(),
                    message: "summary model unavailable".to_string(),
                });
            }
            Ok(GenerateResponse {
                output: assistant_with_no_memories("unexpected agent call"),
                provider: "test".to_string(),
                model: request.model.unwrap_or_else(|| "fake-model".to_string()),
                response_id: None,
                usage: None,
            })
        })
    }
}

#[tokio::test]
async fn failed_preflight_compaction_records_recoverable_error_and_blocks_agent_call() {
    let (handle, store) =
        test_runtime_handle_with_store(FailingCompactionProvider::default()).await;
    let conversation_id = handle
        .start_conversation(Some("default".to_string()), None)
        .await
        .expect("conversation")
        .conversation_id;
    for index in 0..10 {
        append_test_text_item(
            &store,
            &conversation_id,
            &format!("large history {index} {}", "context ".repeat(90)),
        )
        .await;
    }

    let (result, events) = collect_turn_result(
        &handle,
        conversation_id.clone(),
        "continue".to_string(),
    )
    .await;
    handle.shutdown().await;

    assert!(result.is_err());
    assert!(events.iter().any(|event| matches!(
        event,
        TurnStreamEvent::ConversationItem { item, .. }
            if matches!(
                item.as_ref(),
                TurnTranscriptItem::ErrorNotice { message, recoverable: true }
                    if message.contains("Context compaction failed before this turn could run")
            )
    )));
    let failed = store
        .list_context_summaries_for_conversation(&conversation_id)
        .await
        .expect("summaries");
    assert!(failed.iter().any(|summary| {
        summary.status == crate::ConversationContextSummaryStatus::Failed
            && summary.error_message.as_deref().is_some_and(|message| message.contains("summary model unavailable"))
    }));
}
```

If `test_runtime_handle_with_store` only accepts `FakeCodexProvider`, replace it with this generic helper and keep `test_runtime_handle` as the FakeCodex convenience wrapper:

```rust
async fn test_runtime_handle(provider: FakeCodexProvider) -> CodexRuntimeHandle {
    test_runtime_handle_with_store(provider).await.0
}

async fn test_runtime_handle_with_store<P>(
    provider: P,
) -> (CodexRuntimeHandle, crate::NoemaStore)
where
    P: super::runtime::RuntimeModelProvider + 'static,
{
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    std::mem::forget(home);
    let handle = CodexRuntimeHandle::spawn_with_provider(Arc::new(provider), store.clone())
        .await
        .expect("runtime");
    (handle, store)
}
```

- [ ] **Step 2: Add failing background compaction test**

Add:

```rust
#[tokio::test]
async fn successful_turn_schedules_background_compaction_when_threshold_crossed() {
    let (handle, store) = test_runtime_handle_with_store(CompactingProvider::default()).await;
    let conversation_id = handle
        .start_conversation(Some("default".to_string()), None)
        .await
        .expect("conversation")
        .conversation_id;
    for index in 0..8 {
        append_test_text_item(
            &store,
            &conversation_id,
            &format!("threshold history {index} {}", "context ".repeat(60)),
        )
        .await;
    }

    let items = collect_turn(&handle, conversation_id.clone(), "short".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert_eq!(assistant_text(&items), "after compaction");
    let active = store
        .latest_active_context_summary(&conversation_id, "codex", Some("default"))
        .await
        .expect("active lookup");
    assert!(active.is_some());
}
```

- [ ] **Step 3: Run failure/background tests to verify they fail**

Run:

```bash
cargo test -p noema-core failed_preflight_compaction_records_recoverable_error_and_blocks_agent_call successful_turn_schedules_background_compaction_when_threshold_crossed --no-fail-fast
```

Expected: fail because recoverable compaction errors and background scheduling are not implemented.

- [ ] **Step 4: Add recoverable failure persistence**

In `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`, add:

```rust
pub(super) async fn record_recoverable_turn_failure(
    &mut self,
    context: &ConversationMemoryContext,
    message: String,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) -> Result<(), DaemonError> {
    self.store.fail_conversation_turn(&context.turn_id).await?;
    self.update_conversation_agent_status(
        &context.conversation_id,
        PersistedAgentStatus::Error,
        item_tx,
    )
    .await?;
    let notice = TurnTranscriptItem::ErrorNotice {
        message,
        recoverable: true,
    };
    self.persist_and_send_turn_item(context, notice, item_tx).await
}
```

- [ ] **Step 5: Persist failed summary rows**

In `context_compaction.rs`, add:

```rust
pub(super) async fn record_failed_compaction(
    store: &NoemaStore,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
    error_code: &str,
    error_message: &str,
) -> Result<(), crate::store::StoreError> {
    store
        .insert_conversation_context_summary(NewConversationContextSummary {
            conversation_id: conversation_id.to_string(),
            provider_kind: provider_kind.to_string(),
            model_profile: model_profile.map(str::to_string),
            summary_text: String::new(),
            covered_item_start_sequence: 0,
            covered_item_end_sequence: 0,
            source_item_ids: Vec::new(),
            input_token_estimate: 0,
            summary_token_estimate: 0,
            compaction_provider_kind: provider_kind.to_string(),
            compaction_model_profile: model_profile.map(str::to_string),
            status: ConversationContextSummaryStatus::Failed,
            error_code: Some(error_code.to_string()),
            error_message: Some(error_message.to_string()),
        })
        .await
        .map(|_| ())
}
```

Add `list_context_summaries_for_conversation` to the store helper for tests.

- [ ] **Step 6: Add threshold scheduler**

In `context_compaction.rs`, add:

```rust
const BACKGROUND_COMPACTION_THRESHOLD_PERCENT: u32 = 70;

pub(super) async fn maybe_compact_after_turn(
    store: &NoemaStore,
    provider: &dyn RuntimeModelProvider,
    conversation_id: &str,
    provider_kind: &str,
    model_profile: Option<&str>,
) {
    let metadata = provider.context_metadata(model_profile);
    let Some(window) = metadata.context_window_tokens else {
        return;
    };
    let Ok(context) = super::prompt_context::load_prompt_context(
        store,
        conversation_id,
        provider_kind,
        model_profile,
    )
    .await else {
        return;
    };
    let projected_tokens = super::context_window::estimate_text_tokens(&context.rendered_context);
    if projected_tokens.saturating_mul(100) < window.saturating_mul(BACKGROUND_COMPACTION_THRESHOLD_PERCENT) {
        return;
    }
    let budget = ContextBudget::from_metadata(metadata);
    if let Err(error) = compact_context(
        store,
        provider,
        conversation_id,
        provider_kind,
        model_profile,
        context.active_summary.as_ref(),
        &context.transcript_items,
        budget,
    )
    .await
    {
        let _ = record_failed_compaction(
            store,
            conversation_id,
            provider_kind,
            model_profile,
            "background_compaction_failed",
            &error.to_string(),
        )
        .await;
    }
}
```

Call it after `persist_successful_provider_turn` succeeds in `turn.rs`. The first slice may await this call directly after the user-visible turn has been persisted. A separate worker queue can replace it after the behavior is stable.

- [ ] **Step 7: Run failure/background tests**

Run:

```bash
cargo test -p noema-core failed_preflight_compaction_records_recoverable_error_and_blocks_agent_call successful_turn_schedules_background_compaction_when_threshold_crossed --no-fail-fast
```

Expected: pass.

- [ ] **Step 8: Commit failure/background work**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/context_compaction.rs crates/noema-core/src/daemon/runtime/transcript_persistence.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/store/context_summaries.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Handle context compaction failures"
```

## Task 6: Restart/Resume And Web Dashboard Verification

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Inspect only: `crates/noema-core/web/src/components/transcript/ErrorNotice.tsx`
- Inspect only: `crates/noema-core/web/src/graphql/operations.ts`

- [ ] **Step 1: Add restart/resume test**

Extend the existing restart-context test pattern in `crates/noema-core/src/daemon/tests.rs` with:

```rust
#[tokio::test]
async fn runtime_reuses_compacted_context_after_restart() {
    if let Ok(phase) = std::env::var("NOEMA_COMPACTION_RESTART_TEST_PHASE") {
        let home = PathBuf::from(
            std::env::var("NOEMA_COMPACTION_RESTART_TEST_HOME").expect("restart home"),
        );
        match phase.as_str() {
            "write" => compaction_restart_write_phase(&home).await,
            "read" => compaction_restart_read_phase(&home).await,
            other => panic!("unknown compaction restart phase: {other}"),
        }
        return;
    }

    let home = tempfile::tempdir().expect("temp noema home");
    run_compaction_restart_child_phase("write", home.path());
    run_compaction_restart_child_phase("read", home.path());
}
```

Add these helper functions next to the existing restart-context helpers:

```rust
fn run_compaction_restart_child_phase(phase: &str, home: &std::path::Path) {
    let output = Command::new(std::env::current_exe().expect("current test binary"))
        .arg("daemon::tests::runtime_reuses_compacted_context_after_restart")
        .arg("--exact")
        .env("NOEMA_COMPACTION_RESTART_TEST_PHASE", phase)
        .env("NOEMA_COMPACTION_RESTART_TEST_HOME", home)
        .output()
        .expect("run compaction restart test phase");
    assert!(
        output.status.success(),
        "compaction restart {phase} phase failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn compaction_restart_write_phase(home: &std::path::Path) {
    let paths = crate::NoemaPaths::from_noema_home(home).expect("paths");
    let config = crate::StoreConfig::from_paths(&paths);
    let store = crate::NoemaStore::open(&config).await.expect("open store");
    store.ensure_default_actors().await.expect("actors");
    let handle = CodexRuntimeHandle::spawn_with_provider_kind(
        Arc::new(MetadataCapturingProvider::default()),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let conversation_id = handle
        .start_conversation(Some("default".to_string()), None)
        .await
        .expect("conversation")
        .conversation_id;
    store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "Summary: restart should preserve compacted context.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 2,
            source_item_ids: vec!["item:1".to_string(), "item:2".to_string()],
            input_token_estimate: 400,
            summary_token_estimate: 16,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("summary");
    append_test_text_item(&store, &conversation_id, "post checkpoint after restart").await;
    std::fs::write(home.join("compaction_restart_conversation_id"), &conversation_id)
        .expect("write conversation id");
    handle.shutdown().await;
    store.close().await.expect("close store");
}

async fn compaction_restart_read_phase(home: &std::path::Path) {
    let conversation_id = std::fs::read_to_string(home.join("compaction_restart_conversation_id"))
        .expect("read conversation id");
    let paths = crate::NoemaPaths::from_noema_home(home).expect("paths");
    let config = crate::StoreConfig::from_paths(&paths);
    let store = crate::NoemaStore::open(&config).await.expect("open store");
    let provider = Arc::new(MetadataCapturingProvider::default());
    let handle = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let restarted_id = handle
        .start_conversation(Some("default".to_string()), None)
        .await
        .expect("conversation")
        .conversation_id;
    assert_eq!(conversation_id, restarted_id);
    let _items = collect_turn(&handle, restarted_id, "continue after restart".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;
    let requests = provider.requests.lock().expect("requests");
    let instructions = requests
        .last()
        .and_then(|request| request.instructions.as_deref())
        .expect("instructions");
    assert!(instructions.contains("Compacted conversation context:"));
    assert!(instructions.contains("restart should preserve compacted context"));
    assert!(instructions.contains("post checkpoint after restart"));
    store.close().await.expect("close store");
}
```

- [ ] **Step 2: Run restart/resume test**

Run:

```bash
cargo test -p noema-core runtime_reuses_compacted_context_after_restart --no-fail-fast
```

Expected: pass after Task 3 and Task 4 are complete.

- [ ] **Step 3: Verify web dashboard error rendering needs no new controls**

Inspect `crates/noema-core/web/src/components/transcript/ErrorNotice.tsx`; it should render a label based on the `recoverable` flag:

```tsx
export function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return <ErrorMarker message={message} label={recoverable ? "Notice" : "Error"} recoverable={recoverable} />;
}
```

Inspect `crates/noema-core/web/src/graphql/operations.ts` and confirm replay and subscription fragments include `ErrorNotice` selections for both `message` and `recoverable`.

No frontend controls should be added in this slice.

- [ ] **Step 4: Run frontend codegen/build only if GraphQL operations changed**

If no GraphQL operation changed, skip frontend validation and note that the first UI slice reuses existing `ErrorNotice` rendering.

If a GraphQL operation changed, run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: codegen, lint, and build pass.

- [ ] **Step 5: Commit restart/UI verification**

Run:

```bash
git add crates/noema-core/src/daemon/tests.rs
git commit -m "Verify context compaction resume"
```

If no files changed after inspection, do not create an empty commit.

## Task 7: Full Validation And Context Update

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update durable context**

Add a concise bullet to `docs/context/current.md` under the Foundation/provider runtime direction:

```markdown
- Context-window handling now uses durable rolling context compaction checkpoints: the runtime plans prompts against provider/model context metadata, stores active conversation summaries as inspectable derived state, compacts synchronously before over-limit turns, and reuses summaries across daemon restart/resume. The first web slice uses existing chat error notices for compaction failures and adds no dashboard controls.
```

- [ ] **Step 2: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: pass. If it fails, run `cargo fmt --all`, inspect the diff, and rerun the check.

- [ ] **Step 3: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: pass.

- [ ] **Step 4: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: pass.

- [ ] **Step 5: Run unit tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: pass. If a Noema daemon/OpenAI provider test fails with local socket `PermissionDenied` under sandboxing, rerun the same command with socket permissions and report the distinction.

- [ ] **Step 6: Run Swift bridge build**

Run:

```bash
swift build --package-path crates/noema-core/apple-foundation-bridge
```

Expected: pass on macOS with the FoundationModels SDK available.

- [ ] **Step 7: Run ship checklist**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: status shows only intended files before staging. `git diff --check` passes.

- [ ] **Step 8: Commit context update**

Run:

```bash
git add docs/context/current.md
git commit -m "Document context compaction implementation"
```

If `docs/context/current.md` was committed together with a previous task, skip this commit and report that it was already included.

## Review Plan

- [ ] **Rust/backend reviewer, high effort**
  - Review store schema, summary lifecycle, prompt planner, compaction retry policy, failure behavior, and restart tests.
  - Focus on data loss risks, hidden context drops, prompt over-limit paths, and whether summaries are properly derived state.
- [ ] **Swift/UI reviewer, high effort**
  - Review Foundation bridge protocol changes, `GenerationOptions`, token counting, cross-platform behavior, and whether web UI changes are correctly omitted.
  - Focus on macOS availability guards and generated JSON protocol compatibility.

## Execution Notes

- Work on `main` unless the user explicitly asks for a branch.
- Preserve unrelated dirty worktree changes.
- Use subagents with disjoint ownership:
  - Swift bridge files only for the Swift subagent.
  - Store/runtime/provider Rust files for the backend subagent.
  - Web inspection and UI report for the frontend subagent.
- The main agent owns final integration, validation, context doc update, and user-facing summary.
