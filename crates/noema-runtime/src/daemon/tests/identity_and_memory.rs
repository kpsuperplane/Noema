#[tokio::test]
async fn update_own_name_continuation_keeps_the_foreground_tool_catalog_stable() {
    let provider = Arc::new(fake_provider(
        FakeCodexScenario::UpdateOwnNameThenIdentityCheck,
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
        "Your name is Mira.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "Mira it is."
    )));
    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "{requests:?}");
    let tool_names = |request: &GenerateRequest| {
        request
            .tools
            .iter()
            .map(|tool| tool.name.as_str().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(tool_names(&requests[0]), tool_names(&requests[1]));
    assert!(
        tool_names(&requests[1])
            .iter()
            .any(|name| name == "update_own_name")
    );

    let context_items = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation items");
    let tool_visibility_updates = context_items
        .iter()
        .filter(|item| {
            item.kind == ConversationItemKind::ModelContextUpdate
                && item.metadata.get("section_id").and_then(Value::as_str)
                    == Some("tools.visibility")
        })
        .count();
    assert_eq!(tool_visibility_updates, 1, "{context_items:?}");
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
