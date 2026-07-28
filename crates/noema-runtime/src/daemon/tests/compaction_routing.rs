#[tokio::test]
async fn foreground_context_compaction_chunks_backlog_to_fit_provider_window() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        context_window_tokens: Some(5_500),
        enforce_context_window: true,
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
    for _ in 0..4 {
        append_test_text_item(
            &store,
            &started.conversation_id,
            &"older context ".repeat(400),
        )
        .await;
    }

    let (result, events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "current turn".to_string(),
    )
    .await;
    result.expect("turn should compact oversized backlog in bounded chunks");
    runtime.shutdown().await;

    assert_eq!(context_compaction_notices(&events), 1);

    {
        let requests = provider.requests.lock().expect("requests");
        let agent_index = requests
            .iter()
            .position(|request| request.options.require_noema_response)
            .expect("agent request");
        let compaction_requests = requests
            .iter()
            .enumerate()
            .filter(|(_, request)| !request.options.require_noema_response)
            .collect::<Vec<_>>();
        assert!(
            compaction_requests.len() > 1,
            "expected multiple bounded compaction requests"
        );
        for (index, request) in compaction_requests {
            assert!(
                index < agent_index,
                "foreground compaction must precede generation"
            );
            let input = request.input.render_for_token_count();
            let input_tokens = request
                .instructions
                .as_deref()
                .map_or(0, estimated_test_tokens)
                + estimated_test_tokens(&input);
            let available = provider
                .context_window_tokens
                .expect("context window")
                .saturating_sub(request.options.max_output_tokens.unwrap_or(512))
                .saturating_sub(128);
            assert!(
                input_tokens <= available,
                "compaction request used {input_tokens} input tokens with {available} available"
            );
        }
        assert!(
            requests
                .iter()
                .any(|request| request.options.require_noema_response)
        );
    }
    assert!(
        store
            .list_context_summaries_for_conversation(&started.conversation_id)
            .await
            .expect("summaries")
            .iter()
            .any(|summary| summary.provider_kind == "foundation_local"
                && summary.model_profile.as_deref() == Some("default")
                && summary.covered_item_end_sequence >= 2)
    );
}

#[tokio::test]
async fn background_context_compaction_creates_checkpoint_after_large_turn() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        context_window_tokens: Some(18_000),
        ..CapturingProvider::default()
    });
    let events = crate::daemon::RuntimeEventRegistry::default();
    let runtime = RuntimeHandle::spawn_with_provider_map_and_events(
        "foundation_local".to_string(),
        HashMap::from([(
            "foundation_local".to_string(),
            provider.clone() as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::artifact_operations(&store).expect("artifact operations"),
        crate::test_support::system_error_logger(),
        events.clone(),
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");
    let mut conversation_events = events.subscribe_conversation(&started.conversation_id);
    append_test_text_item(
        &store,
        &started.conversation_id,
        &"background context ".repeat(2_000),
    )
    .await;

    let (result, _events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "current turn".to_string(),
    )
    .await;
    result.expect("turn");
    wait_for_context_summary_count(&store, &started.conversation_id, 1).await;
    let notice = tokio::time::timeout(Duration::from_secs(1), conversation_events.recv())
        .await
        .expect("context notice wakeup")
        .expect("context notice event");
    assert!(matches!(notice, crate::daemon::ConversationRuntimeEvent::Turn { event, .. }
        if context_compaction_notices(std::slice::from_ref(event.as_ref())) == 1));
    runtime.shutdown().await;

    {
        let requests = provider.requests.lock().expect("requests");
        let agent_index = requests
            .iter()
            .position(|request| request.options.require_noema_response)
            .expect("agent request");
        let compaction_index = requests
            .iter()
            .position(|request| !request.options.require_noema_response)
            .expect("background compaction request");
        assert!(agent_index < compaction_index);
        assert_eq!(
            requests[compaction_index].options.generation_priority,
            noema_providers::GenerationPriority::Background
        );
    }
    let active = store
        .latest_active_context_summary(
            &started.conversation_id,
            "foundation_local",
            Some("default"),
        )
        .await
        .expect("active summary")
        .expect("active summary exists");
    assert_eq!(active.summary_text, "fake answer");
    let replay = store
        .list_conversation_items(&started.conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert_eq!(
        replay
            .iter()
            .filter(|item| {
                item.kind == ConversationItemKind::Activity
                    && item.payload_json["activity_kind"] == "context_checkpoint"
            })
            .count(),
        1
    );
}

fn context_compaction_notices(events: &[TurnStreamEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(
            event,
            TurnStreamEvent::ConversationItem { item, .. }
                if matches!(item.as_ref(), TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    status: TurnActivityStatus::Completed,
                    ..
                } if activity_kind == "context_checkpoint" && title == "Context compacted")
        ))
        .count()
}

#[tokio::test]
async fn foreground_context_compaction_failure_blocks_turn_with_recoverable_notice() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(CapturingProvider {
        context_window_tokens: Some(4_096),
        fail_compaction: true,
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
    for _ in 0..4 {
        append_test_text_item(
            &store,
            &started.conversation_id,
            &"older context ".repeat(400),
        )
        .await;
    }

    let (result, events) = collect_turn_events(
        &runtime,
        started.conversation_id.clone(),
        "current turn".to_string(),
    )
    .await;
    result.expect_err("compaction failure");
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    assert!(
        requests
            .iter()
            .any(|request| !request.options.require_noema_response)
    );
    assert!(
        !requests
            .iter()
            .any(|request| request.options.require_noema_response)
    );
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem {
                item,
                ..
            } if matches!(
                item.as_ref(),
                TurnTranscriptItem::ErrorNotice {
                    message,
                    recoverable: true,
                } if message.contains("Context compaction failed before this turn could run")
            )
        )
    }));
}

