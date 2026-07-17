#[tokio::test]
async fn runtime_actor_executes_search_memory_as_local_tool_result() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [{
                "id": "mem_train",
                "memory": "Kevin likes trains.",
                "metadata": {"source": "test"},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.91
            }],
            "timing": 2,
            "total": 1
        }),
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
async fn user_message_submits_memory_observation() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::Simple),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (result, _events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "remember this turn".to_string(),
    )
    .await;
    result.expect("turn");
    wait_for_memory_observation_requests(&server, 1).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    assert!(bodies.iter().any(|body| {
        body["user_id"] == "human:local"
            && body["agent_id"] == "agent:local"
            && body["run_id"] == conversation_id
            && body["metadata"]["noemaConversationId"] == conversation_id
            && body["metadata"]["sourceKind"] == "user_message"
            && body["metadata"]["turnId"]
                .as_str()
                .is_some_and(|turn_id| turn_id.starts_with("turn:"))
            && body["metadata"]["userItemId"]
                .as_str()
                .is_some_and(|item_id| item_id.starts_with("item:"))
            && body["messages"].as_array().is_some_and(|messages| {
                messages.as_slice() == [json!({"role": "user", "content": "remember this turn"})]
            })
    }));
}

#[tokio::test]
async fn provider_failure_after_user_message_still_submits_memory_observation() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::TurnError),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (result, _events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "remember even if generation fails".to_string(),
    )
    .await;
    assert!(result.is_err());
    wait_for_memory_observation_requests(&server, 1).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    assert!(bodies.iter().any(|body| {
        body["messages"]
            == json!([{"role": "user", "content": "remember even if generation fails"}])
    }));
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
    let server = FakeMemoryServer::start(json!({"results": []}), 1).await;
    let memory_operations =
        crate::test_support::mnemosyne_operations_for_base_url(server.base_url());
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(provider),
        store,
        Some(memory_operations),
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
        tokio::time::timeout(Duration::from_millis(50), server.wait_for_request())
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
    wait_for_memory_observation_requests(&server, 1).await;
    handle.shutdown().await;
}

#[tokio::test]
async fn slow_memory_ingest_does_not_delay_provider_response() {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    store.ensure_default_actors().await.expect("actors");
    let server = FakeMemoryServer::start_with_add_delay(
        json!({"results": []}),
        1,
        Some(Duration::from_secs(5)),
    )
    .await;
    std::mem::forget(home);
    let memory_operations =
        crate::test_support::mnemosyne_operations_for_base_url(server.base_url());
    let handle = RuntimeHandle::spawn_with_provider_and_memory(
        Arc::new(fake_provider(FakeCodexScenario::Simple)),
        store,
        Some(memory_operations),
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

#[tokio::test]
async fn memory_observation_uses_distinct_source_ids_and_current_user_source_observation() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::Simple),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn_events(
        &handle,
        conversation_id.clone(),
        "first turn should not repeat".to_string(),
    )
    .await
    .0
    .expect("first turn");
    collect_turn_events(
        &handle,
        conversation_id.clone(),
        "second turn should be submitted".to_string(),
    )
    .await
    .0
    .expect("second turn");
    wait_for_memory_observation_requests(&server, 2).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    let observation_bodies = bodies
        .iter()
        .filter(|body| body["metadata"]["noemaConversationId"] == conversation_id)
        .collect::<Vec<_>>();
    assert_eq!(observation_bodies.len(), 2);
    let source_item_ids = observation_bodies
        .iter()
        .filter_map(|body| body["metadata"]["userItemId"].as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(source_item_ids.len(), 2);
    assert!(
        source_item_ids
            .iter()
            .all(|source_item_id| source_item_id.starts_with("item:"))
    );
    assert!(observation_bodies.iter().any(|body| {
        body["metadata"]["sourceObservation"] == "first turn should not repeat"
            && body["messages"]
                == json!([{"role": "user", "content": "first turn should not repeat"}])
    }));
    assert!(observation_bodies.iter().any(|body| {
        body["metadata"]["sourceObservation"] == "second turn should be submitted"
            && body["messages"].as_array().is_some_and(|messages| {
                messages.last()
                    == Some(&json!({"role": "user", "content": "second turn should be submitted"}))
            })
    }));
}

#[tokio::test]
async fn memory_observation_includes_assistant_context_since_previous_user() {
    let (handle, _store, server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::MemoryContextQuestion),
        json!({"results": []}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn_events(
        &handle,
        conversation_id.clone(),
        "start memory context test".to_string(),
    )
    .await
    .0
    .expect("first turn");
    collect_turn_events(&handle, conversation_id.clone(), "cars".to_string())
        .await
        .0
        .expect("second turn");
    wait_for_memory_observation_requests(&server, 2).await;
    handle.shutdown().await;

    let bodies = server.request_bodies().await;
    let observation = bodies
        .iter()
        .find(|body| body["metadata"]["sourceObservation"] == "cars")
        .expect("cars observation");
    assert_eq!(
        observation["messages"],
        json!([
            {
                "role": "assistant",
                "content": "what are some topics you find interesting?"
            },
            {
                "role": "assistant",
                "content": "short answers are fine too"
            },
            {"role": "user", "content": "cars"}
        ])
    );
}

#[tokio::test]
async fn search_memory_skips_mnemosyne_results_without_memory() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [
                {
                    "id": "mem_missing_text",
                    "metadata": {"source": "test"},
                    "updated_at": "2026-07-07T12:00:00.000Z",
                    "score": 0.99
                },
                {
                    "id": "mem_train",
                    "memory": "Kevin likes trains.",
                    "metadata": {"source": "test"},
                    "updated_at": "2026-07-07T12:00:00.000Z",
                    "score": 0.91
                }
            ],
            "timing": 2,
            "total": 2
        }),
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
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("search turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "tool_result" && title == "Tool result: search_memory" => {
                Some(&metadata["action"]["payload"])
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("search_memory tool result, got {items:?}"));
    let memories = payload["memories"].as_array().expect("memories");
    assert!(!memories.is_empty());
    assert!(memories.iter().all(|memory| {
        memory["id"] == "mem_train" && memory["memory"] == "Kevin likes trains."
    }));
}

