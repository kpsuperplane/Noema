#[tokio::test]
async fn runtime_actor_allocates_distinct_conversation_ids() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::Simple)).await;

    let first = handle
        .start_conversation(None)
        .await
        .expect("first conversation");
    let second = handle
        .start_conversation(None)
        .await
        .expect("second conversation");

    assert_ne!(first.conversation_id, second.conversation_id);

    let items = collect_turn(&handle, first.conversation_id.clone(), "hello".to_string())
        .await
        .expect("turn response");
    assert_eq!(assistant_text(&items), "fake answer");

    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_handle_generate_once_uses_provider_without_conversation() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::Simple)).await;

    let response = handle
        .generate_once(GenerateRequest::text("hello"))
        .await
        .expect("generate once");
    handle.shutdown().await;

    assert_eq!(response.assistant_text(), "fake answer");
}

#[tokio::test]
async fn task_completion_delivery_writes_primary_assistant_item_without_human_input() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::Simple)).await;
    let conversation = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let request = super::runtime::TaskCompletionDeliveryRequest {
        delivery_id: "event:completion".to_string(),
        task_id: "task:completion".to_string(),
        conversation_id: conversation.clone(),
        source_item_id: None,
        title: "Research the result".to_string(),
        status: "completed".to_string(),
        request_markdown: "Find the result".to_string(),
        summary: Some("The result is ready.".to_string()),
        result_markdown: Some("A durable result.".to_string()),
        artifacts: vec![super::runtime::TaskCompletionArtifact {
            artifact_id: "artifact:task-report".to_string(),
            artifact_version_id: "artifact_version:task-report".to_string(),
            title: "Task report".to_string(),
            artifact_kind: "document".to_string(),
            storage_kind: "local_file".to_string(),
            external_url: None,
            download_url: Some("/artifacts/versions/task-report/download".to_string()),
            media_type: Some("text/markdown".to_string()),
        }],
        review_feedback: Some("All criteria passed.".to_string()),
        criteria: Vec::new(),
        detail: None,
    };
    handle
        .deliver_task_completion(request.clone())
        .await
        .expect("completion delivery");
    handle
        .deliver_task_completion(request)
        .await
        .expect("idempotent completion delivery");

    let items = store
        .list_conversation_items(&conversation, ReplayMode::Audit)
        .await
        .expect("conversation items");
    assert!(items.iter().any(|item| {
        item.item_id == "item:task_completion:event:completion"
            && item.kind == ConversationItemKind::AssistantText
            && item.content_text.as_deref() == Some("fake answer")
    }));
    assert!(
        !items
            .iter()
            .any(|item| item.kind == ConversationItemKind::UserText)
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| item.item_id == "item:task_completion:event:completion")
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| item.kind == ConversationItemKind::ArtifactReference)
            .count(),
        1
    );

    handle.shutdown().await;
}

