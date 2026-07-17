#[tokio::test]
async fn update_own_name_tool_does_not_start_repeated_continuation_tool_calls() {
    let (handle, store) = test_runtime_handle_with_store(fake_provider(
        FakeCodexScenario::RepeatedUpdateOwnNameContinuation,
    ))
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "Hey! How about Fred?".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Fred"));

    let update_name_tool_calls = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_call"
                    && title == "Tool call: update_own_name"
                    && metadata.get("provider").and_then(serde_json::Value::as_str) == Some("noema_local")
            )
        })
        .count();
    assert_eq!(update_name_tool_calls, 1, "{items:?}");
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "Fred it is."
    )));
}

#[tokio::test]
async fn update_own_name_tool_history_is_visible_before_later_turns() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::UpdateOwnNameThenYay))
            .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(
        &handle,
        conversation_id.clone(),
        "Let's rename you to Momo".to_string(),
    )
    .await
    .expect("rename turn");
    let items = collect_turn(&handle, conversation_id, "Yay".to_string())
        .await
        .expect("yay turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Momo"));
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "yay acknowledged after saved name"
    )));
}

#[tokio::test]
async fn ambiguous_name_suggestion_asks_confirmation_without_tool_call() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::AmbiguousUpdateOwnName))
            .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id,
        "Maybe you could be Mira?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name, None);
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "Please confirm what you'd like to call me."
    )));
}

#[tokio::test]
async fn runtime_prompt_includes_stored_agent_name_after_update() {
    let handle = test_runtime_handle(fake_provider(
        FakeCodexScenario::UpdateOwnNameThenIdentityCheck,
    ))
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(
        &handle,
        conversation_id.clone(),
        "Your name is Mira.".to_string(),
    )
    .await
    .expect("name turn");
    let items = collect_turn(&handle, conversation_id, "What is your name?".to_string())
        .await
        .expect("identity turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw stored identity"
    )));
}

#[tokio::test]
async fn search_memory_profile_continuation_uses_scoped_empty_query() {
    let (handle, store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryProfileContinuation),
        json!({
            "results": [{
                "id": "mem_plane",
                "memory": "Kevin likes planes.",
                "metadata": {"source": "test"},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.93
            }]
        }),
    )
    .await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like planes.".to_string(),
    )
    .await
    .expect("seed turn");

    collect_turn(
        &handle,
        conversation_id.clone(),
        "What memories do you have of me?".to_string(),
    )
    .await
    .expect("profile search turn");
    handle.shutdown().await;

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        replay.iter().any(|item| {
            item.kind == ConversationItemKind::ToolResult
                && item.status == ConversationItemStatus::Completed
                && item.payload_json["metadata"]["action"]["success"] == true
                && item.payload_json["metadata"]["action"]["payload"]["scope_ids"]
                    == json!(["human:local"])
                && item.payload_json["metadata"]["action"]["payload"]["memories"]
                    .as_array()
                    .is_some_and(|memories| {
                        memories.iter().any(|memory| {
                            memory["kind"] == "mnemosyne"
                                && memory["memory"] == "Kevin likes planes."
                                && memory["scope_id"] == "human:local"
                        })
                    })
        }),
        "expected persisted successful scoped profile tool result, got {replay:?}"
    );
    let request_paths = server.request_paths().await;
    assert!(
        request_paths
            .iter()
            .any(|path| path == "/v1/memories?user_id=human%3Alocal&limit=8"),
        "expected scoped empty-query memory list, got {request_paths:?}"
    );
}

#[tokio::test]
async fn search_memory_uses_runtime_connection_until_restart() {
    let second_server = FakeMemoryServer::start(
        json!({"results": [{"id": "new", "memory": "new memory", "score": 0.9}]}),
        32,
    )
    .await;
    let (handle, store, first_server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({"results": [{"id": "old", "memory": "old memory", "score": 0.1}]}),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("first turn");
    store
        .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
            mode: noema_memory::MemoryServiceMode::External,
            base_url: Some(second_server.base_url()),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect("save second settings");
    let second_conversation_id = handle
        .start_conversation(None)
        .await
        .expect("second conversation")
        .conversation_id;

    collect_turn(
        &handle,
        second_conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("second turn");
    handle.shutdown().await;

    assert!(!first_server.request_bodies().await.is_empty());
    assert!(second_server.request_bodies().await.is_empty());
}

#[tokio::test]
async fn search_memory_tool_returns_empty_mnemosyne_result_without_unavailable() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                metadata,
                ..
            } if activity_kind == "tool_result" => Some(&metadata["action"]["payload"]),
            _ => None,
        })
        .expect("tool result payload");
    assert_eq!(payload["omissions"], json!([]));
    assert!(payload.get("unavailable").is_none());
    assert!(payload.get("context_packet_id").is_none());
    assert!(
        payload["memories"]
            .as_array()
            .is_some_and(|memories| memories.is_empty())
    );
}

#[tokio::test]
async fn search_memory_tool_invalid_arguments_are_failed_tool_result() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::InvalidSearchMemory)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                metadata,
                ..
            } if activity_kind == "tool_result" => Some(&metadata["action"]["payload"]),
            _ => None,
        })
        .expect("failed tool result payload");
    assert_eq!(payload["error"], "unsupported purpose: dump_everything");
}
