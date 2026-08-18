#[tokio::test]
async fn native_provider_can_call_web_fetch_and_continue() {
    let provider = Arc::new(native_fake_provider(
        FakeCodexScenario::NativeWebFetchContinuation,
    ));
    let store = crate::test_support::test_store().await;
    store
        .record_observed_urls(
            noema_store::ObservedUrlSource::SearchResult,
            "tool_call:test_search",
            &["https://example.com/page".to_string()],
        )
        .await
        .expect("record observed URL");
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(&handle, conversation_id, "Fetch the example page.".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I read the fetched page."
    )));
    assert!(provider.requests().iter().any(|request| {
        input_tool_results(&request.input).iter().any(|result| {
            result.name == "web.fetch"
                && result.success
                && result.payload["content"] == "Test page content"
        })
    }));
}

async fn submit_a2ui_form_action(
    handle: &RuntimeHandle,
    conversation_id: &str,
    interaction_id: &str,
    context_source: &str,
    item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
    client_message_id: &str,
) -> Result<(), RuntimeError> {
    handle
        .submit_a2ui_action_with_client_message_id(
            conversation_id.to_string(),
            interaction_id.to_string(),
            1,
            "main".to_string(),
            "submit".to_string(),
            "submit".to_string(),
            Some(json!({"source": context_source, "model": {"form": {"name": "Ada"}}})),
            Some(json!({"form": {"name": "Ada"}})),
            item_tx,
            Some(client_message_id.to_string()),
        )
        .await
}

#[tokio::test]
async fn a2ui_action_resolution_emits_settled_projection_and_correlates_provider_resume() {
    let provider = Arc::new(fake_provider(FakeCodexScenario::A2UIActionContinuation));
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store.clone())
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let (initial_result, initial_events) =
        collect_turn_events(&handle, conversation_id.clone(), "show the form".to_string()).await;
    initial_result.expect("A2UI turn pauses successfully");
    let (interaction_id, pending_surface_revision) = initial_events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. } => match item.as_ref() {
                TurnTranscriptItem::A2UISurface {
                    interaction_id: Some(interaction_id),
                    surface_id,
                    interaction_revision: Some(1),
                    lifecycle,
                    has_actions: true,
                    revision,
                    ..
                } if surface_id == "main" && lifecycle == "pending" => {
                    Some((interaction_id.clone(), *revision))
                }
                _ => None,
            },
            _ => None,
        })
        .expect("pending actionable A2UI surface");

    let (forged_tx, _forged_rx) = mpsc::unbounded_channel();
    let forged = submit_a2ui_form_action(
        &handle,
        &conversation_id,
        &interaction_id,
        "forged",
        forged_tx,
        "client:a2ui-forged",
    )
        .await;
    assert!(matches!(
        forged,
        Err(RuntimeError::Protocol(message)) if message.contains("context does not match")
    ));

    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    submit_a2ui_form_action(
        &handle,
        &conversation_id,
        &interaction_id,
        "surface",
        item_tx,
        "client:a2ui-submit",
    )
        .await
        .expect("A2UI action continuation");
    let mut resolution_events = Vec::new();
    while let Ok(event) = item_rx.try_recv() {
        resolution_events.push(event);
    }

    let settled = resolution_events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. } => match item.as_ref() {
                TurnTranscriptItem::A2UISurface {
                    interaction_id: Some(id),
                    interaction_revision: Some(2),
                    lifecycle,
                    surface_id,
                    revision,
                    snapshot,
                    ..
                } if id == &interaction_id && surface_id == "main" && lifecycle == "answered" => {
                    Some((*revision, snapshot.clone()))
                }
                _ => None,
            },
            _ => None,
        })
        .expect("settled A2UI projection");
    assert_eq!(settled.0, pending_surface_revision + 1);
    assert_eq!(settled.1["data_model"], json!({"form": {"name": "Ada"}}));
    assert!(resolution_events.iter().any(|event| matches!(
        event,
        TurnStreamEvent::ConversationItem { item, .. }
            if matches!(item.as_ref(), TurnTranscriptItem::AssistantText { text } if text == "A2UI action continued")
    )));

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::A2UICard
            && item.payload_json["payload"]["interaction_id"] == interaction_id
            && item.payload_json["payload"]["interaction_revision"] == 2
            && item.payload_json["payload"]["lifecycle"] == "answered"
    }));

    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "initial presentation plus one resume: {requests:?}");
    let correlated_result = requests[1]
        .input
        .clone();
    let correlated_result = input_tool_results(&correlated_result)
        .into_iter()
        .find(|result| result.name == "noema.present_a2ui")
        .expect("correlated A2UI tool result");
    assert_eq!(correlated_result.call_id, "call_a2ui_form");
    assert_eq!(correlated_result.payload["status"], "resolved");
    assert_eq!(correlated_result.payload["interaction_id"], interaction_id);
    assert_eq!(
        correlated_result.payload["context"],
        json!({"source": "surface", "model": {"form": {"name": "Ada"}}})
    );
    assert_eq!(correlated_result.payload["data_model"], json!({"form": {"name": "Ada"}}));

    let (duplicate_tx, _duplicate_rx) = mpsc::unbounded_channel();
    let duplicate = submit_a2ui_form_action(
        &handle,
        &conversation_id,
        &interaction_id,
        "surface",
        duplicate_tx,
        "client:a2ui-duplicate",
    )
        .await;
    assert!(matches!(
        duplicate,
        Err(RuntimeError::Protocol(message)) if message.contains("A2UI interaction is stale")
    ));
    handle.shutdown().await;

    let failing_provider = Arc::new(fake_provider(FakeCodexScenario::A2UIActionResumeFailure));
    let failing_store = crate::test_support::test_store().await;
    let failing_handle = RuntimeHandle::spawn_with_provider(
        failing_provider,
        failing_store.clone(),
    )
    .await
    .expect("runtime");
    let failing_conversation_id = failing_handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (_, pending_events) = collect_turn_events(
        &failing_handle,
        failing_conversation_id.clone(),
        "show the failing form".to_string(),
    )
    .await;
    let failing_interaction_id = pending_events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. } => match item.as_ref() {
                TurnTranscriptItem::A2UISurface {
                    interaction_id: Some(interaction_id),
                    lifecycle,
                    ..
                } if lifecycle == "pending" => Some(interaction_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("pending failing A2UI interaction");
    let (failure_tx, _failure_rx) = mpsc::unbounded_channel();
    let failure = submit_a2ui_form_action(
        &failing_handle,
        &failing_conversation_id,
        &failing_interaction_id,
        "surface",
        failure_tx,
        "client:a2ui-failure",
    )
        .await;
    assert!(failure.is_err(), "provider continuation should fail");
    let failed_replay = failing_store
        .list_conversation_items(&failing_conversation_id, ReplayMode::Visible)
        .await
        .expect("failed replay");
    assert!(failed_replay.iter().any(|item| {
        item.kind == ConversationItemKind::A2UICard
            && item.payload_json["payload"]["interaction_id"] == failing_interaction_id
            && item.payload_json["payload"]["lifecycle"] == "failed"
    }));
    failing_handle.shutdown().await;
}

#[tokio::test]
async fn native_provider_can_create_local_artifact_with_two_versions_and_continue() {
    let provider = Arc::new(native_fake_provider(
        FakeCodexScenario::NativeArtifactCreateLocalFileContinuation,
    ));
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store.clone())
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Create a small artifact and revise it once.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "I created the two-version artifact."
    )));
    let artifacts = store
        .list_artifacts_for_owner(
            noema_artifacts::ArtifactOwnerRef::conversation(&conversation_id),
            10,
        )
        .await
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].versions.len(), 2);
    assert_eq!(artifacts[0].current_version.version_index, 2);
    assert!(provider.requests().iter().any(|request| {
        input_tool_results(&request.input).iter().any(|result| {
            result.name == "artifact.create_local_file"
                && result.success
                && result.payload["current_version_index"] == 2
        })
    }));
}

