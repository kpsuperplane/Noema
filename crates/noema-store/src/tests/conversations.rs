use super::test_store;

#[tokio::test]
async fn idempotent_conversation_item_id_prevents_duplicate_task_delivery() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let item = || noema_conversations::NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: None,
        parent_item_id: None,
        kind: noema_conversations::ConversationItemKind::TaskReference,
        status: noema_conversations::ConversationItemStatus::Completed,
        author: noema_conversations::ActorRef::system("system:task-runtime")
            .expect("static task runtime actor id must be valid"),
        content_text: Some("Task update".to_string()),
        payload_json: serde_json::json!({"task_id": "task:1", "status": "completed"}),
        metadata: serde_json::json!({"task_event_id": "event:1"}),
    };

    let first = store
        .append_conversation_item_with_id("item:task_status:event:1".to_string(), item())
        .await
        .expect("first delivery");
    let second = store
        .append_conversation_item_with_id("item:task_status:event:1".to_string(), item())
        .await
        .expect("idempotent delivery");
    let rows = store
        .list_conversation_items(
            &conversation.conversation_id,
            noema_conversations::ReplayMode::Audit,
        )
        .await
        .expect("conversation items");

    assert_eq!(first.item_id, second.item_id);
    assert_eq!(rows.len(), 1);
}
