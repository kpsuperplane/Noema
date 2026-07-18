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

    let first_delta = events
        .iter()
        .position(|event| matches!(event, TurnStreamEvent::AssistantTextDelta { .. }))
        .expect("assistant delta");
    let durable_assistant = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnStreamEvent::ConversationItem { item, .. }
                    if matches!(item.as_ref(), TurnTranscriptItem::AssistantText { .. })
            )
        })
        .expect("durable assistant");
    assert!(first_delta < durable_assistant);

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
async fn runtime_turn_passes_conversation_id_to_provider_request() {
    let provider = Arc::new(fake_provider(FakeCodexScenario::Simple));
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    collect_turn(&handle, conversation_id.clone(), "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(provider.requests().iter().any(|request| {
        request.conversation_id.as_deref() == Some(conversation_id.as_str())
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

    let invalid_prompt_item_id = append_test_multiple_choice_prompt(
        &store,
        &conversation.conversation_id,
        MultipleChoiceSelectionMode::PickOne,
    )
    .await;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = handle
        .select_multiple_choice_with_client_message_id(
            conversation.conversation_id.clone(),
            invalid_prompt_item_id.clone(),
            vec!["missing".to_string()],
            tx,
            None,
        )
        .await
        .expect_err("invalid id");
    assert!(
        error
            .to_string()
            .contains("multiple-choice option id is not in the prompt")
    );
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
    assert!(
        replay.iter().all(|item| item.kind
            != ConversationItemKind::MultipleChoiceSelection
            || item.payload_json["prompt_item_id"] != invalid_prompt_item_id)
    );
}

#[tokio::test]
async fn primary_agent_preferences_route_model_and_reasoning_by_provider_kind() {
    for (provider_kind, model_profile, reasoning_effort) in [
        (
            "codex",
            "gpt-5.5",
            Some(noema_providers::ReasoningEffort::High),
        ),
        (
            "foundation_local",
            "default",
            Some(noema_providers::ReasoningEffort::Medium),
        ),
        ("codex", "gpt-5.6-luna", None),
    ] {
        assert_primary_preference_routes(provider_kind, model_profile, reasoning_effort).await;
    }
}

async fn assert_primary_preference_routes(
    provider_kind: &str,
    model_profile: &str,
    reasoning_effort: Option<noema_providers::ReasoningEffort>,
) {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider_account_id = match provider_kind {
        "codex" => store
            .ensure_default_provider_account()
            .await
            .expect("codex account")
            .provider_account_id,
        "foundation_local" => store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account")
            .provider_account_id,
        _ => unreachable!("unsupported test provider"),
    };
    authenticate_provider_account(&store, &provider_account_id).await;
    if reasoning_effort.is_some() {
        upsert_ready_agent_runtime_preference(
            &store,
            noema_store::NewAgentRuntimePreference {
                agent_id: "agent:primary".to_string(),
                provider_kind: provider_kind.to_string(),
                provider_account_id,
                model_profile: model_profile.to_string(),
                reasoning_effort,
            },
        )
        .await;
    }

    let selected_provider = Arc::new(CapturingProvider::default());
    let other_provider = Arc::new(CapturingProvider::default());
    let other_kind = if provider_kind == "codex" {
        "foundation_local"
    } else {
        "codex"
    };
    let default_kind = if reasoning_effort.is_some() {
        other_kind
    } else {
        provider_kind
    };
    let runtime = RuntimeHandle::spawn_with_provider_map(
        default_kind,
        vec![
            (
                provider_kind.to_string(),
                selected_provider.clone() as noema_providers::ProviderHandle,
            ),
            (
                other_kind.to_string(),
                other_provider.clone() as noema_providers::ProviderHandle,
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

    assert!(other_provider.requests.lock().expect("other").is_empty());
    let requests = selected_provider.requests.lock().expect("selected requests");
    assert_eq!(
        requests.last().and_then(|request| request.model.as_deref()),
        Some(model_profile)
    );
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        reasoning_effort
    );
}
