# Transcript Pagination Virtualization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace unbounded chat replay rendering with a paged transcript API, a client transcript window, and TanStack Virtual-backed rendering.

**Architecture:** Keep the store's `sequence_index` as the durable append-order implementation detail, but expose opaque cursors through GraphQL and the web client. Split the current `startPrimaryConversation` operation into read-only `primaryConversation`, side-effectful `ensurePrimaryConversation`, and paged `conversationTranscriptPage`; then load, merge, and render transcript pages through a dedicated frontend model and virtualized scroller.

**Tech Stack:** Rust, SurrealDB, async-graphql, React 19, Apollo Client, Bun, StyleX, `@tanstack/react-virtual`.

---

## Scope Notes

This is one integrated product slice with backend and frontend tasks that can be validated independently at each commit. Do not implement transcript page eviction. Do not migrate to TanStack Query or TanStack Router. Preserve all unrelated dirty worktree changes; several Rust runtime and GraphQL files may already have user edits.

## File Structure

- Modify `crates/noema-core/src/conversation/records.rs`: add store-level transcript page result structs.
- Modify `crates/noema-core/src/store/conversations.rs`: add cursor encode/decode helpers and visible transcript page reads.
- Modify `crates/noema-core/src/store/tests.rs`: add pagination tests next to existing conversation item replay tests.
- Modify `crates/noema-core/src/daemon/web/replay.rs`: carry opaque cursors in web replay items.
- Modify `crates/noema-core/src/daemon/protocol.rs`: add optional cursor on `TurnStreamEvent::ConversationItem`.
- Modify `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`: populate event cursors for durable items and leave transient cursors absent.
- Modify `crates/noema-core/src/graphql/chat.rs`: replace `GraphqlConversationStarted` API surface with primary conversation, ensure conversation, transcript page, and cursor-aware conversation events.
- Modify `crates/noema-core/src/graphql/schema.rs`: wire new query/mutation fields and update GraphQL tests.
- Modify `crates/noema-core/web/package.json` and `crates/noema-core/web/bun.lock`: add `@tanstack/react-virtual`.
- Modify `crates/noema-core/web/src/graphql/operations.ts`: replace `StartPrimaryConversation` with `PrimaryConversation`, `EnsurePrimaryConversation`, and `ConversationTranscriptPage`.
- Modify `crates/noema-core/web/src/shared/types.ts`: add `cursor?: string | null` to durable transcript entries.
- Modify `crates/noema-core/web/src/transcript/events.ts`: map paged replay and live cursors into transcript entries.
- Create `crates/noema-core/web/src/transcript/window.ts`: pure transcript window merge model.
- Create `crates/noema-core/web/src/transcript/window.test.ts`: node:test coverage for merge, dedupe, optimistic replacement, and cursors.
- Modify `crates/noema-core/web/src/app/App.tsx`: use the split GraphQL operations and transcript window model.
- Modify `crates/noema-core/web/src/components/ChatSurface.tsx`: pass top-pagination state into `Transcript`.
- Modify `crates/noema-core/web/src/components/transcript/Transcript.tsx`: provide rendered rows and top-load callback to the scroller.
- Modify `crates/noema-core/web/src/components/transcript/TranscriptScroller.tsx`: render rows with TanStack Virtual and expose near-top loading.
- Modify `crates/noema-core/web/src/components/transcript/TranscriptBottomFollower.tsx`: keep bottom-following compatible with virtual content height.
- Modify `crates/noema-core/web/src/components/transcript/scrollModel.ts`: keep bottom threshold helpers and remove assumptions that all rows are mounted.

---

### Task 1: Store Transcript Page Reads

**Files:**
- Modify: `crates/noema-core/src/conversation/records.rs`
- Modify: `crates/noema-core/src/store/conversations.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add failing store tests for visible latest page, cursor page, deleted filtering, and malformed cursors**

Add these tests after `conversation_items_replay_exposes_sequence_index` in `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn conversation_items_latest_page_returns_newest_visible_items_in_ascending_order() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    for label in ["one", "two", "three", "four"] {
        store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some(label.to_string()),
                payload_json: json!({}),
                metadata: json!({ "turn_index": 1 }),
            })
            .await
            .expect("item");
    }

    let page = store
        .list_visible_conversation_item_page(&conversation.conversation_id, None, 2)
        .await
        .expect("latest page");

    assert_eq!(
        page.items
            .iter()
            .map(|item| item.content_text.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("three"), Some("four")]
    );
    assert!(page.has_more_before);
    assert_eq!(page.before_cursor, page.items.first().map(|item| item.cursor.clone()));
    assert_eq!(page.limit, 2);
}

#[tokio::test]
async fn conversation_items_cursor_page_returns_items_before_cursor() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    for label in ["one", "two", "three", "four"] {
        store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some(label.to_string()),
                payload_json: json!({}),
                metadata: json!({ "turn_index": 1 }),
            })
            .await
            .expect("item");
    }

    let latest = store
        .list_visible_conversation_item_page(&conversation.conversation_id, None, 2)
        .await
        .expect("latest page");
    let older = store
        .list_visible_conversation_item_page(
            &conversation.conversation_id,
            latest.before_cursor.as_deref(),
            2,
        )
        .await
        .expect("older page");

    assert_eq!(
        older
            .items
            .iter()
            .map(|item| item.content_text.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("one"), Some("two")]
    );
    assert!(!older.has_more_before);
    assert_eq!(older.before_cursor, older.items.first().map(|item| item.cursor.clone()));
}

#[tokio::test]
async fn conversation_items_page_excludes_deleted_items() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    let deleted = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("deleted".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("deleted item");
    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("visible".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("visible item");

    store
        .db()
        .query("UPDATE conversation_items SET deleted_at = time::now() WHERE item_id = $item_id;")
        .bind(("item_id", deleted.item_id))
        .await
        .expect("soft delete")
        .check()
        .expect("checked soft delete");

    let page = store
        .list_visible_conversation_item_page(&conversation.conversation_id, None, 10)
        .await
        .expect("page");

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].content_text.as_deref(), Some("visible"));
}

#[tokio::test]
async fn conversation_items_page_rejects_malformed_cursor() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");

    let error = store
        .list_visible_conversation_item_page(&conversation.conversation_id, Some("not-a-cursor"), 10)
        .await
        .expect_err("malformed cursor should fail");

    assert!(
        matches!(error, StoreError::Schema(message) if message.contains("invalid conversation item cursor")),
        "{error:?}"
    );
}
```

- [ ] **Step 2: Run store tests and verify the new API does not exist yet**

Run:

```bash
cargo test -p noema-core conversation_items_latest_page_returns_newest_visible_items_in_ascending_order --no-fail-fast
```

Expected: compile failure mentioning `list_visible_conversation_item_page` or missing `cursor` on `ConversationItemRecord`.

- [ ] **Step 3: Add cursor and page structs**

In `crates/noema-core/src/conversation/records.rs`, extend `ConversationItemRecord` and add `ConversationItemPage`:

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
    /// Opaque pagination cursor derived from append order.
    pub cursor: String,
    /// Semantic item kind.
    pub kind: ConversationItemKind,
    /// Item execution status.
    pub status: ConversationItemStatus,
    /// Readable item text, when any.
    pub content_text: Option<String>,
    /// Structured item payload.
    pub payload_json: Value,
}

/// Bounded visible conversation item page returned to product replay callers.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationItemPage {
    /// Visible items in ascending transcript order.
    pub items: Vec<ConversationItemRecord>,
    /// Cursor to pass when fetching the next older page.
    pub before_cursor: Option<String>,
    /// Whether older visible items exist before this page.
    pub has_more_before: bool,
    /// Effective clamped item limit.
    pub limit: i64,
}
```

