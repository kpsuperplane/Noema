use super::*;

#[test]
fn creates_conversation_turn_and_replays_items() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    repo.ensure_default_actors().expect("actors");

    let conversation = repo
        .create_conversation(NewConversation {
            title: Some("Object model chat".to_string()),
            owner: ObjectRef::human("human:local"),
            primary_human_id: Some("human:local".to_string()),
            primary_agent_id: Some("agent:primary".to_string()),
            provider: "codex".to_string(),
            model: Some("test-model".to_string()),
            provider_thread_id: Some("thread_123".to_string()),
            cwd: Some("/tmp/noema".to_string()),
            metadata: json!({"source": "test"}),
        })
        .expect("conversation");
    assert_eq!(
        conversation.provider_thread_id.as_deref(),
        Some("thread_123")
    );

    let turn = repo
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({"turn": 1}),
        })
        .expect("turn");

    let user_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::human("human:local"),
            content_text: Some("remember this: I like durable transcript".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .expect("user item");
    let assistant_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::agent("agent:primary"),
            content_text: Some("Noted.".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .expect("assistant item");

    repo.complete_conversation_turn(&turn.turn_id)
        .expect("complete turn");

    let replay = repo
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .expect("replay");
    assert_eq!(
        replay
            .iter()
            .map(|item| item.item_id.as_str())
            .collect::<Vec<_>>(),
        vec![user_item.item_id.as_str(), assistant_item.item_id.as_str()]
    );
    assert_eq!(replay[0].kind, ConversationItemKind::UserText);
    assert_eq!(
        replay[0].content_text.as_deref(),
        Some("remember this: I like durable transcript")
    );
    assert_eq!(replay[1].kind, ConversationItemKind::AssistantText);
    assert_eq!(replay[1].content_text.as_deref(), Some("Noted."));
}

#[test]
fn rejects_item_turn_from_another_conversation() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    repo.ensure_default_actors().expect("actors");

    let first = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .expect("first");
    let second = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .expect("second");
    let first_turn = repo
        .create_conversation_turn(NewConversationTurn {
            conversation_id: first.conversation_id,
            trigger_item_id: None,
            metadata: json!({}),
        })
        .expect("turn");

    let error = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: second.conversation_id.clone(),
            turn_id: Some(first_turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::human("human:local"),
            content_text: Some("hello".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .expect_err("turn mismatch");

    assert!(matches!(
        error,
        MemoryPersistenceError::TurnConversationMismatch { turn_id, conversation_id }
          if turn_id == first_turn.turn_id && conversation_id == second.conversation_id
    ));
}
