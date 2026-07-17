#[tokio::test]
async fn allowed_tools_keep_native_catalog_stable_across_continuation() {
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeSearchMemoryContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::Native,
                parallel_tool_calls: true,
                tool_choice: true,
                allowed_tools: true,
                schema_dialect: noema_providers::ProviderToolSchemaDialect::OpenAiResponses,
                strict_schema: false,
                custom_tools: false,
                native_tool_results: true,
                prompt_cache_retention: true,
                prompt_cache_key: true,
                prompt_cache_options: false,
                prompt_cache_breakpoints: false,
                encrypted_reasoning: false,
            })
            .with_response_continuation(ProviderResponseContinuation::PreviousResponseId {
                store_response: true,
            }),
    );
    let (handle, _store, _server) = spawn_runtime_with_memory_provider(
        provider.clone(),
        json!({"results": [{"memory": "Kevin likes trains."}]}),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    let initial_catalog = requests[0]
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<Vec<_>>();
    let continuation_catalog = requests[1]
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(initial_catalog, continuation_catalog);
    let noema_providers::NoemaToolChoice::Allowed(allowed) = &requests[1].tool_choice else {
        panic!("expected an allowed-tools restriction");
    };
    assert!(
        allowed
            .tools
            .iter()
            .any(|tool| tool.as_str() == "search_memory")
    );
    assert!(
        allowed
            .tools
            .iter()
            .all(|tool| tool.as_str() != "update_own_name")
    );
}

#[tokio::test]
async fn rejected_response_chain_falls_back_to_complete_local_replay() {
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
            })
            .rejecting_first_chained_request(),
    );
    let (handle, _store, _server) = spawn_runtime_with_memory_provider(
        provider.clone(),
        json!({"results": [{"memory": "Kevin likes trains."}]}),
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
    .expect("turn succeeds after replay fallback");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "native tool result received"
    )));
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[1].options.previous_response_id.as_deref(),
        Some("resp_1")
    );
    assert!(matches!(requests[1].input, GenerateInput::Items(_)));
    assert_eq!(input_tool_results(&requests[1].input).len(), 1);
    assert!(requests[2].options.previous_response_id.is_none());
    assert!(matches!(requests[2].input, GenerateInput::Items(_)));
    assert_eq!(input_tool_results(&requests[2].input).len(), 1);
}

