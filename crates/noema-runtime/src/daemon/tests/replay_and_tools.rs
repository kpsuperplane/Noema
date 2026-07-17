#[tokio::test]
async fn runtime_primary_conversation_sends_recent_durable_context_after_restart() {
    if let Ok(phase) = std::env::var(RESTART_CONTEXT_TEST_PHASE_ENV) {
        let home = PathBuf::from(
            std::env::var(RESTART_CONTEXT_TEST_HOME_ENV).expect("restart test home env"),
        );
        match phase.as_str() {
            "write" => restart_context_write_phase(&home).await,
            "read" => restart_context_read_phase(&home).await,
            other => panic!("unknown restart context test phase: {other}"),
        }
        return;
    }

    let home = tempfile::tempdir().expect("temp noema home");
    run_restart_context_child_phase("write", home.path());
    run_restart_context_child_phase("read", home.path());
}

#[tokio::test]
async fn runtime_prompt_includes_unnamed_agent_onboarding() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::IdentityPromptCheck)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw unnamed identity"
    )));
}

#[tokio::test]
async fn runtime_provider_prompt_includes_assistant_text_phase_contract() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::PromptPhaseContract)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw phase contract"
    )));
}

#[tokio::test]
async fn runtime_provider_prompt_allows_markdown_in_assistant_text() {
    let handle =
        test_runtime_handle(fake_provider(FakeCodexScenario::PromptMarkdownContract)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw markdown contract"
    )));
}

#[tokio::test]
async fn start_primary_conversation_generates_initial_name_onboarding_message() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::InitialNameOnboarding))
            .await;

    let conversation_id = handle
        .start_primary_conversation(None)
        .await
        .expect("primary conversation")
        .conversation_id;
    handle.shutdown().await;

    let items = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        !items
            .iter()
            .any(|item| item.kind == ConversationItemKind::UserText),
        "initial onboarding should not fake a user message: {items:?}"
    );
    let assistant_texts: Vec<_> = items
        .iter()
        .filter(|item| item.kind == ConversationItemKind::AssistantText)
        .filter_map(|item| item.content_text.as_deref())
        .collect();
    assert_eq!(
        assistant_texts,
        vec![
            "hey, i’m glad to be here with you 👋",
            "i can help you think, plan, make, untangle, and keep life moving with a little more ease",
            "what would you like to name me?",
        ]
    );
}

#[tokio::test]
async fn failed_initial_name_onboarding_logs_runtime_invariant() {
    let (handle, _store, logger) = test_runtime_handle_with_store_and_system_errors(fake_provider(
        FakeCodexScenario::InitialNameOnboardingNoAssistant,
    ))
    .await;

    let error = handle
        .start_primary_conversation(None)
        .await
        .expect_err("onboarding should fail");

    assert!(
        error
            .to_string()
            .contains("initial onboarding response did not include assistant text")
    );
    let events = read_system_error_events(logger.path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["category"], SYSTEM_ERROR_RUNTIME_INVARIANT);
    assert_eq!(
        events[0]["message"],
        "initial onboarding response did not include assistant text"
    );
    assert_eq!(events[0]["raw"]["persisted_count"], 0);
    assert_eq!(
        events[0]["raw"]["provider_response"]["responses"],
        json!([])
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn failed_initial_name_onboarding_recomputes_turn_index_on_retry() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_provider(FakeCodexScenario::TurnError)).await;

    assert!(handle.start_primary_conversation(None).await.is_err());
    assert!(handle.start_primary_conversation(None).await.is_err());
    handle.shutdown().await;

    let conversation_id = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("primary conversation")
        .conversation_id;
    assert_eq!(
        store
            .next_conversation_turn_index(&conversation_id)
            .await
            .expect("next turn index"),
        3
    );
}

