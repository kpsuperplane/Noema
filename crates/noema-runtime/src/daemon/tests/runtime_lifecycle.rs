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
    let items = collect_turn(&handle, first.conversation_id, "hello".to_string())
        .await
        .expect("turn response");
    assert_eq!(assistant_text(&items), "fake answer");
    handle.shutdown().await;
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
async fn notification_delivery_waits_for_foreground_turn_and_publishes_exact_item() {
    let store = crate::test_support::test_store().await;
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("primary conversation");
    let notification = seed_waiting_notification(&store).await;
    let expected_task_id = notification.payload["task_id"]
        .as_str()
        .expect("notification task id")
        .to_string();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider = BlockingOnceProvider {
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(Some(release_rx)),
    };
    let events = crate::daemon::RuntimeEventRegistry::default();
    let mut conversation_events = events.subscribe_conversation(&conversation.conversation_id);
    let mut work_events = events.subscribe_work("workspace:personal");
    let runtime = RuntimeHandle::spawn_with_provider_map_and_events(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(provider) as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::artifact_operations(&store).expect("artifact operations"),
        crate::test_support::system_error_logger(),
        events,
    )
    .await
    .expect("runtime");
    let turn_runtime = runtime.clone();
    let turn_conversation_id = conversation.conversation_id.clone();
    let turn = tokio::spawn(async move {
        let (items, _receiver) = mpsc::unbounded_channel();
        turn_runtime
            .turn(turn_conversation_id, "hold foreground".to_string(), items)
            .await
    });
    started_rx.await.expect("foreground provider started");

    let delivery_runtime = runtime.clone();
    let notification_id = notification.notification_id.clone();
    let expected_notification_id = notification_id.clone();
    let delivery_conversation_id = conversation.conversation_id.clone();
    let work_event = WorkRuntimeEvent::Committed {
        workspace_id: "workspace:personal".to_string(),
        task_id: Some(expected_task_id.clone()),
    };
    let mut delivery = tokio::spawn(async move {
        delivery_runtime
            .deliver_work_notification(
                noema_store::CompleteWorkNotification {
                    notification_id,
                    lease_token: notification.lease_token,
                    conversation_id: delivery_conversation_id,
                },
                work_event,
            )
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut delivery)
            .await
            .is_err(),
        "notification must stay queued behind the active foreground turn"
    );

    release_tx.send(()).expect("release foreground turn");
    turn.await
        .expect("turn task")
        .expect("foreground turn completes");
    delivery
        .await
        .expect("delivery task")
        .expect("notification delivery");
    let event = tokio::time::timeout(Duration::from_secs(1), conversation_events.recv())
        .await
        .expect("notification wakeup")
        .expect("conversation event");
    let crate::daemon::ConversationRuntimeEvent::Turn { event, .. } = event else {
        panic!("expected notification turn event");
    };
    let TurnStreamEvent::ConversationItem {
        conversation_id,
        cursor,
        metadata,
        item,
        ..
    } = event.as_ref()
    else {
        panic!("expected notification conversation item");
    };
    let TurnTranscriptItem::TaskReference {
        task_id,
        stage_id,
        revision,
        ..
    } = item.as_ref()
    else {
        panic!("expected task reference");
    };
    assert_eq!(conversation_id, &conversation.conversation_id);
    assert!(
        cursor.is_some(),
        "notification event must expose durable cursor"
    );
    assert_eq!(metadata["notification_kind"].as_str(), Some("task_waiting"));
    assert_eq!(
        metadata["notification_id"].as_str(),
        Some(expected_notification_id.as_str())
    );
    assert_eq!(
        metadata["work_notification"]["task_id"].as_str(),
        Some(task_id.as_str())
    );
    assert_eq!(stage_id, "stage:personal:waiting");
    assert!(*revision > 0);
    let work_event = tokio::time::timeout(Duration::from_secs(1), work_events.recv())
        .await
        .expect("Work notification wakeup")
        .expect("Work event");
    let WorkRuntimeEvent::Committed {
        workspace_id,
        task_id,
    } = work_event;
    assert_eq!(workspace_id, "workspace:personal");
    assert_eq!(task_id.as_deref(), Some(expected_task_id.as_str()));

    runtime.shutdown().await;
}