Keep existing fields and derive attributes intact. Add only the new `cursor` field and page struct.

- [ ] **Step 4: Implement cursor helpers and page reads**

In `crates/noema-core/src/store/conversations.rs`, add constants near the top of the file:

```rust
const CONVERSATION_ITEM_CURSOR_PREFIX: &str = "conversation_item:";
const DEFAULT_TRANSCRIPT_PAGE_LIMIT: i64 = 80;
const MAX_TRANSCRIPT_PAGE_LIMIT: i64 = 200;
```

Add these helpers near other private helpers:

```rust
fn conversation_item_cursor(sequence_index: i64) -> String {
    format!("{CONVERSATION_ITEM_CURSOR_PREFIX}{sequence_index}")
}

fn sequence_index_from_conversation_item_cursor(cursor: &str) -> Result<i64, StoreError> {
    let raw = cursor
        .strip_prefix(CONVERSATION_ITEM_CURSOR_PREFIX)
        .ok_or_else(|| StoreError::Schema(format!("invalid conversation item cursor: {cursor}")))?;
    let sequence_index = raw
        .parse::<i64>()
        .map_err(|_| StoreError::Schema(format!("invalid conversation item cursor: {cursor}")))?;
    if sequence_index < 1 {
        return Err(StoreError::Schema(format!(
            "invalid conversation item cursor: {cursor}"
        )));
    }
    Ok(sequence_index)
}

fn clamp_transcript_page_limit(limit: i64) -> i64 {
    limit.clamp(1, MAX_TRANSCRIPT_PAGE_LIMIT)
}
```

Update every `ConversationItemRecord` construction in this file to set `cursor: conversation_item_cursor(sequence_index)` or `cursor: conversation_item_cursor(row.sequence_index)`.

Add the new store method after `list_conversation_items`:

```rust
pub async fn list_visible_conversation_item_page(
    &self,
    conversation_id: &str,
    cursor: Option<&str>,
    limit: i64,
) -> Result<ConversationItemPage, StoreError> {
    self.require_conversation(conversation_id).await?;
    let limit = if limit == 0 {
        DEFAULT_TRANSCRIPT_PAGE_LIMIT
    } else {
        clamp_transcript_page_limit(limit)
    };
    let fetch_limit = limit.saturating_add(1);

    let mut rows = if let Some(cursor) = cursor {
        let before_sequence_index = sequence_index_from_conversation_item_cursor(cursor)?;
        let mut response = self
            .db
            .query(
                r#"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                FROM conversation_items
                WHERE conversation_id = $conversation_id
                  AND deleted_at = NONE
                  AND sequence_index < $before_sequence_index
                ORDER BY sequence_index DESC
                LIMIT $limit;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("before_sequence_index", before_sequence_index))
            .bind(("limit", fetch_limit))
            .await?;
        response.take::<Vec<ConversationItemRow>>(0)?
    } else {
        let mut response = self
            .db
            .query(
                r#"
                SELECT item_id, conversation_id, turn_id, kind, status, content_text, payload_json, sequence_index
                FROM conversation_items
                WHERE conversation_id = $conversation_id
                  AND deleted_at = NONE
                ORDER BY sequence_index DESC
                LIMIT $limit;
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("limit", fetch_limit))
            .await?;
        response.take::<Vec<ConversationItemRow>>(0)?
    };

    let has_more_before = i64::try_from(rows.len()).unwrap_or(i64::MAX) > limit;
    if has_more_before {
        rows.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    }
    rows.reverse();
    let items = rows
        .into_iter()
        .map(conversation_item_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let before_cursor = items.first().map(|item| item.cursor.clone());

    Ok(ConversationItemPage {
        items,
        before_cursor,
        has_more_before,
        limit,
    })
}
```

- [ ] **Step 5: Run focused store tests**

Run:

```bash
cargo test -p noema-core conversation_items_page --no-fail-fast
cargo test -p noema-core conversation_items_latest_page_returns_newest_visible_items_in_ascending_order --no-fail-fast
cargo test -p noema-core conversation_items_cursor_page_returns_items_before_cursor --no-fail-fast
```

Expected: all focused store pagination tests pass.

- [ ] **Step 6: Commit store pagination**

Run:

```bash
git add crates/noema-core/src/conversation/records.rs crates/noema-core/src/store/conversations.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: add paged conversation transcript store reads"
```

---

### Task 2: GraphQL Conversation API Split And Cursor Events

**Files:**
- Modify: `crates/noema-core/src/daemon/web/replay.rs`
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/graphql/chat.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Test: `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Add failing GraphQL tests for the new API**

In `crates/noema-core/src/graphql/schema.rs`, replace the existing `start_primary_conversation_uses_saved_agent_provider_preference` test query with `ensurePrimaryConversation`, and add these tests near the existing conversation subscription tests:

```rust
#[tokio::test]
async fn primary_conversation_returns_identity_without_transcript() {
    use crate::store::tests::test_store;

    let store = test_store().await;
    let conversation = store
        .get_or_create_primary_conversation_for_provider(
            "human:local",
            "codex",
            Some("gpt-test".to_string()),
            None,
        )
        .await
        .expect("primary conversation");
    let runtime = test_autofill_runtime(store.clone(), "ok").await;
    let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(store, runtime));

    let response = schema
        .execute(async_graphql::Request::new(
            r#"
            query {
              primaryConversation {
                provider
                conversationId
              }
            }
            "#,
        ))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    assert_eq!(
        data["primaryConversation"]["conversationId"],
        conversation.conversation_id
    );
    assert_eq!(data["primaryConversation"]["provider"], "codex");
    assert!(data["primaryConversation"].get("replay").is_none());
}

