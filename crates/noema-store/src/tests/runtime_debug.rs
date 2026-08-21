use noema_conversations::NewConversationTurn;
use serde_json::json;

use super::test_store;
use crate::{
    NewRuntimeDebugSpan, RuntimeDebugChildSpan, RuntimeDebugMetadata, RuntimeDebugScope,
    RuntimeDebugSpanCategory, RuntimeDebugSpanStatus,
};

#[tokio::test]
async fn turn_finalization_finishes_debug_span_and_child_item() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
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
    let scope = RuntimeDebugScope::ConversationTurn(turn.turn_id.clone());
    store
        .begin_runtime_debug_span(NewRuntimeDebugSpan {
            scope: scope.clone(),
            category: RuntimeDebugSpanCategory::Provider,
            name: "Initial provider request".to_string(),
            metadata: RuntimeDebugMetadata {
                phase: Some("initial".to_string()),
                ..RuntimeDebugMetadata::default()
            },
        })
        .await
        .expect("start span");
    let live = store
        .runtime_debug_profile(scope.clone())
        .await
        .expect("live profile")
        .expect("turn profile");
    assert_eq!(live.spans[0].status, RuntimeDebugSpanStatus::Running);

    store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_actor_id) VALUES ('item:unfinished-call', ?1, ?2, 1, 'tool_call', 'running', 'agent:primary')",
                rusqlite::params![conversation.conversation_id, turn.turn_id],
            )?;
            Ok(())
        })
        .await
        .expect("unfinished call");
    store
        .complete_conversation_turn(&turn.turn_id)
        .await
        .expect("complete turn");
    let durable = store
        .runtime_debug_profile(scope)
        .await
        .expect("durable profile")
        .expect("turn profile");
    assert_eq!(durable.spans[0].status, RuntimeDebugSpanStatus::Completed);
    assert!(durable.spans[0].duration_milliseconds.is_some());

    let connection = store.connection_for_tests();
    let connection = connection.lock().await;
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM conversation_items WHERE item_id = 'item:unfinished-call'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("finished call"),
        "failed"
    );
    let invalid = connection.execute(
        "INSERT INTO runtime_debug_spans (span_id, category, name) VALUES ('debug_span:invalid', 'runtime', 'invalid')",
        [],
    );
    assert!(
        invalid.is_err(),
        "a span without exactly one owner must fail"
    );
}

#[tokio::test]
async fn provider_child_spans_keep_exact_parent_offsets() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id,
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("turn");
    let scope = RuntimeDebugScope::ConversationTurn(turn.turn_id.clone());
    let parent_id = store
        .begin_runtime_debug_span(NewRuntimeDebugSpan {
            scope: scope.clone(),
            category: RuntimeDebugSpanCategory::Provider,
            name: "Initial provider request".to_string(),
            metadata: RuntimeDebugMetadata::default(),
        })
        .await
        .expect("parent span");
    store
        .finish_runtime_debug_span_with_children(
            &parent_id,
            RuntimeDebugSpanStatus::Failed,
            100,
            RuntimeDebugMetadata {
                error: Some("provider rejected previous_response_id".to_string()),
                ..RuntimeDebugMetadata::default()
            },
            &[RuntimeDebugChildSpan {
                name: "Hosted web search".to_string(),
                start_offset_milliseconds: 25,
                duration_milliseconds: 30,
                metadata: RuntimeDebugMetadata {
                    tool_name: Some("web.search".to_string()),
                    ..RuntimeDebugMetadata::default()
                },
            }],
        )
        .await
        .expect("finish with child");

    let profile = store
        .runtime_debug_profile(scope)
        .await
        .expect("profile")
        .expect("stored profile");
    assert_eq!(profile.spans.len(), 2);
    let parent = profile
        .spans
        .iter()
        .find(|span| span.span_id == parent_id)
        .expect("parent span");
    assert_eq!(parent.status, RuntimeDebugSpanStatus::Failed);
    assert_eq!(
        parent.metadata.error.as_deref(),
        Some("provider rejected previous_response_id")
    );
    let child = profile
        .spans
        .iter()
        .find(|span| span.name == "Hosted web search")
        .expect("child span");
    assert_eq!(child.duration_milliseconds, Some(30));
    assert_eq!(child.metadata.tool_name.as_deref(), Some("web.search"));

    let invalid_parent_id = store
        .begin_runtime_debug_span(NewRuntimeDebugSpan {
            scope: RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
            category: RuntimeDebugSpanCategory::Provider,
            name: "Second provider request".to_string(),
            metadata: RuntimeDebugMetadata::default(),
        })
        .await
        .expect("second parent");
    let result = store
        .finish_runtime_debug_span_with_children(
            &invalid_parent_id,
            RuntimeDebugSpanStatus::Completed,
            100,
            RuntimeDebugMetadata::default(),
            &[RuntimeDebugChildSpan {
                name: "Outside parent".to_string(),
                start_offset_milliseconds: 90,
                duration_milliseconds: 20,
                metadata: RuntimeDebugMetadata::default(),
            }],
        )
        .await;
    assert!(result.is_err());
    let invalid_profile = store
        .runtime_debug_profile(RuntimeDebugScope::ConversationTurn(turn.turn_id))
        .await
        .expect("profile")
        .expect("stored profile");
    assert_eq!(
        invalid_profile
            .spans
            .iter()
            .find(|span| span.span_id == invalid_parent_id)
            .expect("second parent")
            .status,
        RuntimeDebugSpanStatus::Running
    );

    let connection = store.connection_for_tests();
    let connection = connection.lock().await;
    let offset = connection
        .query_row(
            "SELECT CAST(ROUND((julianday(child.started_at) - julianday(parent.started_at)) * 86400000) AS INTEGER) FROM runtime_debug_spans child JOIN runtime_debug_spans parent ON parent.span_id = ?1 WHERE child.span_id = ?2",
            rusqlite::params![parent_id, child.span_id],
            |row| row.get::<_, i64>(0),
        )
        .expect("child offset");
    assert_eq!(offset, 25);
}