fn run_restart_context_child_phase(phase: &str, home: &std::path::Path) {
    let output = Command::new(std::env::current_exe().expect("current test binary"))
        .arg(concat!(
            module_path!(),
            "::runtime_primary_conversation_sends_recent_durable_context_after_restart"
        ))
        .arg("--exact")
        .env(RESTART_CONTEXT_TEST_PHASE_ENV, phase)
        .env(RESTART_CONTEXT_TEST_HOME_ENV, home)
        .output()
        .expect("run restart context test phase");
    assert!(
        output.status.success(),
        "restart context {phase} phase failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn restart_context_write_phase(home: &std::path::Path) {
    let paths = NoemaPaths::from_noema_home(home).expect("paths");
    let first_store = crate::test_support::test_store_for_paths(&paths).await;
    let first_handle = RuntimeHandle::spawn_with_provider(
        Arc::new(fake_provider(FakeCodexScenario::RestartContext)),
        first_store.clone(),
    )
    .await
    .expect("first runtime");
    let first_conversation_id = first_handle
        .start_primary_conversation(None)
        .await
        .expect("first primary conversation")
        .conversation_id;
    let first_items = collect_turn(
        &first_handle,
        first_conversation_id.clone(),
        "first durable question".to_string(),
    )
    .await
    .expect("first turn");
    assert_eq!(assistant_text(&first_items), "fake answer");
    first_handle.shutdown().await;
    std::fs::write(
        home.join(RESTART_CONTEXT_TEST_CONVERSATION_FILE),
        &first_conversation_id,
    )
    .expect("write restart conversation id");
    first_store.close().await.expect("close first store");
}

async fn restart_context_read_phase(home: &std::path::Path) {
    let first_conversation_id =
        std::fs::read_to_string(home.join(RESTART_CONTEXT_TEST_CONVERSATION_FILE))
            .expect("read restart conversation id");
    let paths = NoemaPaths::from_noema_home(home).expect("paths");
    let reopened_store = crate::test_support::test_store_for_paths(&paths).await;
    let second_handle = RuntimeHandle::spawn_with_provider(
        Arc::new(fake_provider(FakeCodexScenario::RestartContext)),
        reopened_store.clone(),
    )
    .await
    .expect("second runtime");
    let restarted_conversation_id = second_handle
        .start_primary_conversation(None)
        .await
        .expect("restarted primary conversation")
        .conversation_id;
    assert_eq!(first_conversation_id, restarted_conversation_id);

    let second_items = collect_turn(
        &second_handle,
        restarted_conversation_id,
        "second durable question".to_string(),
    )
    .await
    .expect("second turn");
    assert_eq!(assistant_text(&second_items), "saw durable context");
    second_handle.shutdown().await;

    let replay = reopened_store
        .list_conversation_items(&first_conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let user_texts = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::UserText)
        .map(|item| item.content_text.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        user_texts,
        vec![
            Some("first durable question"),
            Some("second durable question")
        ]
    );
    reopened_store.close().await.expect("close reopened store");
}

#[tokio::test]
async fn runtime_actor_persists_provider_tool_items_as_action_rows() {
    let (handle, store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::ToolItem),
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
        "Use your tool".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));

    let started_tool_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    ..
                } if activity_kind == "tool_call"
                    && *status == TurnActivityStatus::Started
                    && id.starts_with("tool_call:")
            )
        })
        .expect("started tool call marker");
    let completed_result_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    ..
                } if activity_kind == "tool_result"
                    && *status == TurnActivityStatus::Completed
                    && id.starts_with("tool_result:")
            )
        })
        .expect("completed tool result marker");
    assert!(
        started_tool_position < completed_result_position,
        "tool call should appear as started before its result completes"
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        replay.iter().any(|item| {
            item.kind == ConversationItemKind::ToolCall
                && item.status == ConversationItemStatus::Running
                && item.payload_json["activity_kind"] == "tool_call"
                && item.payload_json["metadata"]["action"]["name"] == "search_memory"
        }),
        "expected replayed tool call item, got {replay:?}"
    );
    let tool_position = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::ToolCall)
        .expect("tool call item");
    let assistant_position = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::AssistantText)
        .expect("assistant item");
    assert!(
        assistant_position < tool_position,
        "assistant commentary should replay before runtime tool execution: {replay:?}"
    );
}

#[tokio::test]
async fn runtime_displays_commentary_before_tool_lifecycle_when_provider_orders_tool_first() {
    let (handle, _store, _server) = test_runtime_handle_with_mnemosyne(
        fake_provider(FakeCodexScenario::ToolCallBeforeCommentary),
        json!({"results": []}),
    )
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(&handle, conversation_id, "Check memory.".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let commentary_position = items
        .iter()
        .position(|item| {
            matches!(item, TurnTranscriptItem::AssistantText { text } if text == "Checking memory.")
        })
        .expect("commentary item");
    let tool_started_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Started,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_call"
                    && title == "Tool call: search_memory"
                    && metadata.get("provider").and_then(serde_json::Value::as_str) == Some("noema_local")
            )
        })
        .expect("tool started item");
    let tool_result_position = items
        .iter()
        .position(|item| {
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
        .expect("tool result item");

    assert!(
        commentary_position < tool_started_position,
        "commentary should describe intent before runtime tool execution starts: {items:?}"
    );
    assert!(
        tool_started_position < tool_result_position,
        "tool lifecycle should start before its result: {items:?}"
    );
}

#[tokio::test]
async fn runtime_keeps_commentary_before_tool_lifecycle_when_provider_orders_text_first() {
    let handle =
        test_runtime_handle(fake_provider(FakeCodexScenario::SearchMemoryContinuation)).await;
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

    let commentary_position = items
        .iter()
        .position(|item| {
            matches!(item, TurnTranscriptItem::AssistantText { text } if text == "Searching memory.")
        })
        .expect("commentary item");
    let tool_started_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Started,
                    title,
                    metadata,
                    ..
                } if activity_kind == "tool_call"
                    && title == "Tool call: search_memory"
                    && metadata.get("provider").and_then(serde_json::Value::as_str) == Some("noema_local")
            )
        })
        .expect("tool started item");
    let final_position = items
        .iter()
        .rposition(|item| {
            matches!(item, TurnTranscriptItem::AssistantText { text } if text == "I found your train memory.")
        })
        .expect("final answer item");

    assert!(commentary_position < tool_started_position, "{items:?}");
    assert!(tool_started_position < final_position, "{items:?}");
}

