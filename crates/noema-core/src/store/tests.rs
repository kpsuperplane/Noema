use serde_json::json;
use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};
use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    NewConversationTurn, ObjectRef, ProviderAccountStatus, ReplayMode, StoreError,
    memory_persistence::NewConversation,
};

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
async fn embedded_store_config_is_stable_for_reopen() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    assert_eq!(config.path, paths.db_dir());
    assert_eq!(StoreConfig::from_paths(&paths), config);
}

#[tokio::test]
async fn strict_schema_rejects_invalid_sensitivity() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::thing('claims', 'invalid_sensitivity') SET
              claim_id = 'claim:invalid-sensitivity',
              subject_entity_id = 'entity:human-local',
              predicate_id = 'likes',
              fact = 'invalid sensitivity test claim',
              status = 'candidate',
              sensitivity = 'galaxy',
              retrieval_hints = {},
              policy_overrides = {},
              metadata = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("invalid claim query")
        .check()
        .expect_err("invalid sensitivity should be rejected");

    assert!(
        error.to_string().contains("sensitivity") || error.to_string().contains("galaxy"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn built_in_personal_predicates_are_seeded() {
    #[derive(Debug, serde::Deserialize)]
    struct PredicateRow {
        label: String,
        default_sensitivity: String,
    }

    let store = test_store().await;
    let mut response = store
        .db()
        .query(
            r#"
            SELECT label, default_sensitivity
            FROM predicates
            WHERE predicate_id = 'likes'
            LIMIT 1;
            "#,
        )
        .await
        .expect("select predicate");
    let rows: Vec<PredicateRow> = response.take(0).expect("predicate rows");
    let likes = rows.first().expect("likes predicate");

    assert_eq!(likes.label, "likes");
    assert_eq!(likes.default_sensitivity, "normal");
}

#[tokio::test]
async fn default_provider_account_round_trips_status() {
    let store = test_store().await;

    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default account");
    assert_eq!(
        account.provider_account_id,
        "provider_account:codex:default"
    );
    assert_eq!(account.status, ProviderAccountStatus::Unknown);

    store
        .update_provider_account_status(
            &account.provider_account_id,
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("update status");

    let fetched = store
        .active_provider_account("codex")
        .await
        .expect("active account")
        .expect("account exists");
    assert_eq!(fetched.provider_account_id, account.provider_account_id);
    assert_eq!(fetched.status, ProviderAccountStatus::Authenticated);
}

#[tokio::test]
async fn primary_conversation_reuses_existing_home_conversation() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");

    let first = store
        .get_or_create_primary_conversation("human:local", Some("gpt-test".to_string()), None)
        .await
        .expect("first primary conversation");
    let second = store
        .get_or_create_primary_conversation("human:local", Some("other-model".to_string()), None)
        .await
        .expect("second primary conversation");

    assert_eq!(first.conversation_id, second.conversation_id);
}

#[tokio::test]
async fn primary_conversation_ignores_pointer_to_other_human_conversation() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let mut other_human_conversation = NewConversation::local_chat(None, None);
    other_human_conversation.owner = ObjectRef::human("human:other");
    other_human_conversation.primary_human_id = Some("human:other".to_string());
    let other = store
        .create_conversation(other_human_conversation)
        .await
        .expect("other human conversation");
    store
        .db()
        .query(
            r#"
            UPDATE humans SET
              primary_conversation_id = $conversation_id,
              updated_at = time::now()
            WHERE human_id = 'human:local';
            "#,
        )
        .bind(("conversation_id", other.conversation_id.clone()))
        .await
        .expect("update human pointer")
        .check()
        .expect("checked pointer update");

    let local = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("local primary conversation");
    let local_again = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("local primary conversation again");

    assert_ne!(local.conversation_id, other.conversation_id);
    assert_eq!(local.conversation_id, local_again.conversation_id);
}

#[tokio::test]
async fn conversation_items_replay_in_append_order() {
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

    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("first".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("first item");
    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary"),
            content_text: Some("second".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("second item");

    let items = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("items");
    let content = items
        .into_iter()
        .map(|item| item.content_text)
        .collect::<Vec<_>>();

    assert_eq!(
        content,
        vec![Some("first".to_string()), Some("second".to_string())]
    );
}

#[tokio::test]
async fn append_conversation_item_rejects_cross_conversation_turn() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let first = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("first conversation");
    let second = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("second conversation");
    let first_turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: first.conversation_id,
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("first turn");

    let error = store
        .append_conversation_item(NewConversationItem {
            conversation_id: second.conversation_id.clone(),
            turn_id: Some(first_turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("wrong turn".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect_err("cross-conversation turn should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationTurnConversationMismatch {
            turn_id,
            conversation_id,
        } if turn_id == first_turn.turn_id && conversation_id == second.conversation_id
    ));
}

#[tokio::test]
async fn create_conversation_turn_rejects_cross_conversation_trigger_item() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let first = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("first conversation");
    let second = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("second conversation");
    let first_item = store
        .append_conversation_item(NewConversationItem {
            conversation_id: first.conversation_id,
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("wrong trigger".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("first conversation item");

    let error = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: second.conversation_id.clone(),
            trigger_item_id: Some(first_item.item_id.clone()),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect_err("cross-conversation trigger item should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationItemConversationMismatch {
            item_id,
            conversation_id,
        } if item_id == first_item.item_id && conversation_id == second.conversation_id
    ));
}

#[tokio::test]
async fn append_conversation_item_rejects_cross_conversation_parent() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let first = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("first conversation");
    let second = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("second conversation");
    let first_turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: first.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("first turn");
    let second_turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: second.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("second turn");
    let first_item = store
        .append_conversation_item(NewConversationItem {
            conversation_id: first.conversation_id,
            turn_id: Some(first_turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("first parent".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("first item");

    let error = store
        .append_conversation_item(NewConversationItem {
            conversation_id: second.conversation_id.clone(),
            turn_id: Some(second_turn.turn_id),
            parent_item_id: Some(first_item.item_id.clone()),
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary"),
            content_text: Some("wrong parent".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect_err("cross-conversation parent should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationItemConversationMismatch {
            item_id,
            conversation_id,
        } if item_id == first_item.item_id && conversation_id == second.conversation_id
    ));
}

async fn test_store() -> NoemaStore {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let store = NoemaStore::open(&config).await.expect("open store");
    std::mem::forget(home);
    store
}
