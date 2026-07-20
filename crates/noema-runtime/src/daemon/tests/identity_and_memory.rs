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
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "Fred it is. what would you like help with first?"
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

    assert_eq!(
        store
            .get_agent("agent:primary")
            .await
            .expect("agent")
            .expect("agent exists")
            .display_name
            .as_deref(),
        Some("Momo")
    );
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity { activity_kind, title, .. }
            if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "yay acknowledged after saved name"
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