#[tokio::test]
async fn blocked_task_completion_generation_does_not_block_a_primary_turn() {
    let (completion_started_tx, completion_started_rx) = oneshot::channel();
    let (primary_started_tx, primary_started_rx) = oneshot::channel();
    let (release_completion_tx, release_completion_rx) = oneshot::channel();
    let provider = BlockingTaskCompletionProvider {
        completion_started: Mutex::new(Some(completion_started_tx)),
        primary_started: Mutex::new(Some(primary_started_tx)),
        release_completion: Mutex::new(Some(release_completion_rx)),
    };
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(Arc::new(provider), store.clone())
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let completion_handle = handle.clone();
    let completion_conversation_id = conversation_id.clone();
    let pending_completion = tokio::spawn(async move {
        completion_handle
            .deliver_task_completion(super::runtime::TaskCompletionDeliveryRequest {
                delivery_id: "event:blocked-completion".to_string(),
                task_id: "task:blocked-completion".to_string(),
                conversation_id: completion_conversation_id,
                source_item_id: None,
                title: "Blocked completion".to_string(),
                status: "completed".to_string(),
                request_markdown: "Complete in the background".to_string(),
                summary: Some("Background work finished".to_string()),
                result_markdown: None,
                artifacts: Vec::new(),
                review_feedback: None,
                criteria: Vec::new(),
                detail: None,
            })
            .await
    });

    completion_started_rx
        .await
        .expect("completion provider started");
    let turn_handle = handle.clone();
    let turn_conversation_id = conversation_id.clone();
    let pending_turn = tokio::spawn(async move {
        collect_turn(
            &turn_handle,
            turn_conversation_id,
            "foreground question".to_string(),
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(1), primary_started_rx)
        .await
        .expect("primary turn should reach the provider while completion generation is blocked")
        .expect("primary provider started");
    release_completion_tx
        .send(())
        .expect("release completion provider");

    let turn_items = pending_turn
        .await
        .expect("turn task")
        .expect("primary turn");
    pending_completion
        .await
        .expect("completion task")
        .expect("completion delivery");
    let transcript = store
        .list_conversation_items(&conversation_id, ReplayMode::Audit)
        .await
        .expect("conversation items");
    handle.shutdown().await;

    assert_eq!(assistant_text(&turn_items), "foreground answer");
    assert!(transcript.iter().any(|item| {
        item.item_id == "item:task_completion:event:blocked-completion"
            && item.content_text.as_deref() == Some("completion answer")
    }));
}

#[tokio::test]
async fn runtime_handle_generate_once_does_not_block_subsequent_commands() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider = BlockingOnceProvider {
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(Some(release_rx)),
    };
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(Arc::new(provider), store)
        .await
        .expect("runtime");
    let generate_handle = handle.clone();
    let pending_generate = tokio::spawn(async move {
        generate_handle
            .generate_once(GenerateRequest::text("slow"))
            .await
    });

    started_rx.await.expect("provider started");
    let start_result =
        tokio::time::timeout(Duration::from_millis(100), handle.start_conversation(None)).await;
    let _ = release_tx.send(());
    let generated = pending_generate
        .await
        .expect("generate task")
        .expect("generate result");
    handle.shutdown().await;

    let conversation = start_result
        .expect("start_conversation should not wait for generate_once provider completion")
        .expect("conversation");
    assert!(conversation.conversation_id.starts_with("conversation:"));
    assert_eq!(generated.assistant_text(), "slow answer");
}

#[tokio::test]
async fn task_supervisor_starts_distinct_tasks_concurrently() {
    let store = crate::test_support::test_store().await;
    let (_, first_run) = crate::test_support::seed_task(&store, "Concurrent task one").await;
    let (_, second_run) = crate::test_support::seed_task(&store, "Concurrent task two").await;
    let (started_tx, mut started_rx) = mpsc::unbounded_channel();
    let runtime = RuntimeHandle::spawn_with_provider(
        Arc::new(ConcurrentTaskProvider {
            started: started_tx,
        }),
        store.clone(),
    )
    .await
    .expect("runtime");
    let subscriptions = crate::daemon::RuntimeEventRegistry::default();
    let task_runtime = TaskRuntimeHandle::start(
        store.clone(),
        runtime.clone(),
        crate::test_support::ready_test_provider_registry(),
        crate::test_support::system_error_logger(),
        subscriptions,
    );

    let first_started = tokio::time::timeout(Duration::from_secs(2), started_rx.recv())
        .await
        .expect("first task should start")
        .expect("first task id");
    let second_started = tokio::time::timeout(Duration::from_secs(2), started_rx.recv())
        .await
        .expect("second task should start before the first finishes")
        .expect("second task id");

    task_runtime.shutdown().await;
    runtime.shutdown().await;

    assert_ne!(first_started, second_started);
    assert!([first_started.as_str(), second_started.as_str()].contains(&first_run.run_id.as_str()));
    assert!(
        [first_started.as_str(), second_started.as_str()].contains(&second_run.run_id.as_str())
    );
}

#[tokio::test]
async fn background_task_pins_local_provider_generation_across_replacement() {
    let store = crate::test_support::test_store().await;
    let (task, run) = crate::test_support::seed_task(&store, "Pinned provider generation").await;
    let lease_token = "lease:provider-generation";
    let provider_registry = crate::test_support::ready_test_provider_registry();
    let claimed = store
        .claim_next_agent_run_with_readiness(
            "worker:provider-generation",
            lease_token,
            120,
            provider_registry.as_ref(),
        )
        .await
        .expect("claim run")
        .expect("leased run");
    assert_eq!(claimed.run_id, run.run_id);
    store
        .transition_agent_run(
            &run.run_id,
            noema_tasks::RunStatus::Running,
            Some(lease_token),
            None,
        )
        .await
        .expect("running run");

    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let old_provider = Arc::new(BlockingBackgroundGenerationProvider {
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(Some(release_rx)),
        requests: Mutex::new(Vec::new()),
    });
    let replacement = Arc::new(CapturingProvider::default());
    let provider_key = noema_providers::ProviderInstanceKey::new("local-model:test-generation")
        .expect("provider key");
    let provider_registry = Arc::new(noema_providers::ProviderRegistry::new());
    provider_registry
        .register(
            provider_key.clone(),
            old_provider.clone() as noema_providers::ProviderHandle,
        )
        .expect("register old provider");
    let runtime = RuntimeHandle::spawn_with_provider_registry_and_memory(
        provider_registry.clone(),
        store.clone(),
        crate::test_support::artifact_operations(&store).expect("artifact operations"),
        crate::test_support::system_error_logger(),
        None,
        crate::daemon::RuntimeEventRegistry::default(),
    )
    .await
    .expect("runtime");
    let generation_runtime = runtime.clone();
    let generation_provider_key = provider_key.clone();
    let generation = tokio::spawn(async move {
        generation_runtime
            .generate_background_task(super::runtime::BackgroundTaskGenerateRequest {
                run_id: run.run_id.clone(),
                task_id: task.task_id.clone(),
                lease_token: lease_token.to_string(),
                cancellation: tokio_util::sync::CancellationToken::new(),
                agent_id: run.agent_id.clone(),
                role: crate::agent_execution::ExecutionRole::TaskExecutor,
                provider_selection: {
                    let mut selection = noema_providers::ProviderSelectionSnapshot::explicit(
                        "local_models",
                        "provider_account:local_models:default",
                        "old-model",
                        None,
                        Some("provider_generation_test".to_string()),
                    );
                    selection.provider_instance_key = Some(generation_provider_key);
                    selection
                },
                execution_policy: run.execution_policy,
                input: task.request_markdown.clone(),
                instructions: "Complete the task and submit the result.".to_string(),
                runtime_events: crate::daemon::RuntimeEventRegistry::default(),
            })
            .await
    });

    started_rx.await.expect("old provider started");
    provider_registry
        .register(
            provider_key,
            replacement.clone() as noema_providers::ProviderHandle,
        )
        .expect("publish replacement");
    release_tx.send(()).expect("release old provider");

    let response = generation
        .await
        .expect("background generation task")
        .expect("background generation");
    let updated_run = store
        .get_agent_run(&claimed.run_id)
        .await
        .expect("load run")
        .expect("run");
    runtime.shutdown().await;

    assert_eq!(response.provider, "old-local");
    assert_eq!(response.model, "old-model");
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].name, "task.submit_result");
    assert_eq!(old_provider.requests.lock().expect("old requests").len(), 2);
    assert!(
        replacement
            .requests
            .lock()
            .expect("replacement requests")
            .is_empty()
    );
    assert_eq!(updated_run.provider_call_count, 2);
    assert_eq!(
        updated_run.actual_provider_kind.as_deref(),
        Some("old-local")
    );
    assert_eq!(
        updated_run.actual_model_profile.as_deref(),
        Some("old-model")
    );
}

