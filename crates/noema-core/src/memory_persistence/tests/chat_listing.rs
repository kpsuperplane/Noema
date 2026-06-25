use super::*;

#[test]
fn records_chat_turn_idempotently() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut turn = NewChatTurn::new(
        "conversation:turns",
        7,
        "human:kevin",
        "agent:primary",
        "Please remember that I care about provenance.",
        "Noted.",
    );
    turn.occurred_at = Some("2026-06-24T12:00:00Z".to_string());
    turn.metadata = json!({"request_id": "req_123"});

    repo.record_chat_turn(&turn).expect("first record");
    repo.record_chat_turn(&turn).expect("second record");

    let message_count: i64 = repo
        .conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE episode_id = ?1",
            params![turn.conversation_id],
            |row| row.get(0),
        )
        .expect("message count");
    assert_eq!(message_count, 2);

    let user_message: (String, String, String, String) = repo
        .conn
        .query_row(
            r"
            SELECT role, author_principal_id, content, occurred_at
            FROM messages
            WHERE message_id = ?1
            ",
            params![turn.user_message_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("user message");
    assert_eq!(
        user_message,
        (
            "human".to_string(),
            "human:kevin".to_string(),
            "Please remember that I care about provenance.".to_string(),
            "2026-06-24T12:00:00Z".to_string(),
        )
    );

    let assistant_message_id_count: i64 = repo
        .conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE message_id = ?1",
            params![turn.assistant_message_id],
            |row| row.get(0),
        )
        .expect("assistant count");
    assert_eq!(assistant_message_id_count, 1);

    let bootstrap_count: i64 = repo
        .conn
        .query_row(
            r"
            SELECT
              (SELECT COUNT(*) FROM sources WHERE source_id = ?1)
              + (SELECT COUNT(*) FROM scopes WHERE scope_id = ?2)
              + (SELECT COUNT(*) FROM episodes WHERE episode_id = ?2 AND home_scope_id = ?2)
              + (SELECT COUNT(*) FROM principals WHERE principal_id IN (?3, ?4))
            ",
            params![
                CHAT_SOURCE_ID,
                turn.conversation_id,
                turn.user_principal_id,
                turn.assistant_principal_id,
            ],
            |row| row.get(0),
        )
        .expect("bootstrap count");
    assert_eq!(bootstrap_count, 5);
}

#[test]
fn lists_recent_memories_with_inspection_fields() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");

    let mut first =
        NewChatMemoryCandidate::new("conversation:older", "Older memory", "agent:primary");
    first.source = Some(ChatMemorySource {
        conversation_id: "conversation:older".to_string(),
        message_id: None,
        evidence_excerpt: None,
    });
    repo.append_chat_memory_candidate(&first).expect("first");

    let mut second =
        NewChatMemoryCandidate::new("conversation:newer", "Newer memory", "agent:primary");
    second.memory_type = MemoryType::Fact;
    second.sensitivity = Sensitivity::Private;
    second.source = Some(ChatMemorySource {
        conversation_id: "conversation:newer".to_string(),
        message_id: None,
        evidence_excerpt: None,
    });
    repo.append_chat_memory_candidate(&second).expect("second");

    let memories = repo.list_recent_memories(Some(10)).expect("list");

    assert_eq!(memories.len(), 2);
    assert_eq!(memories[0].content, "Newer memory");
    assert_eq!(memories[0].memory_type, MemoryType::Fact);
    assert_eq!(memories[0].sensitivity, Sensitivity::Private);
    assert_eq!(
        memories[0].conversation_id.as_deref(),
        Some("conversation:newer")
    );
    assert_eq!(memories[1].content, "Older memory");
}

#[test]
fn list_redacts_sensitive_and_secret_memory_content() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut sensitive = NewChatMemoryCandidate::new(
        "conversation:sensitive",
        "Sensitive medical detail",
        "agent:primary",
    );
    sensitive.title = Some("Medical detail".to_string());
    sensitive.sensitivity = Sensitivity::Sensitive;
    let sensitive_created = repo
        .append_chat_memory_candidate(&sensitive)
        .expect("sensitive");
    let mut secret = NewChatMemoryCandidate::new(
        "conversation:secret",
        "Secret credential-like detail",
        "agent:primary",
    );
    secret.title = Some("Credential detail".to_string());
    secret.sensitivity = Sensitivity::Secret;
    repo.append_chat_memory_candidate(&secret).expect("secret");

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
    let mut candidate = NewChatMemoryCandidate::new(
        "conversation:show",
        "Memory show should fetch one row by id.",
        "agent:primary",
    );
    candidate.source = Some(ChatMemorySource {
        conversation_id: "conversation:show".to_string(),
        message_id: Some("message:show_1".to_string()),
        evidence_excerpt: None,
    });

    let created = repo
        .append_chat_memory_candidate(&candidate)
        .expect("append");

    let fetched = repo
        .get_memory(&created.id)
        .expect("get memory")
        .expect("created memory");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.content, "Memory show should fetch one row by id.");
    assert_eq!(
        fetched.conversation_id.as_deref(),
        Some("conversation:show")
    );

    let missing = repo.get_memory("mem_missing").expect("missing lookup");
    assert_eq!(missing, None);
}