#[tokio::test]
async fn search_memory_omits_raw_mnemosyne_metadata() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [{
                "id": "mem_train",
                "memory": "Kevin likes trains.",
                "metadata": {"raw": "secret provider detail"},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.91
            }],
            "timing": 2,
            "total": 1
        }),
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
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("search turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "tool_result" && title == "Tool result: search_memory" => {
                Some(&metadata["action"]["payload"])
            }
            _ => None,
        })
        .expect("search_memory tool result");
    assert!(payload["memories"][0].get("metadata").is_none());
}

#[tokio::test]
async fn search_memory_returns_sanitized_mnemosyne_failure() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({"results": [{}]}),
    )
    .await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id,
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
                title,
                metadata,
                ..
            } if activity_kind == "tool_result"
                && title == "Tool result: search_memory"
                && metadata["action"]["success"] == false =>
            {
                Some(&metadata["action"]["payload"])
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("failed search_memory tool result, got {items:?}"));
    assert_eq!(
        payload,
        &json!({
            "error": {
                "code": "decode_failed",
                "message": "memory service returned an unreadable response"
            }
        })
    );
}

#[tokio::test]
async fn runtime_actor_executes_web_search_as_local_tool_result() {
    let search_provider =
        static_web_search_backend(noema_capabilities::web::search::SearchResponse {
            provider: "duckduckgo_public".to_string(),
            provider_contract: "best_effort_public".to_string(),
            query: String::new(),
            summary: "Found 1 web result".to_string(),
            results: vec![noema_capabilities::web::search::SearchResult {
                rank: 1,
                title: "Rust Programming Language".to_string(),
                url: "https://www.rust-lang.org/".to_string(),
                snippet: "A language empowering everyone to build reliable software.".to_string(),
            }],
        });
    let (handle, store) = test_runtime_handle_with_search_provider(
        Arc::new(fake_provider(FakeCodexScenario::NativeWebSearch)),
        search_provider,
    )
    .await;
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Search the web for Rust.".to_string(),
    )
    .await
    .expect("turn");

    let items = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Audit)
        .await
        .expect("items");

    assert!(items.iter().any(|item| {
        matches!(item.kind, ConversationItemKind::ToolCall)
            && item.payload_json["metadata"]["action"]["name"] == "web.search"
            && item.payload_json["metadata"]["display"]["target"] == "rust language"
    }));
    assert!(items.iter().any(|item| {
        matches!(item.kind, ConversationItemKind::ToolResult)
            && item.payload_json["metadata"]["action"]["name"] == "web.search"
            && item.payload_json["metadata"]["action"]["success"] == true
            && item.payload_json["metadata"]["action"]["payload"]["provider"] == "duckduckgo_public"
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn native_capable_provider_continuation_uses_native_tool_result_input() {
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeSearchMemoryContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::Native,
                parallel_tool_calls: true,
                tool_choice: true,
                allowed_tools: false,
                schema_dialect: noema_providers::ProviderToolSchemaDialect::OpenAiResponses,
                strict_schema: false,
                custom_tools: false,
                native_tool_results: true,
                prompt_cache_retention: true,
                prompt_cache_key: false,
                prompt_cache_options: false,
                prompt_cache_breakpoints: false,
                encrypted_reasoning: false,
            })
            .with_response_continuation(ProviderResponseContinuation::PreviousResponseId {
                store_response: false,
            }),
    );
    let (handle, _store, _server) = spawn_runtime_with_memory_provider(
        provider.clone(),
        json!({
            "results": [{
                "id": "mem_native_train",
                "memory": "Kevin likes trains.",
                "metadata": {},
                "updated_at": "2026-07-07T12:00:00.000Z",
                "score": 0.9
            }]
        }),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    assert!(
        items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::AssistantText { text }
                    if text == "native tool result received"
            )
        }),
        "expected native continuation final answer: {items:?}"
    );
    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected initial request and continuation"
    );
    let results = input_tool_results(&requests[1].input);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id.as_deref(), Some("item_native_1"));
    assert_eq!(results[0].call_id, "call_native_1");
    assert_eq!(results[0].name, "search_memory");
    assert_eq!(results[0].provider_name.as_deref(), Some("search_memory"));
    assert_eq!(results[0].arguments["arguments"]["query"], "trains");
    assert!(results[0].success);
    assert_eq!(
        requests[1].options.previous_response_id.as_deref(),
        Some("resp_1")
    );
    assert!(!requests[1].options.store_response);
    assert!(
        !requests[1]
            .input
            .render_for_token_count()
            .contains("NOEMA_LOCAL_TOOL_RESULT")
    );
}
