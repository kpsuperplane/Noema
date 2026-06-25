use super::*;

fn conversation_with_user_item(repo: &mut SqliteMemoryRepository) -> (String, String) {
    repo.ensure_default_actors().expect("actors");
    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .expect("conversation");
    let turn = repo
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .expect("turn");
    let item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::human("human:local"),
            content_text: Some("remember this: trains are welcome when relevant".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .expect("item");
    (conversation.conversation_id, item.item_id)
}

#[test]
fn deleting_item_redacts_sole_provenance_memory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let (conversation_id, item_id) = conversation_with_user_item(&mut repo);

    let memory = repo
        .append_memory_candidate(&NewMemoryCandidate {
            owner: ObjectRef::new(ObjectType::Conversation, conversation_id).expect("owner"),
            memory_type: MemoryType::Preference,
            title: Some("Train references".to_string()),
            content: "Train references are welcome when relevant.".to_string(),
            sensitivity: Sensitivity::Normal,
            status: MemoryStatus::Confirmed,
            created_by: ObjectRef::human("human:local"),
            owner_actor: Some(ObjectRef::human("human:local")),
            authority_level: MemoryAuthorityLevel::ExplicitHumanStatement,
            extraction_method: MemoryExtractionMethod::ExplicitHuman,
            confidence: None,
            retrieval_hints: json!({}),
            observed_at: None,
            source: Some(ObjectProvenanceSource {
                source: ObjectRef::conversation_item(item_id.clone()),
                evidence_excerpt: Some(
                    "remember this: trains are welcome when relevant".to_string(),
                ),
            }),
            participants: vec![
                NewMemoryParticipant::new(
                    ObjectRef::human("human:local"),
                    ParticipantRole::HumanInScope,
                ),
                NewMemoryParticipant::new(
                    ObjectRef::agent("agent:primary"),
                    ParticipantRole::AgentInScope,
                ),
            ],
            subjects: Vec::new(),
            metadata: json!({}),
        })
        .expect("memory");

    repo.soft_delete_conversation_item(DeleteConversationItem {
        item_id,
        deleted_by: ObjectRef::human("human:local"),
        reason: Some("user deleted source message".to_string()),
    })
    .expect("delete item");

    let visible_items = repo
        .list_conversation_items(
            &memory.conversation_id.expect("conversation id"),
            ReplayMode::Visible,
        )
        .expect("visible items");
    assert!(visible_items.is_empty());

    let stored: (String, String) = repo
        .conn
        .query_row(
            "SELECT status, content FROM memory_items WHERE memory_id = ?1",
            params![memory.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored memory");
    assert_eq!(stored, ("deleted".to_string(), "[redacted]".to_string()));
}

#[test]
fn deleting_item_keeps_memory_with_other_provenance() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let (conversation_id, first_item_id) = conversation_with_user_item(&mut repo);
    let second_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::Activity,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::agent("agent:primary"),
            content_text: None,
            payload_json: json!({"summary": "second evidence"}),
            metadata: json!({}),
        })
        .expect("second item");

    let memory = repo
        .append_memory_candidate(&NewMemoryCandidate::confirmed_note(
            ObjectRef::new(ObjectType::Conversation, conversation_id).expect("owner"),
            "Durable evidence can come from multiple items.",
            ObjectRef::agent("agent:primary"),
            ObjectRef::conversation_item(first_item_id.clone()),
        ))
        .expect("memory");
    repo.add_object_provenance_edge(NewObjectProvenanceEdge {
        target: ObjectRef::new(ObjectType::MemoryItem, memory.id.clone()).expect("target"),
        source: ObjectRef::conversation_item(second_item.item_id),
        relation: "supports".to_string(),
        evidence_excerpt: Some("second evidence".to_string()),
        created_by: ObjectRef::agent("agent:primary"),
        metadata: json!({}),
    })
    .expect("second edge");

    repo.soft_delete_conversation_item(DeleteConversationItem {
        item_id: first_item_id,
        deleted_by: ObjectRef::human("human:local"),
        reason: Some("delete one source".to_string()),
    })
    .expect("delete");

    let status: String = repo
        .conn
        .query_row(
            "SELECT status FROM memory_items WHERE memory_id = ?1",
            params![memory.id],
            |row| row.get(0),
        )
        .expect("status");
    assert_eq!(status, "confirmed");
}
