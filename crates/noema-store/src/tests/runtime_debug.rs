use noema_conversations::NewConversationTurn;
use serde_json::json;

use super::test_store;
use crate::{
    NewRuntimeDebugSpan, RuntimeDebugMetadata, RuntimeDebugScope, RuntimeDebugSpanCategory,
    RuntimeDebugSpanStatus,
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