#[tokio::test]
async fn web_search_result_is_sent_as_native_tool_result_input() {
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
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeWebSearchContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::Native,
                parallel_tool_calls: true,
                native_tool_results: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                ..ProviderToolCapabilities::default()
            }),
    );
    let (handle, _store) =
        test_runtime_handle_with_search_provider(provider.clone(), search_provider).await;
    let conversation = handle.start_conversation(None).await.expect("conversation");

    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Search for Rust.".to_string(),
    )
    .await
    .expect("turn");

    let requests = provider.requests();
    assert!(requests.iter().any(|request| {
        let results = input_tool_results(&request.input);
        results.iter().any(|result| {
            result.name == "web.search"
                && result.success
                && result.payload["provider"] == "duckduckgo_public"
                && result
                    .payload
                    .get("results")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|results| !results.is_empty())
        })
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn native_provider_can_call_web_fetch_and_continue() {
    let fetch_response = noema_capabilities::web::fetch::FetchResponse {
        provider: crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID.to_string(),
        url: String::new(),
        final_url: "https://example.com/page".to_string(),
        title: Some("Example Page".to_string()),
        format: "markdown".to_string(),
        extraction: crate::web_fetch::types::EXTRACTION_READABILITYRS.to_string(),
        content_kind: noema_capabilities::web::fetch::FetchContentKind::RawMarkdown,
        content: "Example fetched page content.".to_string(),
        raw_excerpt: None,
        raw_chars: 29,
        returned_chars: 29,
        summary_model: None,
        summary_strategy: noema_capabilities::web::fetch::FetchSummaryStrategy::NotSummarized,
        truncated: false,
    };
    let web_fetch_provider = static_web_fetch_backend(fetch_response);
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::NativeWebFetchContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::Native,
                parallel_tool_calls: true,
                native_tool_results: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                ..ProviderToolCapabilities::default()
            }),
    );
    let (handle, _store) =
        test_runtime_handle_with_search_and_fetch_providers(provider.clone(), web_fetch_provider)
            .await;
    let conversation = handle.start_conversation(None).await.expect("conversation");

    let items = collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Fetch the example page.".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I read the fetched page."
    )));
    let requests = provider.requests();
    assert!(requests.iter().any(|request| {
        let results = input_tool_results(&request.input);
        results.iter().any(|result| {
            result.name == "web.fetch"
                && result.success
                && result.payload["provider"] == crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID
                && result.payload["content"] == "Example fetched page content."
        })
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn native_provider_can_create_local_artifact_with_two_versions_and_continue() {
    let provider = Arc::new(
        RecordingFakeProvider::new(
            "codex",
            FakeCodexScenario::NativeArtifactCreateLocalFileContinuation,
        )
        .with_tool_capabilities(ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            native_tool_results: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            ..ProviderToolCapabilities::default()
        }),
    );
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::test_support::test_store_for_paths(&paths).await;
    std::mem::forget(home);
    let handle = RuntimeHandle::spawn_with_provider(provider.clone(), store.clone())
        .await
        .expect("runtime");
    let conversation = handle.start_conversation(None).await.expect("conversation");

    let items = collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Create a small artifact and revise it once.".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "I created the two-version artifact."
    )));
    let artifacts = store
        .list_artifacts_for_owner(
            noema_artifacts::ArtifactOwnerRef::conversation(&conversation.conversation_id),
            10,
        )
        .await
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    let artifact = &artifacts[0];
    assert_eq!(artifact.artifact.title, "Agent artifact smoke note");
    assert_eq!(artifact.versions.len(), 2);
    assert_eq!(artifact.current_version.version_index, 2);
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::ArtifactReference {
            artifact_id,
            artifact_version_id,
            title,
            storage_kind,
            download_url: Some(download_url),
            ..
        } if artifact_id == &artifact.artifact.artifact_id
            && artifact_version_id.as_deref() == Some(artifact.current_version.artifact_version_id.as_str())
            && title == "Agent artifact smoke note"
            && storage_kind == "local_file"
            && download_url.ends_with("/download")
    )));
    let requests = provider.requests();
    assert!(requests.iter().any(|request| {
        let results = input_tool_results(&request.input);
        results.iter().any(|result| {
            result.name == "artifact.create_local_file"
                && result.success
                && result.payload["current_version_index"] == 2
        })
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_actor_continues_after_continuation_tool_call() {
    let provider = Arc::new(
        RecordingFakeProvider::new("codex", FakeCodexScenario::ChainedSearchMemoryContinuation)
            .with_tool_capabilities(ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::Native,
                parallel_tool_calls: true,
                native_tool_results: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                ..ProviderToolCapabilities::default()
            }),
    );
    let (handle, _store, _server) =
        spawn_runtime_with_memory_provider(provider.clone(), json!({"results": []})).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Check memory twice before answering.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let completed_search_results = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "tool_result" && title == "Tool result: search_memory"
            )
        })
        .count();
    assert_eq!(completed_search_results, 2, "{items:?}");
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I checked both memory topics."
    )));
    let requests = provider.requests();
    assert_eq!(requests.len(), 3, "expected two ordered continuations");
    let final_input = &requests[2].input;
    let results = input_tool_results(final_input);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].call_id, "call_1");
    assert_eq!(results[1].call_id, "call_2");
    let rendered = final_input.render_for_token_count();
    assert!(rendered.contains("Checking memory first."));
    assert!(rendered.contains("I need one more memory check."));
}

