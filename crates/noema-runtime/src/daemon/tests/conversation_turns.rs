#[tokio::test]
async fn runtime_turn_streams_durable_assistant_item_and_idle_status() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "hello".to_string()).await;
    result.expect("turn");
    handle.shutdown().await;

    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::InputReceived,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Thinking,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Idle,
            } if id == &conversation_id
        )
    }));

    let Some((assistant_item_id, assistant_turn_id)) =
        events.iter().find_map(|event| match event {
            TurnStreamEvent::ConversationItem {
                conversation_id: id,
                item_id,
                turn_id,
                item,
                ..
            } if id == &conversation_id => match item.as_ref() {
                TurnTranscriptItem::AssistantText { text } if text == "fake answer" => {
                    Some((item_id.clone(), turn_id.clone()))
                }
                _ => None,
            },
            _ => None,
        })
    else {
        panic!("expected durable assistant conversation item, got {events:?}");
    };
    assert!(assistant_item_id.starts_with("item:"));
    assert!(assistant_turn_id.is_some());

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.item_id == assistant_item_id
            && item.kind == ConversationItemKind::AssistantText
            && item.status == ConversationItemStatus::Completed
    }));
}

#[tokio::test]
async fn turn_persists_multiple_choice_prompt() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::MultipleChoice)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "choose".to_string()).await;
    result.expect("turn");
    handle.shutdown().await;

    let prompt_event = events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem { item_id, item, .. } => match item.as_ref() {
                TurnTranscriptItem::MultipleChoicePrompt {
                    prompt,
                    selection_mode,
                    options,
                } if prompt == "Pick a direction"
                    && selection_mode == &MultipleChoiceSelectionMode::PickOne =>
                {
                    Some((item_id.clone(), options.clone()))
                }
                _ => None,
            },
            TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .expect("multiple choice prompt event");
    assert_eq!(
        prompt_event.1,
        vec![
            MultipleChoiceOption {
                id: "ship".to_string(),
                label: "Ship it".to_string(),
            },
            MultipleChoiceOption {
                id: "polish".to_string(),
                label: "Polish first".to_string(),
            },
        ]
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.item_id == prompt_event.0
            && item.kind == ConversationItemKind::MultipleChoicePrompt
            && item.status == ConversationItemStatus::Completed
            && item.content_text.as_deref() == Some("Pick a direction")
            && item.payload_json["selection_mode"] == "pick_one"
            && item.payload_json["options"][0]["id"] == "ship"
    }));
}

#[tokio::test]
async fn multiple_choice_selection_pick_one_appends_user_item() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let prompt_item_id = append_test_multiple_choice_prompt(
        &store,
        &conversation.conversation_id,
        MultipleChoiceSelectionMode::PickOne,
    )
    .await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    handle
        .select_multiple_choice_with_client_message_id(
            conversation.conversation_id.clone(),
            prompt_item_id.clone(),
            vec!["ship".to_string()],
            tx,
            None,
        )
        .await
        .expect("selection turn");
    while rx.recv().await.is_some() {}
    handle.shutdown().await;

    let replay = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::MultipleChoiceSelection
            && item.content_text.as_deref() == Some("Ship it")
            && item.payload_json["prompt_item_id"] == prompt_item_id
            && item.payload_json["selected_options"][0]["id"] == "ship"
    }));
}

#[tokio::test]
async fn multiple_choice_selection_rejects_invalid_option_id() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let prompt_item_id = append_test_multiple_choice_prompt(
        &store,
        &conversation.conversation_id,
        MultipleChoiceSelectionMode::PickOne,
    )
    .await;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = handle
        .select_multiple_choice_with_client_message_id(
            conversation.conversation_id.clone(),
            prompt_item_id,
            vec!["missing".to_string()],
            tx,
            None,
        )
        .await
        .expect_err("invalid id");
    handle.shutdown().await;

    assert!(
        error
            .to_string()
            .contains("multiple-choice option id is not in the prompt")
    );
    let replay = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        !replay
            .iter()
            .any(|item| item.kind == ConversationItemKind::MultipleChoiceSelection)
    );
}