#[tokio::test]
async fn runtime_shutdown_cancels_and_drains_generate_once() {
    assert_shutdown_cancels_blocked_operation(false).await;
}

#[tokio::test]
async fn runtime_shutdown_cancels_blocked_task_completion_generation() {
    let (completion_started_tx, completion_started_rx) = oneshot::channel();
    let (primary_started_tx, _primary_started_rx) = oneshot::channel();
    let (release_completion_tx, release_completion_rx) = oneshot::channel();
    let provider = BlockingTaskCompletionProvider {
        completion_started: Mutex::new(Some(completion_started_tx)),
        primary_started: Mutex::new(Some(primary_started_tx)),
        release_completion: Mutex::new(Some(release_completion_rx)),
    };
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(Arc::new(provider), store)
        .await
        .expect("runtime");
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let completion_handle = handle.clone();
    let pending_completion = tokio::spawn(async move {
        completion_handle
            .deliver_task_completion(super::runtime::TaskCompletionDeliveryRequest {
                delivery_id: "event:shutdown-completion".to_string(),
                task_id: "task:shutdown-completion".to_string(),
                conversation_id,
                source_item_id: None,
                title: "Shutdown completion".to_string(),
                status: "completed".to_string(),
                request_markdown: "Complete before shutdown".to_string(),
                summary: None,
                result_markdown: None,
                artifacts: Vec::new(),
                review_feedback: None,
                criteria: Vec::new(),
                detail: None,
            })
            .await
    });

    completion_started_rx
        .await
        .expect("completion provider started");
    tokio::time::timeout(Duration::from_secs(1), handle.shutdown())
        .await
        .expect("shutdown should cancel completion generation");
    let error = tokio::time::timeout(Duration::from_secs(1), pending_completion)
        .await
        .expect("completion delivery reply should resolve")
        .expect("completion task")
        .expect_err("cancelled completion should fail");

    assert!(error.to_string().contains("daemon runtime stopped"));
    assert!(
        release_completion_tx.send(()).is_err(),
        "completion provider future was not dropped"
    );
}

