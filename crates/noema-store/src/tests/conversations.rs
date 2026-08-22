use super::test_store;

#[tokio::test]
async fn provider_assistant_text_shares_one_row_and_omits_equal_source_text() {
    use noema_conversations::{
        ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    };

    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let new_item = |text: &str| NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: None,
        parent_item_id: None,
        kind: ConversationItemKind::AssistantText,
        status: ConversationItemStatus::Completed,
        author: ActorRef::new("agent:primary").expect("agent"),
        content_text: Some(text.to_string()),
        payload_json: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };

    let equal = store
        .append_provider_conversation_item(new_item("Same text"), "Same text".to_string())
        .await
        .expect("equal item");
    let projected = store
        .append_provider_conversation_item(
            new_item("Projected text"),
            "Projected text \u{e200}cite\u{e202}turn0search0\u{e201}".to_string(),
        )
        .await
        .expect("projected item");

    assert_eq!(
        store
            .stored_provider_content_text(&equal.item_id)
            .await
            .expect("equal provider text"),
        None
    );
    assert_eq!(
        store
            .stored_provider_content_text(&projected.item_id)
            .await
            .expect("projected provider text")
            .as_deref(),
        Some("Projected text \u{e200}cite\u{e202}turn0search0\u{e201}")
    );
    assert_eq!(projected.content_text.as_deref(), Some("Projected text"));
}

#[tokio::test]
async fn primary_notification_writes_resume_after_a_partial_save() {
    use noema_conversations::{
        ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
        NewConversationTurn, ReplayMode,
    };

    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let turn_id = "turn:notification:test".to_string();
    store
        .create_conversation_turn_with_id_if_absent(
            turn_id.clone(),
            NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({"turn_index": 1, "notification_id": "test"}),
            },
        )
        .await
        .expect("turn");
    let item = |index, text: &str, phase: &str| NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: Some(turn_id.clone()),
        parent_item_id: None,
        kind: ConversationItemKind::AssistantText,
        status: ConversationItemStatus::Completed,
        author: ActorRef::new("agent:primary").expect("agent"),
        content_text: Some(text.to_string()),
        payload_json: serde_json::json!({}),
        metadata: serde_json::json!({
            "source": "task_notification",
            "notification_id": "test",
            "response_index": index,
            "phase": phase,
        }),
    };

    let (_, first_inserted) = store
        .append_provider_conversation_item_with_id_if_absent(
            "item:assistant:notification:test:0".to_string(),
            item(0, "Progress", "commentary"),
            "Progress".to_string(),
        )
        .await
        .expect("first item");
    let (_, repeated_inserted) = store
        .append_provider_conversation_item_with_id_if_absent(
            "item:assistant:notification:test:0".to_string(),
            item(0, "Progress", "commentary"),
            "Progress".to_string(),
        )
        .await
        .expect("repeated first item");
    let (_, second_inserted) = store
        .append_provider_conversation_item_with_id_if_absent(
            "item:assistant:notification:test:1".to_string(),
            item(1, "Done", "final_answer"),
            "Done".to_string(),
        )
        .await
        .expect("second item");

    assert!(first_inserted);
    assert!(!repeated_inserted);
    assert!(second_inserted);
    let items = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].metadata["phase"], "commentary");
    assert_eq!(items[1].metadata["phase"], "final_answer");
}

#[tokio::test]
async fn conversation_working_directory_is_allocated_and_persisted() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");

    let first = store
        .conversation_working_directory(&conversation.conversation_id, None)
        .await
        .expect("allocate working directory");
    let second = store
        .conversation_working_directory(&conversation.conversation_id, None)
        .await
        .expect("reload working directory");

    assert!(first.is_absolute());
    assert!(first.is_dir());
    assert_eq!(first, second);
    assert!(first.ends_with(format!("conversations/{}", conversation.conversation_id)));
}

#[tokio::test]
async fn explicit_conversation_working_directory_replaces_default() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let explicit = tempfile::tempdir().expect("explicit directory");

    let selected = store
        .conversation_working_directory(
            &conversation.conversation_id,
            Some(&explicit.path().to_string_lossy()),
        )
        .await
        .expect("select working directory");
    let reloaded = store
        .conversation_working_directory(&conversation.conversation_id, None)
        .await
        .expect("reload working directory");

    assert_eq!(
        selected,
        std::path::absolute(explicit.path()).expect("absolute")
    );
    assert_eq!(selected, reloaded);
}

#[tokio::test]
async fn final_tool_result_finishes_exact_call_and_repeats_without_a_duplicate() {
    use noema_conversations::{
        ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem, ReplayMode,
    };

    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let author = ActorRef::new("agent:primary").expect("agent");

    for (case, status) in [
        ("success", ConversationItemStatus::Completed),
        ("failure", ConversationItemStatus::Failed),
        ("decline", ConversationItemStatus::Cancelled),
        ("interruption", ConversationItemStatus::Interrupted),
    ] {
        let call = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: None,
                parent_item_id: None,
                kind: ConversationItemKind::ToolCall,
                status: ConversationItemStatus::Running,
                author: author.clone(),
                content_text: Some(format!("Call {case}")),
                payload_json: serde_json::json!({"case": case}),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("call");
        let result = NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: Some(call.item_id.clone()),
            kind: ConversationItemKind::ToolResult,
            status,
            author: author.clone(),
            content_text: Some(format!("Result {case}")),
            payload_json: serde_json::json!({"case": case}),
            metadata: serde_json::json!({}),
        };

        let (first, inserted) = store
            .finish_conversation_tool_call(&call.item_id, result.clone())
            .await
            .expect("first result");
        let (repeat, repeat_inserted) = store
            .finish_conversation_tool_call(&call.item_id, result)
            .await
            .expect("repeat result");

        assert!(inserted);
        assert!(!repeat_inserted);
        assert_eq!(first.item_id, repeat.item_id);
    }

    let items = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Audit)
        .await
        .expect("items");
    assert_eq!(items.len(), 8);
    for pair in items.chunks_exact(2) {
        assert_eq!(pair[0].kind, ConversationItemKind::ToolCall);
        assert_eq!(pair[1].kind, ConversationItemKind::ToolResult);
        assert_eq!(pair[0].status, pair[1].status);
    }
}

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
    store
        .append_conversation_item(append(
            noema_conversations::ConversationItemKind::ToolResult,
            noema_conversations::ActorRef::new("agent:primary").expect("agent actor"),
            "tool evidence",
        ))
        .await
        .expect("tool result");

    let first = store
        .capture_memory_source_range(&conversation.conversation_id, 0)
        .await
        .expect("first source range");
    assert_eq!(first.captured_head_sequence, 3);
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.sequence_index)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
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

    assert_eq!(resumed.captured_head_sequence, 4);
    assert_eq!(resumed.items.len(), 1);
    assert_eq!(resumed.items[0].content_text.as_deref(), Some("later"));
}
