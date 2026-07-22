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
    assert_eq!(
        items
            .iter()
            .filter(|item| item.kind == ConversationItemKind::AssistantText)
            .filter_map(|item| item.content_text.as_deref())
            .collect::<Vec<_>>(),
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

    let first_error = handle
        .start_primary_conversation(None)
        .await
        .expect_err("onboarding should fail");
    let second_error = handle
        .start_primary_conversation(None)
        .await
        .expect_err("onboarding retry should fail");

    assert!(
        first_error
            .to_string()
            .contains("initial onboarding response did not include assistant text")
    );
    assert_eq!(first_error.to_string(), second_error.to_string());
    let events = read_system_error_events(logger.path());
    assert_eq!(events.len(), 2);
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
    let conversation_id = _store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("primary conversation")
        .conversation_id;
    assert_eq!(
        _store
            .next_conversation_turn_index(&conversation_id)
            .await
            .expect("next turn index"),
        3
    );
    handle.shutdown().await;
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
        !noema_store::test_support::task_created_by_call(
            &store,
            &conversation_id,
            "call_task_mixed",
        )
        .await
        .expect("mixed task lookup")
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
        vec!["I could not combine delegation with another tool."]
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

#[tokio::test]
async fn runtime_executes_every_homogeneous_delegation_and_uses_provider_handoff_narration() {
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

    for (call_id, expected) in [
        ("call_task_canada", true),
        ("call_task_usa", true),
        ("call_task_invalid", false),
    ] {
        assert_eq!(
            noema_store::test_support::task_created_by_call(
                &store,
                &conversation_id,
                call_id,
            )
            .await
            .expect("task lookup"),
            expected,
            "creation call {call_id}"
        );
    }

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert_eq!(
        replay
            .iter()
            .filter(|item| item.kind == ConversationItemKind::AssistantText)
            .filter_map(|item| item.content_text.as_deref())
            .collect::<Vec<_>>(),
        vec![
            "I started all three background tasks.",
            "They are underway.",
        ]
    );
    assert_eq!(
        replay
            .iter()
            .filter(|item| item.kind == ConversationItemKind::TaskReference)
            .count(),
        0
    );
    assert!(!replay
        .iter()
        .any(|item| item.metadata["source"] == "task_delegation_receipt"));
    assert!(events.iter().any(|event| matches!(
        event,
        TurnStreamEvent::AssistantTextDelta { delta, .. }
            if delta.contains("I st") || delta.contains("They")
    )));
}
