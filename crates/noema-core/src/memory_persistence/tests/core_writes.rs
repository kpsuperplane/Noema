use super::*;

#[test]
fn open_initializes_canonical_database_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");

    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    assert_eq!(repo.db_path(), &dir.path().join("db").join("noema.sqlite"));
    assert!(repo.db_path().is_file());
}

#[test]
fn readonly_open_missing_database_does_not_create_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let db_dir = dir.path().join("db");
    let db_path = db_dir.join("noema.sqlite");

    let error =
        SqliteMemoryRepository::open_existing_readonly(&paths).expect_err("missing database");

    match error {
        MemoryPersistenceError::MissingDatabase { path } => assert_eq!(path, db_path),
        other => panic!("unexpected error: {other}"),
    }
    assert!(!db_dir.exists());
    assert!(!db_path.exists());
}

#[test]
fn appends_chat_candidate_with_participants_provenance_and_event() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = new_conversation_memory_candidate(
        &mut repo,
        "Kevin prefers persistent chat memory to be inspectable.",
    );
    candidate.memory_type = MemoryType::Preference;
    candidate.title = Some("Inspectable chat memory".to_string());
    candidate.owner_actor = Some(ObjectRef::human("human:kevin"));
    candidate.authority_level = MemoryAuthorityLevel::ExplicitHumanStatement;
    if let Some(source) = &mut candidate.source {
        source.evidence_excerpt = Some("remember this preference".to_string());
    }
    candidate.participants = vec![
        NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];

    let summary = repo.append_memory_candidate(&candidate).expect("append");

    assert_eq!(summary.status, MemoryStatus::Candidate);
    assert_eq!(summary.memory_type, MemoryType::Preference);
    assert_eq!(summary.home_scope_id, object_scope_key(&candidate.owner));
    assert_eq!(summary.sensitivity, Sensitivity::Normal);
    assert_eq!(summary.title, "Inspectable chat memory");
    assert_eq!(
        summary.conversation_id.as_deref(),
        Some(candidate.owner.object_id.as_str())
    );

    let participant_count: i64 = repo
        .conn
        .query_row(
            "SELECT COUNT(*) FROM memory_participants WHERE memory_id = ?1",
            params![summary.id],
            |row| row.get(0),
        )
        .expect("participant count");
    assert_eq!(participant_count, 2);

    let provenance_count: i64 = repo
        .conn
        .query_row(
            "SELECT COUNT(*) FROM object_provenance_edges WHERE target_object_type = 'memory_item' AND target_object_id = ?1",
            params![summary.id],
            |row| row.get(0),
        )
        .expect("provenance count");
    assert_eq!(provenance_count, 1);

    let event_count: i64 = repo
        .conn
        .query_row(
            "SELECT COUNT(*) FROM object_events WHERE target_object_type = 'memory_item' AND target_object_id = ?1 AND event_type = 'memory_created'",
            params![summary.id],
            |row| row.get(0),
        )
        .expect("event count");
    assert_eq!(event_count, 1);
}

#[test]
fn appends_confirmed_explicit_chat_memory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = new_conversation_memory_candidate(
        &mut repo,
        "Kevin prefers explicit remember commands to be confirmed.",
    );
    candidate.status = MemoryStatus::Confirmed;
    candidate.created_by = ObjectRef::human("human:local");
    candidate.authority_level = MemoryAuthorityLevel::ExplicitHumanStatement;
    candidate.extraction_method = MemoryExtractionMethod::ExplicitHuman;

    let summary = repo.append_memory_candidate(&candidate).expect("append");

    assert_eq!(summary.status, MemoryStatus::Confirmed);
}

