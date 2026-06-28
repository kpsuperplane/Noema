use serde_json::json;
use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};
use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    NewConversationTurn, ProviderAccountStatus, ReplayMode, memory_persistence::NewConversation,
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

async fn test_store() -> NoemaStore {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let store = NoemaStore::open(&config).await.expect("open store");
    std::mem::forget(home);
    store
}
