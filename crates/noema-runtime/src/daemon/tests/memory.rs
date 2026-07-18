#[tokio::test]
async fn search_memory_uses_private_runtime_memory_endpoint() {
    let memory = Arc::new(RecordingMemoryOperations::returning(
        SearchMemoriesResponse {
            results: vec![noema_memory::MemoryRecord {
                id: "mem_train".to_string(),
                memory: Some("Kevin likes trains.".to_string()),
                score: Some(0.91),
                metadata: Some(json!({"source": "test"})),
                created_at: None,
                updated_at: Some("2026-07-07T12:00:00.000Z".to_string()),
            }],
        },
    ));
    let (handle, _store, _memory) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        memory,
    )
    .await;

    let conversation = handle.start_conversation(None).await.expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "I'm a big fan of trains".to_string(),
    )
    .await
    .expect("seed turn");

    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("search turn");
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Completed,
            title,
            metadata,
            ..
        } if activity_kind == "tool_result"
            && title == "Tool result: search_memory"
            && metadata["action"]["success"] == true
            && metadata["action"]["payload"]["memories"]
                .as_array()
                .is_some_and(|memories| memories.iter().any(|memory| {
                    memory["kind"] == "mnemosyne"
                        && memory["memory"] == "Kevin likes trains."
                        && memory["scope_id"]
                            .as_str()
                            .is_some_and(|scope_id| scope_id.starts_with("conversation:"))
                }))
            && metadata["action"]["payload"].get("unavailable").is_none()
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I found your train memory."
    )));
    handle.shutdown().await;
}

#[tokio::test]
async fn persisted_user_message_submits_observation_even_when_generation_fails() {
    for (scenario, text, should_fail) in [
        (FakeCodexScenario::Simple, "remember this turn", false),
        (
            FakeCodexScenario::TurnError,
            "remember even if generation fails",
            true,
        ),
    ] {
        let memory = Arc::new(RecordingMemoryOperations::default());
        let (handle, _store, memory) =
            test_runtime_handle_with_mnemosyne(fake_provider(scenario), memory).await;
        let conversation_id = handle
            .start_conversation(None)
            .await
            .expect("conversation")
            .conversation_id;
        let (result, _) =
            collect_turn_events(&handle, conversation_id.clone(), text.to_string()).await;
        assert_eq!(result.is_err(), should_fail);
        memory.wait_for_add().await;
        handle.shutdown().await;

        assert!(memory.add_requests().await.iter().any(|request| {
            request.user_id == "human:local"
                && request.agent_id.as_deref() == Some("agent:local")
                && request.run_id.as_deref() == Some(conversation_id.as_str())
                && request.metadata["noemaConversationId"] == conversation_id
                && request.metadata["sourceKind"] == "user_message"
                && request.metadata["turnId"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("turn:"))
                && request.metadata["userItemId"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("item:"))
                && request.messages
                    == vec![noema_memory::MemoryMessage {
                        role: "user".to_string(),
                        content: text.to_string(),
                    }]
        }));
    }
}

#[tokio::test]
async fn memory_observation_waits_for_the_foreground_turn_to_finish() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider = BlockingOnceProvider {
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(Some(release_rx)),
    };
    let store = crate::test_support::test_store().await;
    let memory = Arc::new(RecordingMemoryOperations::default());
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(provider),
        store,
        Some(memory.clone()),
    )
    .await
    .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let turn_handle = handle.clone();
    let pending_turn = tokio::spawn(async move {
        collect_turn_events(
            &turn_handle,
            conversation_id,
            "remember after replying".to_string(),
        )
        .await
    });

    started_rx.await.expect("foreground provider started");
    assert!(
        tokio::time::timeout(Duration::from_millis(50), memory.wait_for_add())
            .await
            .is_err(),
        "memory observation should remain blocked while the foreground provider is active"
    );

    release_tx.send(()).expect("release foreground provider");
    pending_turn
        .await
        .expect("turn task")
        .0
        .expect("foreground turn");
    memory.wait_for_add().await;
    handle.shutdown().await;
}

#[tokio::test]
async fn slow_memory_ingest_does_not_delay_provider_response() {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    store.ensure_default_actors().await.expect("actors");
    let memory = Arc::new(RecordingMemoryOperations::blocking_add());
    std::mem::forget(home);
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(fake_provider(FakeCodexScenario::Simple)),
        store,
        Some(memory),
    )
    .await
    .expect("runtime");

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let result = tokio::time::timeout(
        Duration::from_millis(500),
        collect_turn_events(
            &handle,
            conversation_id,
            "do not wait for memory indexing".to_string(),
        ),
    )
    .await
    .expect("turn should not wait for Mnemosyne response")
    .0;
    handle.shutdown().await;

    result.expect("turn");
}
