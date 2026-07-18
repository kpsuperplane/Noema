use super::{seed_task, test_store};

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

#[tokio::test]
async fn durable_task_events_expose_pending_status_deliveries_until_materialized() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let (task, _) = seed_task(&store, "Pending delivery").await;
    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE tasks SET source_conversation_id = ?2, status = 'failed' WHERE task_id = ?1",
                rusqlite::params![task.task_id, conversation.conversation_id],
            )?;
            Ok(())
        })
        .await
        .expect("terminal task");
    let event_id = store
        .append_task_event(noema_tasks::NewTaskEvent {
            event_id: Some("event:pending-delivery".to_string()),
            task_id: task.task_id.clone(),
            event_kind: noema_tasks::TaskEventKind::new("task.failed").expect("valid event kind"),
            actor_id: "system:task-runtime".to_string(),
            causation_id: None,
            correlation_id: None,
            payload: serde_json::json!({}),
        })
        .await
        .expect("status event");
    assert_eq!(
        store
            .list_pending_task_status_deliveries(10)
            .await
            .expect("pending"),
        vec![task.task_id.clone()]
    );
    assert_eq!(
        store
            .list_pending_task_completion_deliveries(10)
            .await
            .expect("pending completion"),
        vec![(task.task_id.clone(), event_id.clone())]
    );

    store
        .append_conversation_item_with_id(
            format!("item:task_status:{event_id}"),
            noema_conversations::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: None,
                parent_item_id: None,
                kind: noema_conversations::ConversationItemKind::TaskReference,
                status: noema_conversations::ConversationItemStatus::Completed,
                author: noema_conversations::ActorRef::system("system:task-runtime")
                    .expect("static task runtime actor id must be valid"),
                content_text: Some("Pending delivery".to_string()),
                payload_json: serde_json::json!({"task_id": task.task_id}),
                metadata: serde_json::json!({"task_event_id": event_id}),
            },
        )
        .await
        .expect("delivery");
    assert!(
        store
            .list_pending_task_status_deliveries(10)
            .await
            .expect("drained")
            .is_empty()
    );

    store
        .append_conversation_item_with_id(
            format!("item:task_completion:{event_id}"),
            noema_conversations::NewConversationItem {
                conversation_id: conversation.conversation_id,
                turn_id: None,
                parent_item_id: None,
                kind: noema_conversations::ConversationItemKind::AssistantText,
                status: noema_conversations::ConversationItemStatus::Completed,
                author: noema_conversations::ActorRef::agent("agent:primary")
                    .expect("static primary agent id must be valid"),
                content_text: Some("The task failed.".to_string()),
                payload_json: serde_json::json!({"task_id": task.task_id}),
                metadata: serde_json::json!({"delivery_id": event_id}),
            },
        )
        .await
        .expect("completion delivery");
    assert!(
        store
            .list_pending_task_completion_deliveries(10)
            .await
            .expect("completion drained")
            .is_empty()
    );
}