#[tokio::test]
async fn primary_preference_change_applies_to_next_turn_without_rerouting_in_flight_turn() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .update_agent_display_name("agent:primary", "Noema")
        .await
        .expect("name primary");
    let codex_account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .update_provider_account_status(
            &codex_account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate codex");
    upsert_ready_agent_runtime_preference(
        &store,
        noema_store::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: codex_account.provider_account_id,
            model_profile: "codex-in-flight".to_string(),
            reasoning_effort: None,
        },
    )
    .await;
    let foundation_account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .update_provider_account_status(
            &foundation_account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate foundation");

    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let codex_provider = Arc::new(BlockingOnceProvider {
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(Some(release_rx)),
    });
    let foundation_provider = Arc::new(CapturingProvider::default());
    let runtime = RuntimeHandle::spawn_with_provider_map(
        "codex",
        vec![
            (
                "codex".to_string(),
                codex_provider as noema_providers::ProviderHandle,
            ),
            (
                "foundation_local".to_string(),
                foundation_provider.clone() as noema_providers::ProviderHandle,
            ),
        ],
        store.clone(),
    )
    .await
    .expect("runtime");
    let conversation_id = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let first_runtime = runtime.clone();
    let first_conversation_id = conversation_id.clone();
    let first_turn = tokio::spawn(async move {
        collect_turn_events(&first_runtime, first_conversation_id, "first".to_string()).await
    });

    started_rx.await.expect("codex turn started");
    upsert_ready_agent_runtime_preference(
        &store,
        noema_store::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: foundation_account.provider_account_id,
            model_profile: "default".to_string(),
            reasoning_effort: None,
        },
    )
    .await;
    release_tx.send(()).expect("release codex turn");
    first_turn
        .await
        .expect("first turn task")
        .0
        .expect("first turn");

    collect_turn_events(&runtime, conversation_id, "second".to_string())
        .await
        .0
        .expect("second turn");
    runtime.shutdown().await;

    let foundation_requests = foundation_provider
        .requests
        .lock()
        .expect("foundation requests");
    assert_eq!(foundation_requests.len(), 1);
    assert_eq!(foundation_requests[0].model.as_deref(), Some("default"));
}

#[tokio::test]
async fn runtime_turn_rehydrates_recorded_failure_conversation_for_retry() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::TurnError)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (first_result, first_events) =
        collect_turn_events(&handle, conversation_id.clone(), "first".to_string()).await;
    first_result.expect_err("first turn failure");
    assert!(first_events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem {
                item,
                ..
            } if matches!(item.as_ref(), TurnTranscriptItem::ErrorNotice { .. })
        )
    }));

    let (second_result, _second_events) =
        collect_turn_events(&handle, conversation_id.clone(), "second".to_string()).await;
    second_result.expect_err("second provider failure should not become unknown conversation");
    assert_eq!(
        store
            .next_conversation_turn_index(&conversation_id)
            .await
            .expect("next turn index"),
        3
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_turn_streams_tool_call_started_before_durable_response_items() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::SearchMemoryContinuation))
            .await;
    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await;
    result.expect("turn");
    handle.shutdown().await;

    let streamed_text = events
        .iter()
        .position(|event| matches!(event, TurnStreamEvent::AssistantTextDelta { .. }))
        .expect("assistant text delta");
    let streamed_tool_started = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnStreamEvent::ConversationItem { item_id, item, .. }
                    if item_id.starts_with("transient:tool_call:")
                        && matches!(
                            item.as_ref(),
                            TurnTranscriptItem::Activity {
                                activity_kind,
                                status: TurnActivityStatus::Started,
                                title,
                                ..
                            } if activity_kind == "tool_call" && title == "Tool call: search_memory"
                        )
            )
        })
        .expect("transient tool call started item");
    let durable_commentary = assistant_text_item_event_index(&events, "Searching memory.")
        .expect("durable commentary item");

    assert!(
        streamed_text < streamed_tool_started,
        "tool marker should not appear before streamed assistant commentary: {events:?}"
    );
    assert!(
        streamed_tool_started < durable_commentary,
        "tool marker should appear while the provider response is still streaming: {events:?}"
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let durable_assistant = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::AssistantText)
        .expect("durable assistant item");
    let durable_tool = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::ToolCall)
        .expect("durable tool item");
    assert_eq!(
        replay[durable_tool].payload_json["metadata"]["display"]["description"],
        "Searching memory."
    );
    assert!(
        durable_assistant < durable_tool,
        "replay should keep durable commentary before durable tool execution: {replay:?}"
    );
}
