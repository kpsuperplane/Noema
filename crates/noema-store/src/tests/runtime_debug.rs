use noema_conversations::NewConversationTurn;
use serde_json::json;

use super::test_store;
use crate::{
    NewRuntimeDebugSpan, RuntimeDebugMetadata, RuntimeDebugScope, RuntimeDebugSpanCategory,
    RuntimeDebugSpanStatus,
};

#[tokio::test]
async fn runtime_debug_span_is_live_then_durable_for_exactly_one_owner() {
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
    let span_id = store
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
        .finish_runtime_debug_span(
            &span_id,
            RuntimeDebugSpanStatus::Completed,
            42,
            RuntimeDebugMetadata::default(),
        )
        .await
        .expect("finish span");
    let durable = store
        .runtime_debug_profile(scope)
        .await
        .expect("durable profile")
        .expect("turn profile");
    assert_eq!(durable.spans[0].duration_milliseconds, Some(42));

    let connection = store.connection_for_tests();
    let connection = connection.lock().await;
    let invalid = connection.execute(
        "INSERT INTO runtime_debug_spans (span_id, category, name) VALUES ('debug_span:invalid', 'runtime', 'invalid')",
        [],
    );
    assert!(
        invalid.is_err(),
        "a span without exactly one owner must fail"
    );
}