pub(crate) async fn seed_waiting_notification(
    store: &noema_store::NoemaStore,
) -> noema_store::ClaimedWorkNotification {
    let (_task, queued_run) = crate::test_support::seed_task(store, "Waiting notification").await;
    let service = noema_store::WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let claimed = service
        .claim_next_work_run("worker:notification-test", 120, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    assert_eq!(claimed.run.run_id, queued_run.run_id);
    let fence = noema_store::WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id,
    };
    service
        .start_work_run(&fence, "actor:test", None, "correlation:notification-test")
        .await
        .expect("start executor");
    service
        .record_work_run_terminal(
            noema_store::WorkRunTerminal::Blocked(noema_store::ReportTaskBlocked {
                fence,
                gate_kind: noema_tasks::TaskGateKind::Clarification,
                prompt_markdown: "Which region?".to_string(),
                context_markdown: "A region is required.".to_string(),
            }),
            "actor:test",
            None,
            "correlation:notification-test",
        )
        .await
        .expect("block executor");
    let notifications = store
        .claim_work_notifications(noema_store::WorkNotificationLeaseRequest {
            worker_id: "worker:notification-test".to_string(),
            lease_seconds: 120,
            limit: 10,
        })
        .await
        .expect("claim notification");
    notifications
        .into_iter()
        .find(|notification| {
            notification.notification_kind == noema_tasks::NotificationKind::TaskWaiting
        })
        .expect("waiting notification")
}

#[tokio::test]
async fn task_supervisor_starts_distinct_tasks_concurrently() {
    let store = crate::test_support::test_store().await;
    let (_first_task, first_run) =
        crate::test_support::seed_task(&store, "Concurrent task one").await;
    let (_second_task, second_run) =
        crate::test_support::seed_task(&store, "Concurrent task two").await;
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
    let command_service = noema_store::WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let claimed = command_service
        .claim_next_work_run("worker:provider-generation", 120, &[])
        .await
        .expect("claim run")
        .expect("leased run");
    assert_eq!(claimed.run.run_id, run.run_id);
    let fence = noema_store::WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token.clone(),
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id.clone(),
    };
    command_service
        .start_work_run(
            &fence,
            "actor:test:runtime",
            None,
            "correlation:provider-generation",
        )
        .await
        .expect("start run");
    let detail = store
        .get_work_task(&task.task_id)
        .await
        .expect("read task detail")
        .expect("task detail");
    let contract = detail.current_contract.expect("execution contract");

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
    let runtime = RuntimeHandle::spawn_with_provider_registry(
        provider_registry.clone(),
        store.clone(),
        crate::test_support::artifact_operations(&store).expect("artifact operations"),
        crate::test_support::system_error_logger(),
        crate::daemon::RuntimeEventRegistry::default(),
    )
    .await
    .expect("runtime");
    let claimed_run_id = claimed.run.run_id.clone();
    let claimed_lease_token = claimed.lease_token.clone();
    let claimed_task_generation = claimed.run.task_generation;
    let claimed_contract_id = claimed.run.contract_id.clone();
    let claimed_agent_id = claimed.run.agent_id.clone();
    let claimed_execution_policy = claimed.run.execution_policy;
    let generation_runtime = runtime.clone();
    let generation_provider_key = provider_key.clone();
    let generation = tokio::spawn(async move {
        generation_runtime
            .generate_background_task(super::runtime::BackgroundTaskGenerateRequest {
                run_id: claimed_run_id,
                task_id: task.task_id.to_string(),
                lease_token: claimed_lease_token,
                task_generation: claimed_task_generation,
                contract_id: claimed_contract_id,
                cancellation: tokio_util::sync::CancellationToken::new(),
                agent_id: claimed_agent_id,
                instance_name: claimed.run.instance_name.clone(),
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
                execution_policy: claimed_execution_policy,
                input: contract.request_markdown.clone(),
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
        .get_work_run_execution_context(&claimed.run.run_id)
        .await
        .expect("load run")
        .expect("run")
        .run;
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
async fn runtime_shutdown_cancels_generate_once_and_inline_turn() {
    assert_shutdown_cancels_blocked_operation(false).await;
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
