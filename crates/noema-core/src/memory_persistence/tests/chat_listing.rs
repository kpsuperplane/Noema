use super::*;

#[test]
fn appends_conversation_turn_items_for_replay() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    ensure_test_actors(&mut repo);

    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .expect("conversation");
    let turn = repo
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({"request_id": "req_123"}),
        })
        .expect("turn");

    let user_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::human("human:kevin"),
            content_text: Some("Please remember that I care about provenance.".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .expect("user item");
    let assistant_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: Some(user_item.item_id.clone()),
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::agent("agent:primary"),
            content_text: Some("Noted.".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .expect("assistant item");

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
}

#[test]
fn lists_recent_memories_with_inspection_fields() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");

    let first = new_conversation_memory_candidate(&mut repo, "Older memory");
    repo.append_memory_candidate(&first).expect("first");

    let mut second = new_conversation_memory_candidate(&mut repo, "Newer memory");
    second.memory_type = MemoryType::Fact;
    second.sensitivity = Sensitivity::Private;
    repo.append_memory_candidate(&second).expect("second");

    let memories = repo.list_recent_memories(Some(10)).expect("list");

    assert_eq!(memories.len(), 2);
    assert_eq!(memories[0].content, "Newer memory");
    assert_eq!(memories[0].memory_type, MemoryType::Fact);
    assert_eq!(memories[0].sensitivity, Sensitivity::Private);
    assert_eq!(
        memories[0].conversation_id.as_deref(),
        Some(second.owner.object_id.as_str())
    );
    assert_eq!(memories[1].content, "Older memory");
}

#[test]
fn list_redacts_sensitive_and_secret_memory_content() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut sensitive = new_conversation_memory_candidate(&mut repo, "Sensitive medical detail");
    sensitive.title = Some("Medical detail".to_string());
    sensitive.sensitivity = Sensitivity::Sensitive;
    let sensitive_created = repo.append_memory_candidate(&sensitive).expect("sensitive");
    let mut secret = new_conversation_memory_candidate(&mut repo, "Secret credential-like detail");
    secret.title = Some("Credential detail".to_string());
    secret.sensitivity = Sensitivity::Secret;
    repo.append_memory_candidate(&secret).expect("secret");

    let memories = repo.list_recent_memories(Some(10)).expect("list");

    assert_eq!(memories.len(), 2);
    assert!(memories.iter().all(|memory| memory.title == "[redacted]"));
    assert!(memories.iter().all(|memory| memory.content == "[redacted]"));
    let fetched = repo
        .get_memory(&sensitive_created.id)
        .expect("get")
        .expect("memory");
    assert_eq!(fetched.title, "Medical detail");
    assert_eq!(fetched.content, "Sensitive medical detail");
}

#[test]
fn fetches_single_memory_by_id() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let candidate =
        new_conversation_memory_candidate(&mut repo, "Memory show should fetch one row by id.");
    let created = repo.append_memory_candidate(&candidate).expect("append");

    let fetched = repo
        .get_memory(&created.id)
        .expect("get memory")
        .expect("created memory");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.content, "Memory show should fetch one row by id.");
    assert_eq!(
        fetched.conversation_id.as_deref(),
        Some(created.owner_object_id.as_str())
    );

    let missing = repo.get_memory("mem_missing").expect("missing lookup");
    assert_eq!(missing, None);
}