#[tokio::test]
async fn conversation_transcript_page_supports_latest_and_cursor_reads() {
    use crate::store::tests::test_store;

    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(crate::NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: serde_json::json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    for label in ["one", "two", "three"] {
        store
            .append_conversation_item(crate::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: crate::ConversationItemKind::UserText,
                status: crate::ConversationItemStatus::Completed,
                author: crate::ActorRef::human("human:local"),
                content_text: Some(label.to_string()),
                payload_json: serde_json::json!({}),
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("item");
    }

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let latest = schema
        .execute(async_graphql::Request::new(format!(
            r#"
            query {{
              conversationTranscriptPage(input: {{ conversationId: "{}", limit: 2 }}) {{
                items {{
                  itemId
                  cursor
                  item {{ __typename ... on UserText {{ text }} }}
                }}
                pageInfo {{ beforeCursor hasMoreBefore limit }}
              }}
            }}
            "#,
            conversation.conversation_id
        )))
        .await;

    assert!(latest.errors.is_empty(), "{:?}", latest.errors);
    let latest_data = latest.data.into_json().expect("latest json");
    let page = &latest_data["conversationTranscriptPage"];
    assert_eq!(page["items"][0]["item"]["text"], "two");
    assert_eq!(page["items"][1]["item"]["text"], "three");
    assert_eq!(page["pageInfo"]["hasMoreBefore"], true);
    let before_cursor = page["pageInfo"]["beforeCursor"].as_str().expect("cursor");

    let older = schema
        .execute(async_graphql::Request::new(format!(
            r#"
            query {{
              conversationTranscriptPage(input: {{ conversationId: "{}", cursor: "{}", limit: 2 }}) {{
                items {{ item {{ __typename ... on UserText {{ text }} }} }}
                pageInfo {{ hasMoreBefore }}
              }}
            }}
            "#,
            conversation.conversation_id, before_cursor
        )))
        .await;

    assert!(older.errors.is_empty(), "{:?}", older.errors);
    let older_data = older.data.into_json().expect("older json");
    assert_eq!(older_data["conversationTranscriptPage"]["items"][0]["item"]["text"], "one");
    assert_eq!(older_data["conversationTranscriptPage"]["pageInfo"]["hasMoreBefore"], false);
}
```

Update `subscription_streams_conversation_item_metadata` to request `cursor` and assert `event["cursor"].is_null()` for a manually-published transient-style test event. Add this second test after it:

```rust
#[tokio::test]
async fn subscription_streams_conversation_item_cursor_when_present() {
    let state = GraphqlState::for_tests();
    let subscriptions = state.subscriptions().clone();
    let schema = build_schema(state);
    let mut stream = schema.execute_stream(async_graphql::Request::new(
        r#"
        subscription {
          conversationEvents(conversationId: "conversation_1") {
            __typename
            ... on ConversationItemEvent {
              itemId
              cursor
            }
          }
        }
        "#,
    ));

    let ready = stream.next().await.expect("ready response");
    assert_eq!(
        ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
        "SubscriptionReadyEvent"
    );

    subscriptions.publish(ConversationLiveEvent::Turn {
        client_message_id: None,
        event: Box::new(TurnStreamEvent::ConversationItem {
            conversation_id: "conversation_1".to_string(),
            item_id: "item_1".to_string(),
            cursor: Some("conversation_item:1".to_string()),
            turn_id: Some("turn_1".to_string()),
            metadata: serde_json::json!({}),
            item: Box::new(crate::TurnTranscriptItem::UserText {
                text: "Hello".to_string(),
            }),
        }),
    });

    let response = stream.next().await.expect("item response");
    let data = response.data.into_json().expect("item json");
    let event = &data["conversationEvents"];
    assert_eq!(event["__typename"], "ConversationItemEvent");
    assert_eq!(event["itemId"], "item_1");
    assert_eq!(event["cursor"], "conversation_item:1");
}
```

- [ ] **Step 2: Run GraphQL tests and verify they fail**

Run:

```bash
cargo test -p noema-core primary_conversation_returns_identity_without_transcript --no-fail-fast
cargo test -p noema-core conversation_transcript_page_supports_latest_and_cursor_reads --no-fail-fast
```

Expected: compile or GraphQL validation failures for missing fields.

- [ ] **Step 3: Carry cursors through replay and durable live events**

In `crates/noema-core/src/daemon/web/replay.rs`, change `ConversationReplayItem`:

```rust
pub(crate) struct ConversationReplayItem {
    pub(crate) item_id: String,
    pub(crate) cursor: String,
    pub(crate) turn_id: Option<String>,
    pub(crate) item: TurnTranscriptItem,
}

impl ConversationReplayItem {
    pub(crate) fn new(
        item_id: String,
        cursor: String,
        turn_id: Option<String>,
        item: TurnTranscriptItem,
    ) -> Self {
        Self {
            item_id,
            cursor,
            turn_id,
            item,
        }
    }
}
```

Update `web_conversation_item_from_record` to pass `record.cursor`.

In `crates/noema-core/src/daemon/protocol.rs`, add an optional cursor to item events:

```rust
ConversationItem {
    conversation_id: String,
    item_id: String,
    cursor: Option<String>,
    turn_id: Option<String>,
    metadata: serde_json::Value,
    item: Box<TurnTranscriptItem>,
},
```

Update every `TurnStreamEvent::ConversationItem` construction:

- Durable persisted records in `send_conversation_item` use `cursor: Some(record.cursor)`.
- Transient runtime-only rows in `send_transient_turn_item` and GraphQL terminal error notices use `cursor: None`.
- Tests that manually construct events use `cursor: None` unless they are specifically testing a durable cursor.

- [ ] **Step 4: Replace GraphQL types and resolvers**

In `crates/noema-core/src/graphql/chat.rs`:

Remove `GraphqlConversationStarted`. Add:

```rust
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "PrimaryConversation")]
pub struct GraphqlPrimaryConversation {
    pub conversation_id: String,
    pub provider: String,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ConversationTranscriptPageInput")]
pub struct GraphqlConversationTranscriptPageInput {
    pub conversation_id: String,
    pub cursor: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationTranscriptPage")]
pub struct GraphqlConversationTranscriptPage {
    pub items: Vec<GraphqlConversationItem>,
    pub page_info: GraphqlConversationTranscriptPageInfo,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationTranscriptPageInfo")]
pub struct GraphqlConversationTranscriptPageInfo {
    pub before_cursor: Option<String>,
    pub has_more_before: bool,
    pub limit: i32,
}
```

Add `cursor: String` to `GraphqlConversationItem` and `cursor: Option<String>` to `GraphqlConversationItemEvent`. Keep the event cursor nullable because some existing subscription rows are transient runtime events without durable store positions.

Replace `start_primary_conversation` with:

```rust
pub(super) async fn primary_conversation(
    state: &GraphqlState,
) -> Result<Option<GraphqlPrimaryConversation>> {
    let store = state.store()?;
    let runtime = state.runtime()?;
    let provider_kind = primary_agent_provider_kind(store, runtime.provider_kind()).await?;
    let account = store
        .active_provider_account(&provider_kind)
        .await
        .map_err(graphql_error)?;
    if !crate::daemon::web::is_user_onboarded_for_chat(account) {
        return Err(async_graphql::Error::new(
            "Noema onboarding is incomplete. Connect a provider account before starting chat.",
        ));
    }
    let conversation = store
        .primary_conversation_for_human("human:local")
        .await
        .map_err(graphql_error)?;
    Ok(conversation.map(|conversation| GraphqlPrimaryConversation {
        conversation_id: conversation.conversation_id,
        provider: provider_kind,
    }))
}

pub(super) async fn ensure_primary_conversation(
    state: &GraphqlState,
    cwd: Option<String>,
) -> Result<GraphqlPrimaryConversation> {
    let store = state.store()?;
    let runtime = state.runtime()?;
    let provider_kind = primary_agent_provider_kind(store, runtime.provider_kind()).await?;
    let account = store
        .active_provider_account(&provider_kind)
        .await
        .map_err(graphql_error)?;
    if !crate::daemon::web::is_user_onboarded_for_chat(account) {
        return Err(async_graphql::Error::new(
            "Noema onboarding is incomplete. Connect a provider account before starting chat.",
        ));
    }
    let started = runtime
        .start_primary_conversation(cwd)
        .await
        .map_err(graphql_error)?;
    Ok(GraphqlPrimaryConversation {
        conversation_id: started.conversation_id,
        provider: provider_kind,
    })
}
```

Add `primary_conversation_for_human` to `NoemaStore` beside `get_or_create_primary_conversation_for_provider`. It reads `humans.primary_conversation_id`, validates ownership with `primary_conversation_matches_human`, and returns `Ok(None)` when no valid pointer exists:

```rust
pub async fn primary_conversation_for_human(
    &self,
    human_id: &str,
) -> Result<Option<ConversationRecord>, StoreError> {
    self.ensure_default_actors().await?;
    let mut response = self
        .db
        .query("SELECT primary_conversation_id FROM humans WHERE human_id = $human_id LIMIT 1;")
        .bind(("human_id", human_id.to_string()))
        .await?;
    let rows: Vec<PrimaryConversationRow> = response.take(0)?;
    let Some(Some(conversation_id)) = rows
        .into_iter()
        .next()
        .map(|row| row.primary_conversation_id)
    else {
        return Ok(None);
    };
    if !self
        .primary_conversation_matches_human(&conversation_id, human_id)
        .await?
    {
        return Ok(None);
    }
    Ok(Some(ConversationRecord { conversation_id }))
}
```

Add:

```rust
pub(super) async fn conversation_transcript_page(
    state: &GraphqlState,
    input: GraphqlConversationTranscriptPageInput,
) -> Result<GraphqlConversationTranscriptPage> {
    let limit = input.limit.unwrap_or(80);
    if limit < 1 {
        return Err(async_graphql::Error::new(
            "conversationTranscriptPage limit must be at least 1",
        ));
    }
    if limit > 200 {
        return Err(async_graphql::Error::new(
            "conversationTranscriptPage limit must be at most 200",
        ));
    }
    let page = state
        .store()?
        .list_visible_conversation_item_page(
            &input.conversation_id,
            input.cursor.as_deref(),
            i64::from(limit),
        )
        .await
        .map_err(graphql_error)?;
    let mut items = Vec::new();
    for record in page.items {
        if let Some(item) =
            crate::daemon::web::web_conversation_item_from_record(record).map_err(graphql_error)?
        {
            items.push(GraphqlConversationItem::from(item));
        }
    }
    Ok(GraphqlConversationTranscriptPage {
        items,
        page_info: GraphqlConversationTranscriptPageInfo {
            before_cursor: page.before_cursor,
            has_more_before: page.has_more_before,
            limit: i32::try_from(page.limit).unwrap_or(i32::MAX),
        },
    })
}
```

Update `conversation_events` pattern matching to pass event cursors:

```rust
crate::daemon::TurnStreamEvent::ConversationItem {
    conversation_id,
    item_id,
    cursor,
    turn_id,
    metadata,
    item,
} => {
    yield GraphqlConversationEvent::ConversationItem(
        Box::new(GraphqlConversationItemEvent {
            conversation_id,
            client_message_id,
            item_id,
            cursor,
            turn_id,
            metadata: async_graphql::Json(metadata),
            item: (*item).into(),
        }),
    );
}
```

- [ ] **Step 5: Wire schema root fields**

In `crates/noema-core/src/graphql/schema.rs`, add query methods:

```rust
async fn primary_conversation(&self, ctx: &Context<'_>) -> Result<Option<GraphqlPrimaryConversation>> {
    let state = ctx.data_unchecked::<GraphqlState>();
    chat::primary_conversation(state).await
}

async fn conversation_transcript_page(
    &self,
    ctx: &Context<'_>,
    input: GraphqlConversationTranscriptPageInput,
) -> Result<GraphqlConversationTranscriptPage> {
    let state = ctx.data_unchecked::<GraphqlState>();
    chat::conversation_transcript_page(state, input).await
}
```

Replace the mutation method `start_primary_conversation` with:

```rust
async fn ensure_primary_conversation(
    &self,
    ctx: &Context<'_>,
    cwd: Option<String>,
) -> Result<GraphqlPrimaryConversation> {
    let state = ctx.data_unchecked::<GraphqlState>();
    chat::ensure_primary_conversation(state, cwd).await
}
```

Update imports in `schema.rs` for the new GraphQL types.

- [ ] **Step 6: Run focused GraphQL tests and schema export**

Run:

```bash
cargo test -p noema-core primary_conversation_returns_identity_without_transcript --no-fail-fast
cargo test -p noema-core conversation_transcript_page_supports_latest_and_cursor_reads --no-fail-fast
cargo test -p noema-core conversation_events_emits_ready_before_live_events --no-fail-fast
```

Then run:

```bash
cargo run --manifest-path crates/noema-core/Cargo.toml --bin export_frontend_types
```

Expected: tests pass and `crates/noema-core/web/src/generated/schema.graphql` changes to include the new fields and remove `startPrimaryConversation`.

- [ ] **Step 7: Commit GraphQL API split**

Run:

```bash
git add crates/noema-core/src/daemon/web/replay.rs crates/noema-core/src/daemon/protocol.rs crates/noema-core/src/daemon/runtime/transcript_persistence.rs crates/noema-core/src/graphql/chat.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/store/conversations.rs crates/noema-core/web/src/generated/schema.graphql
git commit -m "feat: split primary conversation replay API"
```

---

### Task 3: Frontend GraphQL Operations And Transcript Window Model

**Files:**
- Modify: `crates/noema-core/web/package.json`
- Modify: `crates/noema-core/web/bun.lock`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/web/src/generated/graphql.ts`
- Modify: `crates/noema-core/web/src/shared/types.ts`
- Modify: `crates/noema-core/web/src/transcript/events.ts`
- Create: `crates/noema-core/web/src/transcript/window.ts`
- Create: `crates/noema-core/web/src/transcript/window.test.ts`

- [ ] **Step 1: Add TanStack Virtual dependency**

Run:

```bash
cd crates/noema-core/web
bun add @tanstack/react-virtual
```

Expected: `package.json` and `bun.lock` include `@tanstack/react-virtual`.

- [ ] **Step 2: Replace GraphQL operations**

In `crates/noema-core/web/src/graphql/operations.ts`, remove `StartPrimaryConversationDocument` and add:

```ts
export const ConversationItemFields = gql`
  fragment ConversationItemFields on ConversationItem {
    itemId
    cursor
    turnId
    item {
      __typename
      ... on UserText {
        text
      }
      ... on AssistantText {
        text
      }
      ... on Activity {
        id
        activityKind
        status
        title
        summary
        metadata
      }
      ... on A2UiCard {
        id
        schema
        payload
      }
      ... on ErrorNotice {
        message
        recoverable
      }
    }
  }
`;

export const PrimaryConversationDocument = gql`
  query PrimaryConversation {
    primaryConversation {
      conversationId
      provider
    }
  }
`;

export const EnsurePrimaryConversationDocument = gql`
  mutation EnsurePrimaryConversation {
    ensurePrimaryConversation {
      conversationId
      provider
    }
  }
`;

export const ConversationTranscriptPageDocument = gql`
  query ConversationTranscriptPage($input: ConversationTranscriptPageInput!) {
    conversationTranscriptPage(input: $input) {
      items {
        ...ConversationItemFields
      }
      pageInfo {
        beforeCursor
        hasMoreBefore
        limit
      }
    }
  }
  ${ConversationItemFields}
`;
```

Update `ConversationEventsDocument` so `ConversationItemEvent` requests `cursor`.

- [ ] **Step 3: Generate frontend types**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: `src/generated/graphql.ts` contains `PrimaryConversationDocument`, `EnsurePrimaryConversationDocument`, `ConversationTranscriptPageDocument`, and no generated `StartPrimaryConversationDocument`.

- [ ] **Step 4: Update transcript entry types and replay mapping**

In `crates/noema-core/web/src/shared/types.ts`, add `cursor?: string | null` to every durable transcript entry variant with `itemId`.

In `crates/noema-core/web/src/transcript/events.ts`, change replay types:

```ts
import type {
  ConversationEventsSubscription,
  ConversationTranscriptPageQuery
} from "@/generated/graphql";

type ReplayItem = ConversationTranscriptPageQuery["conversationTranscriptPage"]["items"][number];
```

Update `entryFromReplayItem`:

```ts
function entryFromReplayItem(item: ReplayItem): TranscriptEntry | null {
  const entry = entryFromConversationItem(
    item.itemId,
    item.cursor,
    item.turnId ?? undefined,
    item.item
  );
  return entry ? { ...entry, source: "replay" } : null;
}
```

Update `entryFromConversationItem` signature and all call sites:

```ts
function entryFromConversationItem(
  itemId: string,
  cursor: string | null | undefined,
  turnId: string | undefined,
  item: GraphqlTranscriptItem,
  metadata?: unknown
): TranscriptEntry | null
```

Set `cursor` on returned entries. For live events, pass `event.cursor`.

- [ ] **Step 5: Add failing transcript window tests**

Create `crates/noema-core/web/src/transcript/window.test.ts`:

```ts
import { describe, test } from "node:test";
import assert from "node:assert/strict";
import {
  appendOptimisticEntry,
  mergeDurableEntries,
  replaceOptimisticEntry,
  transcriptWindowEntries,
  type TranscriptWindowState
} from "./window";
import type { TranscriptEntry } from "@/shared/types";

function userEntry(id: string, text: string, cursor?: string): TranscriptEntry {
  return { id, itemId: id, cursor, type: "user", text };
}

describe("transcript window model", () => {
  test("merges pages in server-returned order and dedupes by item id", () => {
    const initial: TranscriptWindowState = {
      durableEntries: [],
      optimisticEntries: [],
      beforeCursor: null,
      hasMoreBefore: true
    };

    const latest = mergeDurableEntries(initial, [userEntry("item:2", "two", "c2"), userEntry("item:3", "three", "c3")], {
      beforeCursor: "c2",
      hasMoreBefore: true,
      placement: "latest"
    });
    const older = mergeDurableEntries(latest, [userEntry("item:1", "one", "c1"), userEntry("item:2", "two updated", "c2")], {
      beforeCursor: "c1",
      hasMoreBefore: false,
      placement: "before"
    });

    assert.deepEqual(
      transcriptWindowEntries(older).map((entry) => "text" in entry ? entry.text : entry.id),
      ["one", "two updated", "three"]
    );
    assert.equal(older.beforeCursor, "c1");
    assert.equal(older.hasMoreBefore, false);
  });

  test("replaces optimistic user entry with durable client item", () => {
    const optimistic = appendOptimisticEntry(
      { durableEntries: [], optimisticEntries: [], beforeCursor: null, hasMoreBefore: false },
      { id: "client:1", type: "user", text: "hello" }
    );
    const replaced = replaceOptimisticEntry(optimistic, "client:1", userEntry("item:1", "hello", "c1"));

    assert.deepEqual(
      transcriptWindowEntries(replaced).map((entry) => entry.id),
      ["client:1"]
    );
    assert.equal(transcriptWindowEntries(replaced)[0]?.itemId, "item:1");
  });

  test("keeps live item after latest replay when subscription wins the race", () => {
    const liveFirst = mergeDurableEntries(
      { durableEntries: [], optimisticEntries: [], beforeCursor: null, hasMoreBefore: false },
      [userEntry("item:3", "three", "c3")],
      {
        beforeCursor: null,
        hasMoreBefore: false,
        placement: "append"
      }
    );
    const latestArrivesSecond = mergeDurableEntries(liveFirst, [userEntry("item:1", "one", "c1"), userEntry("item:2", "two", "c2")], {
      beforeCursor: "c1",
      hasMoreBefore: false,
      placement: "latest"
    });

    assert.deepEqual(
      transcriptWindowEntries(latestArrivesSecond).map((entry) => "text" in entry ? entry.text : entry.id),
      ["one", "two", "three"]
    );
  });
});
```

- [ ] **Step 6: Run window tests and verify failure**

Run:

```bash
cd crates/noema-core/web
bun test src/transcript/window.test.ts
```

Expected: module-not-found failure for `./window`.

- [ ] **Step 7: Implement transcript window model**

Create `crates/noema-core/web/src/transcript/window.ts`:

```ts
import type { TranscriptEntry } from "@/shared/types";

export type TranscriptPagePlacement = "latest" | "before" | "append";

export type TranscriptWindowState = {
  durableEntries: TranscriptEntry[];
  optimisticEntries: TranscriptEntry[];
  beforeCursor: string | null;
  hasMoreBefore: boolean;
};

export function emptyTranscriptWindow(): TranscriptWindowState {
  return {
    durableEntries: [],
    optimisticEntries: [],
    beforeCursor: null,
    hasMoreBefore: false
  };
}

export function transcriptWindowEntries(state: TranscriptWindowState): TranscriptEntry[] {
  return [...state.durableEntries, ...state.optimisticEntries];
}

export function appendOptimisticEntry(
  state: TranscriptWindowState,
  entry: TranscriptEntry
): TranscriptWindowState {
  return {
    ...state,
    optimisticEntries: [...state.optimisticEntries, entry]
  };
}

export function replaceOptimisticEntry(
  state: TranscriptWindowState,
  optimisticId: string,
  durableEntry: TranscriptEntry
): TranscriptWindowState {
  const optimisticIndex = state.optimisticEntries.findIndex((entry) => entry.id === optimisticId);
  const replacement = { ...durableEntry, id: optimisticId };
  if (optimisticIndex === -1) {
    return mergeDurableEntries(state, [durableEntry], {
      placement: "append",
      beforeCursor: state.beforeCursor,
      hasMoreBefore: state.hasMoreBefore
    });
  }
  return {
    ...state,
    durableEntries: mergeEntriesByItemId(state.durableEntries, [replacement], "append"),
    optimisticEntries: state.optimisticEntries.filter((_, index) => index !== optimisticIndex)
  };
}

export function mergeDurableEntries(
  state: TranscriptWindowState,
  entries: TranscriptEntry[],
  page: {
    placement: TranscriptPagePlacement;
    beforeCursor: string | null;
    hasMoreBefore: boolean;
  }
): TranscriptWindowState {
  const durableEntries = mergeEntriesByItemId(state.durableEntries, entries, page.placement);
  return {
    durableEntries,
    optimisticEntries: removeOptimisticEntriesWithDurableMatches(state.optimisticEntries, durableEntries),
    beforeCursor: page.beforeCursor,
    hasMoreBefore: page.hasMoreBefore
  };
}

function mergeEntriesByItemId(
  current: TranscriptEntry[],
  incoming: TranscriptEntry[],
  placement: TranscriptPagePlacement
): TranscriptEntry[] {
  const next = placement === "append" ? [...current, ...incoming] : [...incoming, ...current];
  const byItemId = new Map<string, TranscriptEntry>();
  const itemOrder: string[] = [];
  const entriesWithoutItemId: TranscriptEntry[] = [];
  for (const entry of next) {
    const itemId = "itemId" in entry ? entry.itemId : undefined;
    if (!itemId) {
      entriesWithoutItemId.push(entry);
      continue;
    }
    if (!byItemId.has(itemId)) {
      itemOrder.push(itemId);
    }
    byItemId.set(itemId, entry);
  }
  return [...itemOrder.map((itemId) => byItemId.get(itemId)).filter((entry): entry is TranscriptEntry => Boolean(entry)), ...entriesWithoutItemId];
}

function removeOptimisticEntriesWithDurableMatches(
  optimisticEntries: TranscriptEntry[],
  durableEntries: TranscriptEntry[]
): TranscriptEntry[] {
  const durableItemIds = new Set(
    durableEntries
      .map((entry) => ("itemId" in entry ? entry.itemId : undefined))
      .filter((itemId): itemId is string => Boolean(itemId))
  );
  return optimisticEntries.filter((entry) => {
    const itemId = "itemId" in entry ? entry.itemId : undefined;
    return !itemId || !durableItemIds.has(itemId);
  });
}
```

- [ ] **Step 8: Run frontend model tests**

Run:

```bash
cd crates/noema-core/web
bun test src/transcript/window.test.ts
bun test src/components/transcript/renderModel.test.ts
```

Expected: tests pass.

- [ ] **Step 9: Commit frontend operations and window model**

Run:

```bash
git add crates/noema-core/web/package.json crates/noema-core/web/bun.lock crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/graphql.ts crates/noema-core/web/src/shared/types.ts crates/noema-core/web/src/transcript/events.ts crates/noema-core/web/src/transcript/window.ts crates/noema-core/web/src/transcript/window.test.ts
git commit -m "feat(web): add transcript pagination client model"
```

---

### Task 4: Wire Chat Boot And Transparent Page Loading

**Files:**
- Modify: `crates/noema-core/web/src/app/App.tsx`
- Modify: `crates/noema-core/web/src/components/ChatSurface.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/Transcript.tsx`

- [ ] **Step 1: Replace `startPrimaryConversation` boot state with split operations**

In `crates/noema-core/web/src/app/App.tsx`, replace imports:

```ts
import {
  ConversationEventsDocument,
  ConversationTranscriptPageDocument,
  EnsurePrimaryConversationDocument,
  LocalStatusDocument,
  OnboardingStatusDocument,
  PrimaryConversationDocument,
  ProviderAuthAttemptDocument,
  SendConversationTurnDocument,
  StartProviderAuthAttemptDocument,
  type ProviderAuthAttemptQuery,
  type StartProviderAuthAttemptMutation
} from "@/generated/graphql";
```

Import the window model:

```ts
import {
  appendOptimisticEntry,
  emptyTranscriptWindow,
  mergeDurableEntries,
  replaceOptimisticEntry,
  transcriptWindowEntries
} from "@/transcript/window";
```

Replace `const [startPrimaryConversation] = useMutation(StartPrimaryConversationDocument);` with:

```ts
const primaryConversation = useQuery(PrimaryConversationDocument, {
  skip: !chatRoute || !onboarded,
  fetchPolicy: "network-only"
});
const [ensurePrimaryConversation] = useMutation(EnsurePrimaryConversationDocument);
```

Replace transcript state:

```ts
const [transcriptWindow, setTranscriptWindow] = React.useState(() => emptyTranscriptWindow());
const transcript = transcriptWindowEntries(transcriptWindow);
const [loadingLatestTranscript, setLoadingLatestTranscript] = React.useState(false);
const [loadingOlderTranscript, setLoadingOlderTranscript] = React.useState(false);
const [transcriptPageError, setTranscriptPageError] = React.useState<string | null>(null);
```

- [ ] **Step 2: Load or ensure the primary conversation**

Replace the current `React.useEffect` that calls `startPrimaryConversation` with this flow:

```ts
React.useEffect(() => {
  if (!chatRoute || !onboarded || conversationId || startingConversationRef.current) {
    return;
  }

  const existing = primaryConversation.data?.primaryConversation;
  if (existing) {
    setConversationId(existing.conversationId);
    setSocketState("ready");
    setAgentStatus("IDLE");
    return;
  }

  if (primaryConversation.loading) {
    setSocketState("connecting");
    setAgentStatus("connecting");
    return;
  }

  if (primaryConversation.error) {
    reportConversationError(primaryConversation.error);
    return;
  }

  startingConversationRef.current = true;
  setSocketState("connecting");
  setAgentStatus("connecting");
  void ensurePrimaryConversation()
    .then((result) => {
      const ensured = result.data?.ensurePrimaryConversation;
      if (!ensured) {
        throw new Error("Noema did not return a conversation.");
      }
      setConversationId(ensured.conversationId);
      setPending(false);
      setAwaitingAssistantTurn(false);
      setSocketState("ready");
      setAgentStatus("IDLE");
    })
    .catch((error: unknown) => {
      setSocketState("closed");
      setAgentStatus("closed");
      setPending(false);
      setAwaitingAssistantTurn(false);
      pushTranscriptWindowError(error instanceof Error ? error.message : "Noema could not start chat.");
    })
    .finally(() => {
      startingConversationRef.current = false;
    });
}, [
  chatRoute,
  conversationId,
  ensurePrimaryConversation,
  onboarded,
  primaryConversation.data,
  primaryConversation.error,
  primaryConversation.loading,
  reportConversationError
]);
```

Add `pushTranscriptWindowError` as a local callback that keeps existing error notice behavior visible in the transcript:

```ts
const pushTranscriptWindowError = React.useCallback((message: string) => {
  setTranscriptWindow((current) =>
    mergeDurableEntries(
      current,
      [
        {
          id: createClientId(),
          type: "error",
          message,
          recoverable: true
        }
      ],
      {
        placement: "append",
        beforeCursor: current.beforeCursor,
        hasMoreBefore: current.hasMoreBefore
      }
    )
  );
}, []);
```

- [ ] **Step 3: Load latest and older transcript pages**

Add a reusable page loader in `App.tsx`:

```ts
const loadConversationTranscriptPage = React.useCallback(
  async ({ cursor, placement }: { cursor: string | null; placement: "latest" | "before" }) => {
    if (!conversationId) {
      return;
    }
    if (placement === "latest") {
      setLoadingLatestTranscript(true);
    } else {
      setLoadingOlderTranscript(true);
    }
    setTranscriptPageError(null);
    try {
      const result = await apolloClient.query({
        query: ConversationTranscriptPageDocument,
        variables: {
          input: {
            conversationId,
            cursor,
            limit: 80
          }
        },
        fetchPolicy: "network-only"
      });
      const page = result.data.conversationTranscriptPage;
      setTranscriptWindow((current) =>
        mergeDurableEntries(current, entriesFromReplay(page.items), {
          placement,
          beforeCursor: page.pageInfo.beforeCursor ?? null,
          hasMoreBefore: page.pageInfo.hasMoreBefore
        })
      );
    } catch (error: unknown) {
      setTranscriptPageError(error instanceof Error ? error.message : "Noema could not load chat history.");
    } finally {
      if (placement === "latest") {
        setLoadingLatestTranscript(false);
      } else {
        setLoadingOlderTranscript(false);
      }
    }
  },
  [apolloClient, conversationId]
);
```

Add effects:

```ts
React.useEffect(() => {
  if (!conversationId || loadingLatestTranscript || transcriptWindow.durableEntries.length > 0) {
    return;
  }
  void loadConversationTranscriptPage({ cursor: null, placement: "latest" });
}, [
  conversationId,
  loadConversationTranscriptPage,
  loadingLatestTranscript,
  transcriptWindow.durableEntries.length
]);

const loadOlderTranscript = React.useCallback(() => {
  if (!transcriptWindow.hasMoreBefore || loadingOlderTranscript || !transcriptWindow.beforeCursor) {
    return;
  }
  void loadConversationTranscriptPage({
    cursor: transcriptWindow.beforeCursor,
    placement: "before"
  });
}, [
  loadConversationTranscriptPage,
  loadingOlderTranscript,
  transcriptWindow.beforeCursor,
  transcriptWindow.hasMoreBefore
]);
```

- [ ] **Step 4: Route live events through pure event helpers**

Refactor `crates/noema-core/web/src/transcript/events.ts` so event interpretation is separate from React state setters. Export these helpers:

```ts
export function entryFromConversationEvent(event: ConversationEvent): TranscriptEntry | null
export function isTurnCompletedEvent(event: ConversationEvent): boolean
export function isAgentStatusEvent(event: ConversationEvent): event is Extract<ConversationEvent, { __typename: "AgentStatusEvent" }>
export function isAssistantTextDeltaEvent(event: ConversationEvent): event is Extract<ConversationEvent, { __typename: "AssistantTextDeltaEvent" }>
export function removeStaleStartedMemoryExtractions(current: TranscriptEntry[]): TranscriptEntry[]
export function appendAssistantTextDeltaEntry(current: TranscriptEntry[], event: { turnId: string; streamId: string; delta: string; conversationId: string }): TranscriptEntry[]
```

In the subscription effect in `App.tsx`, use those helpers:

```ts
const event = conversationEvents.data?.conversationEvents;
if (!event) {
  return;
}
if (isTurnCompletedEvent(event)) {
  setTranscriptWindow((current) => ({
    ...current,
    durableEntries: removeStaleStartedMemoryExtractions(current.durableEntries)
  }));
  setPending(false);
  setAwaitingAssistantTurn(false);
  setAgentStatus("IDLE");
  return;
}
if (isAgentStatusEvent(event)) {
  setAgentStatus(event.status);
  return;
}
if (isAssistantTextDeltaEvent(event)) {
  setAwaitingAssistantTurn(false);
  setTranscriptWindow((current) => ({
    ...current,
    durableEntries: appendAssistantTextDeltaEntry(current.durableEntries, {
      conversationId: event.conversationId,
      turnId: event.deltaTurnId,
      streamId: event.streamId,
      delta: event.delta
    })
  }));
  return;
}
const entry = entryFromConversationEvent(event);
if (!entry) {
  return;
}
if (event.__typename === "ConversationItemEvent" && event.clientMessageId && entry.type === "user") {
  setTranscriptWindow((current) => replaceOptimisticEntry(current, event.clientMessageId!, entry));
} else {
  setTranscriptWindow((current) =>
    mergeDurableEntries(current, [entry], {
      placement: "append",
      beforeCursor: current.beforeCursor,
      hasMoreBefore: current.hasMoreBefore
    })
  );
}
```

- [ ] **Step 5: Pass pagination state to chat and transcript components**

In `ChatSurfaceProps`, add:

```ts
loadingOlderTranscript: boolean;
hasMoreTranscriptBefore: boolean;
transcriptPageError: string | null;
onLoadOlderTranscript: () => void;
```

Pass those props from `App.tsx`:

```tsx
<ChatSurface
  transcript={transcript}
  loadingOlderTranscript={loadingOlderTranscript}
  hasMoreTranscriptBefore={transcriptWindow.hasMoreBefore}
  transcriptPageError={transcriptPageError}
  onLoadOlderTranscript={loadOlderTranscript}
  ...
/>
```

Pass them from `ChatSurface` to `Transcript`.

- [ ] **Step 6: Run frontend typecheck and focused tests**

Run:

```bash
cd crates/noema-core/web
bun test src/transcript/window.test.ts
bun run lint
```

Expected: tests and lint pass.

- [ ] **Step 7: Commit chat boot pagination wiring**

Run:

```bash
git add crates/noema-core/web/src/app/App.tsx crates/noema-core/web/src/components/ChatSurface.tsx crates/noema-core/web/src/components/transcript/Transcript.tsx crates/noema-core/web/src/transcript/events.ts crates/noema-core/web/src/transcript/window.ts crates/noema-core/web/src/transcript/window.test.ts
git commit -m "feat(web): load chat transcript pages"
```

---

### Task 5: Virtualize Transcript Rendering

**Files:**
- Modify: `crates/noema-core/web/src/components/transcript/Transcript.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/TranscriptScroller.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/TranscriptBottomFollower.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/scrollModel.ts`

- [ ] **Step 1: Refactor `Transcript` to pass rendered row data into `TranscriptScroller`**

In `Transcript.tsx`, add props:

```ts
loadingOlderTranscript: boolean;
hasMoreTranscriptBefore: boolean;
transcriptPageError: string | null;
onLoadOlderTranscript: () => void;
```

Replace the children-based `TranscriptScroller` call with a render prop:

```tsx
<TranscriptScroller
  aria-label="Conversation transcript"
  entries={renderedEntries}
  hasMoreBefore={hasMoreTranscriptBefore}
  loadingBefore={loadingOlderTranscript}
  loadBeforeError={transcriptPageError}
  onLoadBefore={onLoadOlderTranscript}
  onViewportScroll={handleViewportScroll}
  renderEntry={(entry, index) => {
    const lane = transcriptLane(entry);
    const previousEntry = renderedEntries[index - 1];
    const previousLane = previousEntry ? transcriptLane(previousEntry) : null;
    const showAvatar = previousLane !== lane;
    const messageId = renderedEntryMessageId(entry);
    const animateArrival = shouldAnimateRenderedEntryArrivalForSeen(entry, messageId, seenArrivalMessageIds);
    const animateText = shouldAnimateRenderedEntryTextForSeen(
      entry,
      messageId,
      seenArrivalMessageIds,
      textAnimatingMessageIds
    );

    return (
      <TranscriptScrollerItem
        align={lane === "human" ? "end" : "start"}
        compact={shouldCompactMarkerClusterSpacing(entry, previousEntry)}
        data-arrival={animateArrival ? "true" : undefined}
        messageId={messageId}
        scrollAnchor={shouldAnchorRenderedEntry(entry)}
      >
        <RenderedTranscriptEntryFrame lane={lane}>
          {renderTranscriptRenderEntry(entry, expandedActivities, onToggleActivity, showAvatar, animateText)}
        </RenderedTranscriptEntryFrame>
      </TranscriptScrollerItem>
    );
  }}
/>
```

- [ ] **Step 2: Implement TanStack Virtual scroller**

In `TranscriptScroller.tsx`, import:

```ts
import { useVirtualizer } from "@tanstack/react-virtual";
import type { RenderTranscriptEntry } from "./renderModel";
import { renderedEntryMessageId } from "./renderModel";
```

Change props:

```ts
type TranscriptScrollerProps = {
  entries: RenderTranscriptEntry[];
  hasMoreBefore: boolean;
  loadingBefore: boolean;
  loadBeforeError: string | null;
  renderEntry: (entry: RenderTranscriptEntry, index: number) => React.ReactNode;
  onLoadBefore: () => void;
  "aria-label"?: string;
  onViewportScroll?: React.UIEventHandler<HTMLDivElement>;
};
```

Use the virtualizer:

```ts
const rowVirtualizer = useVirtualizer({
  count: entries.length,
  getScrollElement: () => viewportRef.current,
  estimateSize: () => 96,
  overscan: 8,
  getItemKey: (index) => renderedEntryMessageId(entries[index])
});
const virtualItems = rowVirtualizer.getVirtualItems();
const totalSize = rowVirtualizer.getTotalSize();
```

Replace the flex column content with a measured virtual content area:

```tsx
<div ref={contentRef} {...stylex.props(styles.content)}>
  <div
    {...stylex.props(styles.virtualSizer)}
    style={{ height: `${totalSize}px` }}
  >
    {virtualItems.map((virtualItem) => {
      const entry = entries[virtualItem.index];
      return (
        <div
          key={virtualItem.key}
          ref={rowVirtualizer.measureElement}
          data-index={virtualItem.index}
          {...stylex.props(styles.virtualRow)}
          style={{ transform: `translateY(${virtualItem.start}px)` }}
        >
          {renderEntry(entry, virtualItem.index)}
        </div>
      );
    })}
  </div>
</div>
```

Add styles:

```ts
virtualSizer: {
  position: "relative",
  width: "100%",
  minHeight: "100%"
},
virtualRow: {
  position: "absolute",
  top: 0,
  left: 0,
  width: "100%"
}
```

Keep `styles.content` width, margins, padding, and bottom composer padding. Remove `display: "flex"`, `flexDirection`, `justifyContent`, and `gap`; row spacing remains on `TranscriptScrollerItem` via margin. Add `marginTop: 12` to `styles.item`, with `compact` overriding to `marginTop: 4`.

- [ ] **Step 3: Trigger older-page loading near the top**

In `TranscriptScroller`, add an effect:

```ts
React.useEffect(() => {
  const first = virtualItems[0];
  if (!first || !hasMoreBefore || loadingBefore) {
    return;
  }
  if (first.index <= 3) {
    onLoadBefore();
  }
}, [hasMoreBefore, loadingBefore, onLoadBefore, virtualItems]);
```

Add a top status row rendered above virtual rows when `hasMoreBefore`, `loadingBefore`, or `loadBeforeError` is true:

```tsx
{(hasMoreBefore || loadingBefore || loadBeforeError) ? (
  <div {...stylex.props(styles.loadBeforeStatus)} role={loadBeforeError ? "alert" : "status"}>
    {loadBeforeError ? (
      <button type="button" {...stylex.props(styles.loadBeforeButton)} onClick={onLoadBefore}>
        Retry loading earlier messages
      </button>
    ) : loadingBefore ? (
      "Loading earlier messages"
    ) : null}
  </div>
) : null}
```

Use small, unobtrusive text. This is not an explanatory feature block; it is a status/control.

- [ ] **Step 4: Keep bottom-following compatible with virtual rows**

Keep `TranscriptBottomFollower` using `viewportRef` and `contentRef`. Because virtual total height changes when rows measure, keep the existing `ResizeObserver` on `contentRef` and `viewportRef`.

Update `scrollToEnd` in `TranscriptScrollerProvider` to use the viewport scroll height:

```ts
viewport.scrollTo({ top: viewport.scrollHeight, behavior });
```

This already matches current behavior. If TanStack measurement causes a one-frame short scroll, keep the existing delayed resize sync in `TranscriptBottomFollower`; do not add a second animation loop.

- [ ] **Step 5: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun test src/transcript/window.test.ts
bun test src/components/transcript/renderModel.test.ts
bun run lint
bun run build
```

Expected: all pass. Do not run browser inspection unless explicitly requested.

- [ ] **Step 6: Commit virtualization**

Run:

```bash
git add crates/noema-core/web/src/components/transcript/Transcript.tsx crates/noema-core/web/src/components/transcript/TranscriptScroller.tsx crates/noema-core/web/src/components/transcript/TranscriptBottomFollower.tsx crates/noema-core/web/src/components/transcript/scrollModel.ts
git commit -m "feat(web): virtualize chat transcript"
```

---

### Task 6: Final Validation And Context Update

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Run required status and diff checks**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: no whitespace errors. Any remaining unstaged files should be listed and classified as either part of this feature or unrelated user work.

- [ ] **Step 2: Run backend validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all pass. Do not modify `CARGO_BUILD_RUSTC_WRAPPER` and do not interfere with `sccache`.

- [ ] **Step 3: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: all pass.

- [ ] **Step 4: Update durable context**

Add a concise bullet to `docs/context/current.md` under Settled Decisions:

```md
- The web chat transcript loads durable history through paged GraphQL replay:
  `primaryConversation` reads identity, `ensurePrimaryConversation` creates the
  rare missing primary conversation, `conversationTranscriptPage` returns
  cursor-based visible item windows, and the frontend renders loaded transcript
  rows through TanStack Virtual while keeping live updates on
  `conversationEvents`.
```

- [ ] **Step 5: Commit context update**

Run:

```bash
git add docs/context/current.md
git commit -m "docs: record transcript pagination context"
```

- [ ] **Step 6: Inspect final staged state if any and report leftovers**

Run:

```bash
git status --short --branch
git diff --cached --stat
git diff --cached --name-status
```

Expected: no staged changes unless the user asked to stage more. Report any untracked or unstaged files, especially unrelated files that were present before implementation.