#[tokio::test]
async fn runtime_executes_every_homogeneous_delegation_and_writes_truthful_receipt() {
    let (handle, store) = test_runtime_handle_with_task_delegation(fake_provider(
        FakeCodexScenario::MultipleTaskDelegation,
    ))
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "Start the Canada and USA research tasks.".to_string(),
    )
    .await;
    result.expect("delegation turn");
    handle.shutdown().await;

    assert!(
        store
            .find_task_by_creation_call(&conversation_id, "call_task_canada")
            .await
            .expect("Canada task lookup")
            .is_some()
    );
    assert!(
        store
            .find_task_by_creation_call(&conversation_id, "call_task_usa")
            .await
            .expect("USA task lookup")
            .is_some()
    );
    assert!(
        store
            .find_task_by_creation_call(&conversation_id, "call_task_invalid")
            .await
            .expect("invalid task lookup")
            .is_none()
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let assistant_texts = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::AssistantText)
        .filter_map(|item| item.content_text.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        assistant_texts,
        vec!["Started 2 background tasks; 1 delegation failed."]
    );
    assert_eq!(
        replay
            .iter()
            .filter(|item| item.kind == ConversationItemKind::TaskReference)
            .count(),
        2
    );
    let receipt = replay
        .iter()
        .find(|item| item.metadata["source"] == "task_delegation_receipt")
        .expect("delegation receipt");
    assert_eq!(receipt.metadata["delegation_success_count"], 2);
    assert_eq!(receipt.metadata["delegation_failure_count"], 1);
    assert_eq!(
        receipt.metadata["reconciled_stream_ids"],
        json!([
            format!(
                "assistant_stream:{}:initial:response:0",
                receipt.turn_id.as_deref().expect("turn id")
            ),
            format!(
                "assistant_stream:{}:initial:response:1",
                receipt.turn_id.as_deref().expect("turn id")
            ),
        ])
    );
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AssistantTextDelta { delta, .. }
                if delta.contains("I st") || delta.contains("They")
        )
    }));
}

#[tokio::test]
async fn runtime_rejects_mixed_delegation_batch_without_executing_any_call() {
    let (handle, store) = test_runtime_handle_with_task_delegation(fake_provider(
        FakeCodexScenario::MixedTaskDelegation,
    ))
    .await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "Delegate the task and rename yourself.".to_string(),
    )
    .await;
    result.expect("mixed delegation turn");
    handle.shutdown().await;

    assert!(
        store
            .find_task_by_creation_call(&conversation_id, "call_task_mixed")
            .await
            .expect("mixed task lookup")
            .is_none()
    );
    assert_eq!(
        store
            .get_agent("agent:primary")
            .await
            .expect("primary agent")
            .expect("primary agent record")
            .display_name,
        None
    );
    assert!(!events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem { item_id, .. }
                if item_id.starts_with("transient:tool_call:")
        )
    }));

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let assistant_texts = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::AssistantText)
        .filter_map(|item| item.content_text.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        assistant_texts,
        vec![
            "1 task delegation failed.",
            "I could not combine delegation with another tool."
        ]
    );
    assert_eq!(
        replay
            .iter()
            .filter(|item| item.kind == ConversationItemKind::ToolResult)
            .filter(|item| item.status == ConversationItemStatus::Failed)
            .count(),
        2
    );
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::ToolResult
            && item.payload_json["metadata"]["action"]["payload"]["error"]
                == "task_delegate_mixed_tool_batch"
    }));
    assert!(!assistant_texts.contains(&"I started the task and renamed myself."));
}

#[tokio::test]
async fn runtime_actor_persists_provider_tool_items_before_turn_failure() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::ToolItemThenFailure)).await;

    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await;
    assert!(matches!(result, Err(RuntimeError::Provider(_))));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Error,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem {
                conversation_id: id,
                item_id,
                turn_id: Some(_),
                item,
                ..
            } if id == &conversation_id
                && item_id.starts_with("item:")
                && matches!(
                    item.as_ref(),
                    TurnTranscriptItem::Activity {
                        activity_kind,
                        status: TurnActivityStatus::Completed,
                        title,
                        ..
                    } if activity_kind == "tool_call"
                        && title == "Tool call: search_memory"
                )
        )
    }));
    let items = transcript_items_from_events(events);
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));
}
