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
        author: noema_conversations::ActorRef::new("system:task-runtime")
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

#[tokio::test]
async fn mcp_setup_tool_result_remains_pending_until_exact_resolution() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let item = store
        .append_conversation_item(noema_conversations::NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: noema_conversations::ConversationItemKind::Activity,
            status: noema_conversations::ConversationItemStatus::Completed,
            author: noema_conversations::ActorRef::new("agent:primary").expect("agent"),
            content_text: None,
            payload_json: serde_json::json!({"metadata": {"action": {
                "name": "mcp.connect_service",
                "success": true,
                "payload": {"status": "needs_auth"}
            }}}),
            metadata: serde_json::json!({"source": "provider_action"}),
        })
        .await
        .expect("setup item");

    assert_eq!(
        store
            .list_pending_mcp_setup_items(&conversation.conversation_id, 10)
            .await
            .expect("pending")
            .len(),
        1
    );
    assert!(
        store
            .resolve_mcp_setup_item(&conversation.conversation_id, &item.item_id, "mcp:notion")
            .await
            .expect("resolve")
    );
    assert!(
        store
            .list_pending_mcp_setup_items(&conversation.conversation_id, 10)
            .await
            .expect("resolved list")
            .is_empty()
    );
}

#[tokio::test]
async fn memory_source_range_captures_one_conversation_head_and_resumes_after_it() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");

    let append = |kind, author, text: &str| noema_conversations::NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: None,
        parent_item_id: None,
        kind,
        status: noema_conversations::ConversationItemStatus::Completed,
        author,
        content_text: Some(text.to_string()),
        payload_json: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };
    store
        .append_conversation_item(append(
            noema_conversations::ConversationItemKind::UserText,
            noema_conversations::ActorRef::new("human:local").expect("human actor"),
            "first",
        ))
        .await
        .expect("first item");
    store
        .append_conversation_item(append(
            noema_conversations::ConversationItemKind::AssistantText,
            noema_conversations::ActorRef::new("agent:primary").expect("agent actor"),
            "context",
        ))
        .await
        .expect("second item");

    let first = store
        .capture_memory_source_range(&conversation.conversation_id, 0)
        .await
        .expect("first source range");
    assert_eq!(first.captured_head_sequence, 2);
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.sequence_index)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    store
        .append_conversation_item(append(
            noema_conversations::ConversationItemKind::UserText,
            noema_conversations::ActorRef::new("human:local").expect("human actor"),
            "later",
        ))
        .await
        .expect("later item");
    let resumed = store
        .capture_memory_source_range(&conversation.conversation_id, first.captured_head_sequence)
        .await
        .expect("resumed source range");

    assert_eq!(resumed.captured_head_sequence, 3);
    assert_eq!(resumed.items.len(), 1);
    assert_eq!(resumed.items[0].content_text.as_deref(), Some("later"));
}