#[tokio::test]
async fn runtime_shutdown_interrupts_inline_turn() {
    assert_shutdown_cancels_blocked_operation(true).await;
}

async fn assert_shutdown_cancels_blocked_operation(inline_turn: bool) {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider = BlockingOnceProvider {
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(Some(release_rx)),
    };
    let store = crate::test_support::test_store().await;
    let handle = RuntimeHandle::spawn_with_provider(Arc::new(provider), store.clone())
        .await
        .expect("runtime");
    let conversation_id = if inline_turn {
        Some(
            handle
                .start_conversation(None)
                .await
                .expect("conversation")
                .conversation_id,
        )
    } else {
        None
    };
    let durable_conversation_id = conversation_id.clone();
    let operation_handle = handle.clone();
    let pending_operation = tokio::spawn(async move {
        if let Some(conversation_id) = conversation_id {
            let (item_tx, _item_rx) = mpsc::unbounded_channel();
            operation_handle
                .turn(conversation_id, "slow".to_string(), item_tx)
                .await
        } else {
            operation_handle
                .generate_once(GenerateRequest::text("slow"))
                .await
                .map(|_| ())
        }
    });

    started_rx.await.expect("provider started");
    tokio::time::timeout(Duration::from_secs(1), handle.shutdown())
        .await
        .expect("shutdown should interrupt the inline turn");

    assert!(
        release_tx.send(()).is_err(),
        "provider future was not dropped"
    );
    assert!(
        pending_operation
            .await
            .expect("operation task")
            .expect_err("cancelled operation should fail")
            .to_string()
            .contains("daemon runtime stopped")
    );

    if let Some(conversation_id) = durable_conversation_id {
        let status = store
            .conversation_runtime_status(&conversation_id)
            .await
            .expect("read conversation runtime status")
            .expect("conversation runtime status");
        assert_eq!(
            status.turn_status,
            noema_conversations::ConversationTurnStatus::Cancelled
        );
        assert_eq!(status.agent_status, noema_conversations::AgentStatus::Idle);
    }
}