#[test]
fn appends_chat_candidate_with_confidence_and_retrieval_hints() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = new_conversation_memory_candidate(
        &mut repo,
        "Kevin prefers scoped retrieval hints to stay inspectable.",
    );
    candidate.confidence = Some(0.82);
    candidate.retrieval_hints = json!({
        "topics": ["memory", "retrieval"],
        "keywords": ["inspectable"],
        "summary": "Scoped memory retrieval preference"
    });

    let summary = repo.append_memory_candidate(&candidate).expect("append");

    let (confidence, retrieval_hints): (Option<f64>, String) = repo
        .conn
        .query_row(
            "SELECT confidence, retrieval_hints FROM memory_items WHERE memory_id = ?1",
            params![summary.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("memory row");
    assert!((confidence.expect("confidence") - 0.82).abs() < f64::EPSILON);
    assert_eq!(
        serde_json::from_str::<Value>(&retrieval_hints).expect("retrieval hints"),
        candidate.retrieval_hints
    );
}

#[test]
fn appends_chat_candidate_with_subject_entities() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate =
        new_conversation_memory_candidate(&mut repo, "Kevin is evaluating Noema memory subjects.");
    let mut human_subject =
        NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::About);
    human_subject.aliases = vec!["KPSuperplane".to_string()];
    human_subject.linked_object = Some(ObjectRef::human("human:kevin"));
    human_subject.metadata = json!({"source": "chat_extraction"});
    candidate.subjects = vec![
        human_subject,
        NewMemorySubject::new(
            "concept:memory_subjects",
            "concept",
            "Memory subjects",
            SubjectRole::About,
        ),
    ];

    let summary = repo.append_memory_candidate(&candidate).expect("append");

    let subject_count: i64 = repo
        .conn
        .query_row(
            "SELECT COUNT(*) FROM memory_subjects WHERE memory_id = ?1",
            params![summary.id],
            |row| row.get(0),
        )
        .expect("subject count");
    assert_eq!(subject_count, 2);

    let (
        owner_object_type,
        owner_object_id,
        entity_type,
        canonical_name,
        aliases,
        linked_object_type,
        linked_object_id,
        metadata,
    ): (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    ) = repo
        .conn
        .query_row(
            r"
            SELECT
              owner_object_type,
              owner_object_id,
              entity_type,
              canonical_name,
              aliases,
              linked_object_type,
              linked_object_id,
              metadata
            FROM entities
            WHERE entity_id = 'human:kevin'
            ",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .expect("entity row");
    assert_eq!(owner_object_type, "conversation");
    assert_eq!(owner_object_id, candidate.owner.object_id);
    assert_eq!(entity_type, "human");
    assert_eq!(canonical_name, "Kevin");
    assert_eq!(
        serde_json::from_str::<Value>(&aliases).expect("aliases"),
        json!(["KPSuperplane"])
    );
    assert_eq!(linked_object_type.as_deref(), Some("human"));
    assert_eq!(linked_object_id.as_deref(), Some("human:kevin"));
    assert_eq!(
        serde_json::from_str::<Value>(&metadata).expect("metadata"),
        json!({"source": "chat_extraction"})
    );
}

#[test]
fn appends_relationship_claim_with_supporting_memory_provenance() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate =
        new_conversation_memory_candidate(&mut repo, "Kevin uses Noema for memory orchestration.");
    candidate.subjects = vec![
        NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
        NewMemorySubject::new("concept:noema", "concept", "Noema", SubjectRole::Target),
    ];
    if let Some(source) = &mut candidate.source {
        source.evidence_excerpt = Some("Kevin uses Noema".to_string());
    }
    let memory = repo.append_memory_candidate(&candidate).expect("memory");

    let mut relationship = NewRelationshipClaim::new(
        candidate.owner.clone(),
        "human:kevin",
        "uses",
        "concept:noema",
    );
    relationship.relationship_id = Some("rel_kevin_uses_noema".to_string());
    relationship.status = RelationshipStatus::Active;
    relationship.memory_id = Some(memory.id.clone());
    relationship.confidence = Some(0.86);

    let summary = repo
        .append_relationship_claim(&relationship)
        .expect("relationship");

    assert_eq!(summary.relationship_id, "rel_kevin_uses_noema");
    assert_eq!(summary.status, RelationshipStatus::Active);
    assert_eq!(summary.memory_id.as_deref(), Some(memory.id.as_str()));
    assert_eq!(summary.subject_name.as_deref(), Some("Kevin"));
    assert_eq!(summary.object_name.as_deref(), Some("Noema"));
}

#[test]
fn active_relationship_claim_requires_supporting_provenanced_memory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate =
        new_conversation_memory_candidate(&mut repo, "Kevin likes policy-aware graph claims.");
    candidate.subjects = vec![
        NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
        NewMemorySubject::new(
            "concept:graph_claims",
            "concept",
            "Graph claims",
            SubjectRole::Target,
        ),
    ];
    let memory = repo.append_memory_candidate(&candidate).expect("memory");

    let mut no_memory = NewRelationshipClaim::new(
        candidate.owner.clone(),
        "human:kevin",
        "likes",
        "concept:graph_claims",
    );
    no_memory.relationship_id = Some("rel_without_memory".to_string());
    no_memory.status = RelationshipStatus::Active;
    assert!(matches!(
        repo.append_relationship_claim(&no_memory),
        Err(MemoryPersistenceError::RelationshipRequiresSupportingMemory { .. })
    ));

    repo.conn
        .execute(
            "UPDATE object_provenance_edges SET deleted_at = CURRENT_TIMESTAMP WHERE target_object_type = 'memory_item' AND target_object_id = ?1",
            params![memory.id],
        )
        .expect("delete provenance");
    let mut no_provenance = NewRelationshipClaim::new(
        candidate.owner,
        "human:kevin",
        "likes",
        "concept:graph_claims",
    );
    no_provenance.relationship_id = Some("rel_without_provenance".to_string());
    no_provenance.status = RelationshipStatus::Confirmed;
    no_provenance.memory_id = Some(memory.id);
    assert!(matches!(
        repo.append_relationship_claim(&no_provenance),
        Err(MemoryPersistenceError::RelationshipSupportingMemoryMissingProvenance { .. })
    ));
}