#[tokio::test]
async fn hard_ceiling_and_audit_failure_get_one_no_tools_finalization_attempt() {
    for scenario in [
        FakeCodexScenario::LongContinuationThenFinalization,
        FakeCodexScenario::ProgressAuditFailsThenFinalization,
    ] {
        assert_one_no_tools_finalization(scenario).await;
    }
}

async fn assert_one_no_tools_finalization(scenario: FakeCodexScenario) {
    let provider = Arc::new(FakeCodexProvider::new(scenario));
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store.clone())
        .await
        .expect("runtime");
    let codex = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    if matches!(
        scenario,
        FakeCodexScenario::LongContinuationThenFinalization
    ) {
        authenticate_provider_account(&store, &codex.provider_account_id).await;
        upsert_ready_agent_runtime_preference(
            &store,
            noema_store::NewAgentRuntimePreference {
                agent_id: "agent:primary".to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: codex.provider_account_id,
                selection: noema_providers::ModelPreferenceSelection::ExplicitProfile {
                    model_profile: "gpt-5.5".to_string(),
                    reasoning_effort: Some(noema_providers::ReasoningEffort::High),
                },
                fast_mode: false,
            },
        )
        .await;
    } else {
        upsert_ready_auxiliary_model_preference(
            &store,
            noema_store::NewAuxiliaryModelPreference {
                task: noema_store::AuxiliaryModelTask::ToolProgressAudit,
                provider_kind: "codex".to_string(),
                provider_account_id: codex.provider_account_id,
                selection: noema_providers::ModelPreferenceSelection::ExplicitProfile {
                    model_profile: "gpt-5.4-mini".to_string(),
                    reasoning_effort: None,
                },
                fast_mode: false,
            },
        )
        .await;
    }
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id,
        "Research healthy restaurants and keep going.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let requests = provider.requests();
    let finalization_requests = requests
        .iter()
        .filter(|request| {
            request.tools.is_empty()
                && !request.parallel_tool_calls
                && request
                    .instructions
                    .as_deref()
                    .is_some_and(|instructions| instructions.contains("must stop now"))
        })
        .collect::<Vec<_>>();
    assert_eq!(finalization_requests.len(), 1);
    if matches!(
        scenario,
        FakeCodexScenario::LongContinuationThenFinalization
    ) {
        assert_eq!(
            finalization_requests[0].options.reasoning_effort,
            Some(noema_providers::ReasoningEffort::High)
        );
    }
}
