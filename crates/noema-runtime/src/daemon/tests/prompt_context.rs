#[tokio::test]
async fn native_provider_turn_request_includes_builtin_tools() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        capabilities: ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: true,
            prompt_cache_key: false,
            prompt_cache_options: false,
            prompt_cache_breakpoints: false,
            encrypted_reasoning: false,
        },
        requests: Mutex::new(Vec::new()),
    });
    let runtime = RuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    assert_eq!(
        request.options.prompt_cache_retention,
        Some(noema_providers::PromptCacheRetention::TwentyFourHours)
    );
    let tool_names = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"search_memory"));
    assert!(request.parallel_tool_calls);
    let instructions = request.instructions.as_deref().expect("instructions");
    assert!(
        latest_model_context_section(&request.input, "tools.visibility")
            .is_some_and(|context| context.contains("provided through the native tool channel"))
    );
    assert!(!instructions.contains("emit the relevant tool_calls item in this response"));
}

#[tokio::test]
async fn normal_turn_instructions_are_stable_across_turns() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = RuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id.clone(),
            "first durable question".to_string(),
            first_tx,
        )
        .await
        .expect("first turn");
    while first_rx.recv().await.is_some() {}

    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id,
            "second durable question".to_string(),
            second_tx,
        )
        .await
        .expect("second turn");
    while second_rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let agent_requests = requests
        .iter()
        .filter(|request| request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert_eq!(agent_requests.len(), 2);

    let first_instructions = agent_requests[0]
        .instructions
        .as_deref()
        .expect("first instructions");
    let second_instructions = agent_requests[1]
        .instructions
        .as_deref()
        .expect("second instructions");
    assert_eq!(first_instructions, second_instructions);
    assert!(!first_instructions.contains("conversation_id:"));
    assert!(!first_instructions.contains("turn_index:"));
    assert!(!first_instructions.contains("cwd_project_hint:"));
    assert!(!first_instructions.contains("Recent durable transcript"));
    assert!(!first_instructions.contains("first durable question"));
    assert!(!second_instructions.contains("second durable question"));
}

#[tokio::test]
async fn normal_turn_appends_only_changed_keyed_context_sections() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = RuntimeHandle::spawn_with_provider(provider.clone(), store.clone())
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id.clone(),
            "first question".to_string(),
            first_tx,
        )
        .await
        .expect("first turn");
    while first_rx.recv().await.is_some() {}

    store
        .update_agent_display_name("agent:primary", "Mira")
        .await
        .expect("rename agent");
    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id,
            "second question".to_string(),
            second_tx,
        )
        .await
        .expect("second turn");
    while second_rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let agent_requests = requests
        .iter()
        .filter(|request| request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert_eq!(agent_requests.len(), 2);
    assert_eq!(
        agent_requests[0].instructions,
        agent_requests[1].instructions
    );
    assert_eq!(
        model_context_section_update_count(&agent_requests[0].input, "agent.identity"),
        1
    );
    assert_eq!(
        model_context_section_update_count(&agent_requests[1].input, "agent.identity"),
        2
    );
    assert_eq!(
        model_context_section_update_count(&agent_requests[1].input, "tools.visibility"),
        1,
        "unchanged tool visibility should stay in the cached prefix"
    );
    assert!(
        latest_model_context_section(&agent_requests[1].input, "agent.identity")
            .is_some_and(|content| content.contains(r#"display_name: "Mira""#))
    );
}

#[tokio::test]
async fn normal_turn_input_replays_previous_turn_as_prefix() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = RuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id.clone(),
            "first durable question".to_string(),
            first_tx,
        )
        .await
        .expect("first turn");
    while first_rx.recv().await.is_some() {}

    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(
            started.conversation_id,
            "second durable question".to_string(),
            second_tx,
        )
        .await
        .expect("second turn");
    while second_rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let agent_requests = requests
        .iter()
        .filter(|request| request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert_eq!(agent_requests.len(), 2);

    let first_items = input_message_texts(&agent_requests[0].input);
    let second_items = input_message_texts(&agent_requests[1].input);
    assert_eq!(first_items, vec!["first durable question".to_string()]);
    assert!(second_items.starts_with(&[
        "first durable question".to_string(),
        "fake answer".to_string(),
    ]));
    assert_eq!(
        second_items.last().map(String::as_str),
        Some("second durable question")
    );
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
    assert!(first_items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::AssistantText { text, .. } if text == "first answer"
        )
    }));

    let second_items = collect_turn(&handle, conversation_id, "second".to_string())
        .await
        .expect("second turn");
    assert!(second_items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::AssistantText { text, .. } if text == "saw encrypted reasoning"
        )
    }));

    handle.shutdown().await;
}

#[tokio::test]
async fn prompt_context_uses_active_summary_and_post_checkpoint_items() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider::default());
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
            summary_text: "Summary: the user approved rolling durable compaction.".to_string(),
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
    assert!(!instructions.contains("Compacted conversation context:"));
    assert!(!instructions.contains("rolling durable compaction"));
    assert!(!instructions.contains("covered user"));
    assert!(!instructions.contains("post checkpoint user"));
    let input = request.input.render_for_token_count();
    assert!(input.contains("Compacted conversation context:"));
    assert!(input.contains("rolling durable compaction"));
    assert!(input.contains("post checkpoint user"));
    assert!(input.contains("current turn"));
    assert!(!input.contains("covered user"));
}

#[tokio::test]
async fn compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider::default());
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
async fn prompt_context_keeps_all_post_checkpoint_items_for_budgeting() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
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

    for index in 1..=45 {
        append_test_text_item(
            &store,
            &started.conversation_id,
            &format!("post checkpoint item {index}"),
        )
        .await;
    }

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
    let input = request.input.render_for_token_count();
    assert!(input.contains("post checkpoint item 1"));
    assert!(input.contains("post checkpoint item 45"));
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

#[tokio::test]
async fn prompt_context_falls_back_to_estimates_when_token_count_fails() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: true,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
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

    let (result, _events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "hello".to_string(),
    )
    .await;
    result.expect("turn should use fallback token estimate");
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    assert!(
        requests
            .iter()
            .any(|request| request.options.require_noema_response)
    );
}
