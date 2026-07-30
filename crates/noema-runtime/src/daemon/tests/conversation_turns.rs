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
            && request.tool_transport == noema_providers::ProviderToolTransport::Native
    }));
}

#[tokio::test]
async fn exact_reset_command_persists_notice_without_calling_provider() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider::default());
    let handle = RuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "/reset".to_string()).await;
    result.expect("reset");
    handle.shutdown().await;

    assert!(provider.requests.lock().expect("requests").is_empty());
    assert!(events.iter().any(|event| matches!(
        event,
        TurnStreamEvent::ConversationItem { item, .. }
            if matches!(item.as_ref(), TurnTranscriptItem::Activity {
                activity_kind,
                title,
                status: TurnActivityStatus::Completed,
                ..
            } if activity_kind == "context_reset" && title == "Context reset")
    )));
    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].payload_json["activity_kind"], "context_reset");
}

#[tokio::test]
async fn reset_with_arguments_remains_a_normal_provider_turn() {
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

    collect_turn(&handle, conversation_id, "/reset now".to_string())
        .await
        .expect("normal turn");
    handle.shutdown().await;

    assert_eq!(provider.requests().len(), 1);
}

#[tokio::test]
async fn turn_persists_native_multiple_choice_tool_call() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::MultipleChoice)).await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "choose".to_string()).await;
    result.expect("turn");
    let (blocked, _) =
        collect_turn_events(&handle, conversation_id.clone(), "interrupt".to_string()).await;
    assert!(
        matches!(blocked, Err(RuntimeError::Protocol(message)) if message.contains("waiting for a human interaction"))
    );
    handle.shutdown().await;

    let tool_call_event = events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. } => match item.as_ref() {
                TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_call"
                    && title == "Tool call: noema.present_multiple_choice" =>
                {
                    Some(metadata.clone())
                }
                _ => None,
            },
            TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .expect("native multiple choice tool call event");
    assert_eq!(
        tool_call_event["action"]["name"],
        "noema.present_multiple_choice"
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::ToolCall
            && item.status == ConversationItemStatus::Running
            && item.payload_json["activity_kind"] == "tool_call"
            && item.payload_json["metadata"]["action"]["name"] == "noema.present_multiple_choice"
    }));
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
        "codex" => {
            store
                .ensure_default_provider_account()
                .await
                .expect("codex account")
                .provider_account_id
        }
        "foundation_local" => {
            store
                .ensure_default_foundation_local_provider_account()
                .await
                .expect("foundation account")
                .provider_account_id
        }
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
                selection: noema_providers::ModelPreferenceSelection::ExplicitProfile {
                    model_profile: model_profile.to_string(),
                    reasoning_effort,
                },
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
    let requests = selected_provider
        .requests
        .lock()
        .expect("selected requests");
    assert_eq!(
        requests.last().and_then(|request| request.model.as_deref()),
        Some(model_profile)
    );
    let expected_reasoning_effort = reasoning_effort.or_else(|| {
        (provider_kind == "codex" && model_profile == "gpt-5.6-luna")
            .then_some(noema_providers::ReasoningEffort::Low)
    });
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        expected_reasoning_effort
    );
}