#[tokio::test]
async fn search_memory_uses_private_runtime_memory_endpoint() {
    let (handle, _store, server) = test_runtime_handle_with_private_memory(
        fake_provider(FakeCodexScenario::SearchMemoryContinuation),
        json!({
            "results": [{
                "id": "mem_private",
                "memory": "Kevin prefers private sidecars",
                "updated_at": "2026-07-08T12:00:00.000Z",
                "score": 0.94
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

    assert!(
        items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_result"
                    && title == "Tool result: search_memory"
                    && metadata["action"]["payload"]["memories"]
                        .as_array()
                        .is_some_and(|memories| memories.iter().any(|memory| {
                            memory["id"] == "mem_private"
                        }))
            )
        }),
        "expected private memory result in tool output"
    );
    assert!(!server.request_bodies().await.is_empty());
}

#[tokio::test]
async fn hard_ceiling_gets_one_no_tools_finalization_attempt() {
    let provider = Arc::new(RecordingFakeProvider::new(
        "codex",
        FakeCodexScenario::LongContinuationThenFinalization,
    ));
    let (handle, store) = test_runtime_handle_with_search_provider(
        provider.clone(),
        static_web_search_backend(noema_capabilities::web::search::SearchResponse {
            provider: "test".to_string(),
            provider_contract: "test".to_string(),
            query: "restaurants".to_string(),
            summary: "Found 1 test result".to_string(),
            results: vec![],
        }),
    )
    .await;
    store.ensure_default_actors().await.expect("actors");
    let codex = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    authenticate_provider_account(&store, &codex.provider_account_id).await;
    upsert_ready_agent_runtime_preference(
        &store,
        noema_store::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: codex.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(noema_providers::ReasoningEffort::High),
        },
    )
    .await;
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
    let finalization_requests: Vec<_> = requests
        .iter()
        .filter(|request| {
            request.tools.is_empty()
                && !request.parallel_tool_calls
                && request
                    .instructions
                    .as_deref()
                    .is_some_and(|instructions| instructions.contains("must stop now"))
        })
        .collect();
    assert_eq!(finalization_requests.len(), 1);
    assert_eq!(
        finalization_requests[0].options.reasoning_effort,
        Some(noema_providers::ReasoningEffort::High)
    );
}

#[tokio::test]
async fn audit_execution_failure_gets_one_no_tools_finalization_attempt() {
    let provider = Arc::new(RecordingFakeProvider::new(
        "codex",
        FakeCodexScenario::ProgressAuditFailsThenFinalization,
    ));
    let (handle, store) = test_runtime_handle_with_search_provider(
        provider.clone(),
        static_web_search_backend(noema_capabilities::web::search::SearchResponse {
            provider: "test".to_string(),
            provider_contract: "test".to_string(),
            query: "restaurants".to_string(),
            summary: "Found 1 test result".to_string(),
            results: vec![],
        }),
    )
    .await;
    let codex = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    upsert_ready_auxiliary_model_preference(
        &store,
        noema_store::NewAuxiliaryModelPreference {
            task_id: noema_store::TOOL_PROGRESS_AUDIT_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: codex.provider_account_id,
            model_profile: "gpt-5.4-mini".to_string(),
            reasoning_effort: None,
        },
    )
    .await;
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
        .count();
    assert_eq!(finalization_requests, 1);
}

#[tokio::test]
async fn update_own_name_tool_updates_agent_and_continues_turn() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::UpdateOwnNameContinuation))
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
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
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
            && title == "Tool result: update_own_name"
            && metadata["action"]["success"] == true
            && metadata["action"]["payload"]["display_name"] == "Fred"
    )));
    assert!(
        items.iter().any(|item| matches!(
            item,
            TurnTranscriptItem::AssistantText { text }
                if text == "Fred it is. what would you like help with first?"
        )),
        "unexpected continuation transcript: {items:?}"
    );
}