#[tokio::test]
async fn slash_remember_is_ordinary_chat_text() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "/remember I like trains".to_string(),
    )
    .await
    .expect("turn response");
    handle.shutdown().await;

    let items = store
        .list_visible_conversation_item_page(&conversation_id, None, 20)
        .await
        .expect("items");
    assert!(
        items
            .items
            .iter()
            .any(|item| item.content_text.as_deref() == Some("/remember I like trains"))
    );
    assert!(!items.items.iter().any(|item| {
        item.payload_json
            .get("activity_kind")
            .and_then(serde_json::Value::as_str)
            == Some("memory_save")
    }));
    assert!(!items.items.iter().any(|item| {
        item.payload_json
            .get("activity_kind")
            .and_then(serde_json::Value::as_str)
            == Some("memory_extraction")
    }));
}

#[tokio::test]
async fn primary_agent_runtime_preference_supplies_turn_model() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    authenticate_provider_account(&store, &account.provider_account_id).await;
    upsert_ready_agent_runtime_preference(
        &store,
        noema_store::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "default".to_string(),
            reasoning_effort: None,
        },
    )
    .await;

    let provider = Arc::new(CapturingProvider::default());
    let runtime =
        RuntimeHandle::spawn_with_provider_kind(provider.clone(), store, "foundation_local")
            .await
            .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
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
    assert_eq!(
        requests.last().and_then(|request| request.model.as_deref()),
        Some("default")
    );
}

#[tokio::test]
async fn primary_agent_runtime_preference_supplies_reasoning_effort() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    authenticate_provider_account(&store, &account.provider_account_id).await;
    upsert_ready_agent_runtime_preference(
        &store,
        noema_store::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(noema_providers::ReasoningEffort::High),
        },
    )
    .await;

    let codex_provider = Arc::new(CapturingProvider::default());
    let runtime = RuntimeHandle::spawn_with_provider_kind(codex_provider.clone(), store, "codex")
        .await
        .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    let requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        Some(noema_providers::ReasoningEffort::High)
    );
}

#[tokio::test]
async fn primary_agent_codex_preference_sends_reasoning_effort_to_codex_provider_kind() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    authenticate_provider_account(&store, &account.provider_account_id).await;
    upsert_ready_agent_runtime_preference(
        &store,
        noema_store::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(noema_providers::ReasoningEffort::High),
        },
    )
    .await;

    let codex_provider = Arc::new(CapturingProvider::default());
    let openai_provider = Arc::new(CapturingProvider::default());
    let runtime = RuntimeHandle::spawn_with_provider_map(
        "openai",
        vec![
            (
                "codex".to_string(),
                codex_provider.clone() as noema_providers::ProviderHandle,
            ),
            (
                "openai".to_string(),
                openai_provider.clone() as noema_providers::ProviderHandle,
            ),
        ],
        store,
    )
    .await
    .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    assert!(openai_provider.requests.lock().expect("openai").is_empty());
    let codex_requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        codex_requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        Some(noema_providers::ReasoningEffort::High)
    );
}

#[tokio::test]
async fn primary_agent_openai_preference_sends_reasoning_effort_to_openai_provider_kind() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider_account_id = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account")
        .provider_account_id;
    authenticate_provider_account(&store, &provider_account_id).await;
    upsert_ready_agent_runtime_preference(
        &store,
        noema_store::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id,
            model_profile: "default".to_string(),
            reasoning_effort: Some(noema_providers::ReasoningEffort::Medium),
        },
    )
    .await;

    let codex_provider = Arc::new(CapturingProvider::default());
    let foundation_provider = Arc::new(CapturingProvider::default());
    let runtime = RuntimeHandle::spawn_with_provider_map(
        "codex",
        vec![
            (
                "codex".to_string(),
                codex_provider.clone() as noema_providers::ProviderHandle,
            ),
            (
                "foundation_local".to_string(),
                foundation_provider.clone() as noema_providers::ProviderHandle,
            ),
        ],
        store,
    )
    .await
    .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    assert!(codex_provider.requests.lock().expect("codex").is_empty());
    let foundation_requests = foundation_provider
        .requests
        .lock()
        .expect("foundation requests");
    assert_eq!(
        foundation_requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        Some(noema_providers::ReasoningEffort::Medium)
    );
}

#[tokio::test]
async fn primary_agent_default_provider_sends_no_reasoning_effort() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let codex_provider = Arc::new(CapturingProvider::default());
    let runtime = RuntimeHandle::spawn_with_provider_kind(codex_provider.clone(), store, "codex")
        .await
        .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    let requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        None
    );
}
