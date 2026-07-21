#[tokio::test]
async fn runtime_debug_profile_projects_safe_timing_and_usage() {
    use noema_store::{
        NewRuntimeDebugSpan, RuntimeDebugMetadata, RuntimeDebugScope,
        RuntimeDebugSpanCategory, RuntimeDebugSpanStatus,
    };

    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id;
    let turn = store
        .create_conversation_turn(noema_conversations::NewConversationTurn {
            conversation_id: conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("turn");
    let span_id = store
        .begin_runtime_debug_span(NewRuntimeDebugSpan {
            scope: RuntimeDebugScope::ConversationTurn(turn.turn_id.clone()),
            category: RuntimeDebugSpanCategory::Provider,
            name: "Final provider request".to_string(),
            metadata: RuntimeDebugMetadata::default(),
        })
        .await
        .expect("start span");
    store
        .finish_runtime_debug_span(
            &span_id,
            RuntimeDebugSpanStatus::Completed,
            65_272,
            RuntimeDebugMetadata {
                provider: Some("codex".to_string()),
                model: Some("gpt-5.6".to_string()),
                phase: Some("finalization".to_string()),
                input_tokens: Some(120),
                output_tokens: Some(40),
                total_tokens: Some(160),
                ..RuntimeDebugMetadata::default()
            },
        )
        .await
        .expect("finish span");
    store
        .complete_conversation_turn(&turn.turn_id)
        .await
        .expect("complete turn");

    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let response = schema
        .execute(format!(
            r#"query {{ runtimeDebugProfile(input: {{ kind: CONVERSATION_TURN, scopeId: "{}" }}) {{ status spans {{ name durationMilliseconds provider phase totalTokens }} }} }}"#,
            turn.turn_id
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("profile json");
    assert_json_fields!(
        data,
        "/runtimeDebugProfile/spans/0/name" => "Final provider request",
        "/runtimeDebugProfile/spans/0/durationMilliseconds" => 65_272,
        "/runtimeDebugProfile/spans/0/provider" => "codex",
        "/runtimeDebugProfile/spans/0/phase" => "finalization",
        "/runtimeDebugProfile/spans/0/totalTokens" => 160,
    );

    let legacy_turn = store
        .create_conversation_turn(noema_conversations::NewConversationTurn {
            conversation_id,
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("legacy turn");
    store
        .complete_conversation_turn(&legacy_turn.turn_id)
        .await
        .expect("complete legacy turn");
    let legacy = schema
        .execute(
            format!(
                r#"query {{ runtimeDebugProfile(input: {{ kind: CONVERSATION_TURN, scopeId: "{}" }}) {{ status }} }}"#,
                legacy_turn.turn_id
            ),
        )
        .await;
    assert!(legacy.errors.is_empty(), "{:?}", legacy.errors);
    assert_eq!(
        legacy.data.into_json().expect("legacy profile json"),
        json!({ "runtimeDebugProfile": null })
    );
}
