#[tokio::test]
async fn compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        context_window_tokens: Some(5_500),
        ..CapturingProvider::default()
    });
    let runtime = RuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    append_test_text_item(&store, &started.conversation_id, "covered user").await;
    append_test_text_item(&store, &started.conversation_id, "covered assistant").await;
    store
        .insert_conversation_context_summary(noema_conversations::NewConversationContextSummary {
            conversation_id: started.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "Summary: compacted checkpoint facts.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 2,
            source_item_ids: vec!["item:1".to_string(), "item:2".to_string()],
            input_token_estimate: 400,
            summary_token_estimate: 16,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: noema_conversations::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("summary");
    append_test_text_item(&store, &started.conversation_id, "post checkpoint user").await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "current turn".to_string(), tx)
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    let instructions = request.instructions.as_deref().expect("instructions");
    assert!(!instructions.contains("compacted checkpoint facts"));
    let input_texts = input_message_texts(&request.input);
    assert_eq!(
        input_texts.first().map(String::as_str),
        Some(
            "Compacted conversation context:\nSummary: compacted checkpoint facts.\n\nRecent transcript after this compacted checkpoint follows in subsequent messages."
        )
    );
    assert!(
        input_texts
            .iter()
            .any(|text| text == "post checkpoint user")
    );
    assert!(input_texts.iter().any(|text| text == "current turn"));
    assert!(!input_texts.iter().any(|text| text == "covered user"));
}

#[tokio::test]
async fn runtime_persists_and_replays_encrypted_reasoning_items() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::ReasoningReplay)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let first_items = collect_turn(&handle, conversation_id.clone(), "first".to_string())
        .await
        .expect("first turn");
    assert!(first_items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "first answer"
    )));

    let second_items = collect_turn(&handle, conversation_id, "second".to_string())
        .await
        .expect("second turn");
    assert!(second_items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw encrypted reasoning"
    )));
    handle.shutdown().await;
}

#[tokio::test]
async fn prompt_context_keeps_all_post_checkpoint_items_for_budgeting() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        context_window_tokens: Some(20_000),
        ..CapturingProvider::default()
    });
    let runtime = RuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let conversation_id = runtime
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    for index in 1..=45 {
        append_test_text_item(
            &store,
            &conversation_id,
            &format!("post checkpoint item {index}"),
        )
        .await;
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(conversation_id, "current turn".to_string(), tx)
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let input = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request")
        .input
        .render_for_token_count();
    assert!(input.contains("post checkpoint item 1"));
    assert!(input.contains("post checkpoint item 45"));
}

#[tokio::test]
async fn prompt_context_falls_back_to_estimates_when_token_count_fails() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        context_window_tokens: Some(20_000),
        fail_token_count: true,
        ..CapturingProvider::default()
    });
    let runtime = RuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store,
        "foundation_local",
    )
    .await
    .expect("runtime");
    let conversation_id = runtime
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    collect_turn(&runtime, conversation_id, "hello".to_string())
        .await
        .expect("fallback token estimate");
    runtime.shutdown().await;
    assert!(
        provider
            .requests
            .lock()
            .expect("requests")
            .iter()
            .any(|request| request.options.require_noema_response)
    );
}

#[tokio::test]
async fn prompt_context_sends_prior_transcript_as_provider_messages() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider::default());
    let runtime = RuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    append_test_text_item_with_kind(
        &store,
        &started.conversation_id,
        ConversationItemKind::UserText,
        "first durable question",
    )
    .await;
    append_test_text_item_with_kind(
        &store,
        &started.conversation_id,
        ConversationItemKind::AssistantText,
        "first durable answer",
    )
    .await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id,
            "second durable question".to_string(),
            tx,
        )
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    assert_eq!(request.options.prompt_cache_retention, None);
    let GenerateInput::Messages(messages) = &request.input else {
        panic!("expected transcript messages, got {:?}", request.input);
    };
    let context_updates = messages
        .iter()
        .filter(|message| message.role == noema_providers::GenerateMessageRole::Developer)
        .collect::<Vec<_>>();
    assert_eq!(context_updates.len(), 3);
    assert!(
        context_updates
            .iter()
            .all(|message| message.content.starts_with("NOEMA_MODEL_CONTEXT_UPDATE"))
    );
    let observed = messages
        .iter()
        .filter(|message| message.role != noema_providers::GenerateMessageRole::Developer)
        .map(|message| (message.role, message.content.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            (
                noema_providers::GenerateMessageRole::User,
                "first durable question"
            ),
            (
                noema_providers::GenerateMessageRole::Assistant,
                "first durable answer"
            ),
            (
                noema_providers::GenerateMessageRole::User,
                "second durable question"
            ),
        ]
    );
}
