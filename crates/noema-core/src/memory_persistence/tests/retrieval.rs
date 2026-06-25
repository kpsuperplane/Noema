use super::*;

#[test]
fn persisted_retrieval_uses_participant_overlap() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = NewChatMemoryCandidate::new(
        "conversation:old_memory",
        "Kevin prefers durable retrieval tests.",
        "agent:primary",
    );
    candidate.status = MemoryStatus::Active;
    candidate.memory_type = MemoryType::Preference;
    candidate.retrieval_hints = json!({"topics": ["memory"], "keywords": ["retrieval"]});
    candidate.participants = vec![
        NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];
    let memory = repo
        .append_chat_memory_candidate(&candidate)
        .expect("memory");

    let mut request = request_for_kevin();
    request.untrusted_hints.fuzzy_topics = vec!["memory".to_string()];
    let result = repo.retrieve_memories(&request).expect("retrieve");

    assert_eq!(included_ids(&result), vec![memory.id.as_str()]);
    assert_eq!(
        result.included[0].eligibility_reason,
        EligibilityReason::ParticipantOverlap
    );
}

#[test]
fn persisted_retrieval_uses_fts_public_hint_candidates() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");

    let mut indexed = NewChatMemoryCandidate::new(
        "conversation:public_search",
        "The context graph needs an alpine FTS candidate path.",
        "agent:primary",
    );
    indexed.status = MemoryStatus::Active;
    indexed.sensitivity = Sensitivity::Public;
    let indexed_memory = repo
        .append_chat_memory_candidate(&indexed)
        .expect("indexed memory");

    let mut unindexed = NewChatMemoryCandidate::new(
        "conversation:public_search",
        "This public memory has a matching hint but no search-index row.",
        "agent:primary",
    );
    unindexed.status = MemoryStatus::Active;
    unindexed.sensitivity = Sensitivity::Public;
    unindexed.retrieval_hints = json!({"keywords": ["ghostneedle"]});
    let unindexed_memory = repo
        .append_chat_memory_candidate(&unindexed)
        .expect("unindexed memory");
    repo.conn
        .execute(
            "DELETE FROM memory_fts WHERE memory_id = ?1",
            params![unindexed_memory.id],
        )
        .expect("remove fts row");

    let mut indexed_request = request_for_kevin();
    indexed_request.untrusted_hints.query_text = "alpine".to_string();
    let indexed_result = repo
        .retrieve_memories(&indexed_request)
        .expect("retrieve indexed");
    assert_eq!(
        included_ids(&indexed_result),
        vec![indexed_memory.id.as_str()]
    );
    assert_eq!(
        indexed_result.included[0].eligibility_reason,
        EligibilityReason::PublicHint
    );

    let mut hostile_query_request = request_for_kevin();
    hostile_query_request.untrusted_hints.query_text = r#"alpine" OR memory_fts : *"#.to_string();
    let hostile_query_result = repo
        .retrieve_memories(&hostile_query_request)
        .expect("retrieve hostile query");
    assert_eq!(
        included_ids(&hostile_query_result),
        vec![indexed_memory.id.as_str()]
    );

    let mut unindexed_request = request_for_kevin();
    unindexed_request.untrusted_hints.query_text = "ghostneedle".to_string();
    let unindexed_result = repo
        .retrieve_memories(&unindexed_request)
        .expect("retrieve unindexed");
    assert!(unindexed_result.included.is_empty());

    let rebuilt = repo
        .rebuild_memory_search_index()
        .expect("rebuild memory fts");
    assert_eq!(rebuilt, 2);
    let rebuilt_result = repo
        .retrieve_memories(&unindexed_request)
        .expect("retrieve rebuilt");
    assert_eq!(
        included_ids(&rebuilt_result),
        vec![unindexed_memory.id.as_str()]
    );
}

#[test]
fn persisted_retrieval_requires_trusted_unlock_for_sensitive_memory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = NewChatMemoryCandidate::new(
        "conversation:health",
        "Kevin needs to follow up about a doctor appointment.",
        "agent:primary",
    );
    candidate.status = MemoryStatus::Active;
    candidate.memory_type = MemoryType::OpenLoop;
    candidate.sensitivity = Sensitivity::Sensitive;
    candidate.owner_principal_id = Some("human:kevin".to_string());
    candidate.retrieval_hints =
        json!({"topics": ["health", "doctor"], "keywords": ["appointment"]});
    candidate.participants = vec![
        NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];
    let memory = repo
        .append_chat_memory_candidate(&candidate)
        .expect("memory");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_purpose_rules (
              memory_id,
              purpose,
              effect,
              created_by_principal_id
            )
            VALUES (?1, 'answer_human_question', 'allow', 'agent:primary')
            ",
            params![memory.id],
        )
        .expect("purpose rule");
    validate_sensitive_policy(&mut repo, &memory.id);

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
    request.untrusted_hints.fuzzy_topics = vec!["health".to_string(), "doctor".to_string()];
    let denied = repo.retrieve_memories(&request).expect("retrieve denied");

    assert!(denied.included.is_empty());
    assert_eq!(
        denied.denied_for_audit[0].reason,
        DenialReason::SensitiveUnlockMissing
    );
    assert_eq!(
        denied.agent_visible_omissions[0].reason,
        "policy_restricted_context"
    );

    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_object_links (
              memory_id,
              object_type,
              object_id,
              relation,
              created_by_principal_id
            )
            VALUES (?1, 'task', 'task:schedule_checkup', 'open_loop_for', 'agent:primary')
            ",
            params![memory.id],
        )
        .expect("object link");
    validate_sensitive_policy(&mut repo, &memory.id);
    request
        .trusted
        .active_object_links
        .push(ObjectLink::new("task", "task:schedule_checkup").with_relation("open_loop_for"));

    let allowed = repo.retrieve_memories(&request).expect("retrieve allowed");

    assert_eq!(included_ids(&allowed), vec![memory.id.as_str()]);
    assert_eq!(
        allowed.included[0].eligibility_reason,
        EligibilityReason::TrustedObjectLink
    );

    repo.conn
        .execute(
            "UPDATE memory_items SET content = 'Changed sensitive content' WHERE memory_id = ?1",
            params![memory.id],
        )
        .expect("change sensitive content");
    let stale = repo.retrieve_memories(&request).expect("retrieve stale");
    assert!(stale.included.is_empty());
    assert_eq!(
        stale.denied_for_audit[0].reason,
        DenialReason::RetrievalPolicyInvalid
    );
}

#[test]
fn persisted_object_link_requires_authorized_scope_when_policy_sets_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = NewChatMemoryCandidate::new(
        "conversation:health",
        "Kevin needs to follow up about a doctor appointment.",
        "agent:primary",
    );
    candidate.status = MemoryStatus::Active;
    candidate.memory_type = MemoryType::OpenLoop;
    candidate.sensitivity = Sensitivity::Sensitive;
    candidate.owner_principal_id = Some("human:kevin".to_string());
    candidate.participants = vec![
        NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];
    let memory = repo
        .append_chat_memory_candidate(&candidate)
        .expect("memory");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_purpose_rules (
              memory_id,
              purpose,
              effect,
              created_by_principal_id
            )
            VALUES (?1, 'answer_human_question', 'allow', 'agent:primary')
            ",
            params![memory.id],
        )
        .expect("purpose rule");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_object_links (
              memory_id,
              object_type,
              object_id,
              relation,
              authorized_scope_id,
              created_by_principal_id
            )
            VALUES (
              ?1,
              'task',
              'task:schedule_checkup',
              'open_loop_for',
              'conversation:health',
              'agent:primary'
            )
            ",
            params![memory.id],
        )
        .expect("object link");
    validate_sensitive_policy(&mut repo, &memory.id);

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
    request.trusted.active_object_links =
        vec![ObjectLink::new("task", "task:schedule_checkup").with_relation("open_loop_for")];
    let denied = repo.retrieve_memories(&request).expect("retrieve denied");
    assert!(denied.included.is_empty());
    assert_eq!(
        denied.denied_for_audit[0].reason,
        DenialReason::SensitiveUnlockMissing
    );

    request
        .trusted
        .active_scopes
        .push("conversation:health".to_string());
    let allowed = repo.retrieve_memories(&request).expect("retrieve allowed");
    assert_eq!(included_ids(&allowed), vec![memory.id.as_str()]);
}

#[test]
fn persisted_retrieval_honors_explicit_grant_for_private_memory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = NewChatMemoryCandidate::new(
        "conversation:private",
        "Kevin keeps a private project preference.",
        "agent:primary",
    );
    candidate.status = MemoryStatus::Active;
    candidate.sensitivity = Sensitivity::Private;
    candidate.owner_principal_id = Some("human:kevin".to_string());
    candidate.participants = vec![
        NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];
    let memory = repo
        .append_chat_memory_candidate(&candidate)
        .expect("memory");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_purpose_rules (
              memory_id,
              purpose,
              effect,
              created_by_principal_id
            )
            VALUES (?1, 'answer_human_question', 'allow', 'agent:primary')
            ",
            params![memory.id],
        )
        .expect("purpose rule");
    validate_private_policy(&mut repo, &memory.id);

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Private;
    let denied = repo.retrieve_memories(&request).expect("retrieve denied");
    assert!(denied.included.is_empty());

    repo.conn
        .execute(
            r"
            INSERT INTO memory_access_grants (
              grant_id,
              memory_id,
              principal_id,
              permission,
              effect,
              created_by_principal_id
            )
            VALUES ('grant_private_memory', ?1, 'agent:primary', 'use_for_retrieval', 'allow', 'human:kevin')
            ",
            params![memory.id],
        )
        .expect("grant");

    let stale_after_grant = repo
        .retrieve_memories(&request)
        .expect("retrieve stale after grant");
    assert!(stale_after_grant.included.is_empty());
    assert_eq!(
        stale_after_grant.denied_for_audit[0].reason,
        DenialReason::RetrievalPolicyInvalid
    );

    validate_private_policy(&mut repo, &memory.id);
    let allowed = repo.retrieve_memories(&request).expect("retrieve allowed");
    assert_eq!(included_ids(&allowed), vec![memory.id.as_str()]);
    assert_eq!(
        allowed.included[0].eligibility_reason,
        EligibilityReason::ExplicitGrant
    );
}

#[test]
fn persisted_retrieval_expands_one_hop_graph_after_policy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut anchor = NewChatMemoryCandidate::new(
        "project:noema",
        "Noema uses an inspectable memory system.",
        "agent:primary",
    );
    anchor.status = MemoryStatus::Active;
    anchor.subjects = vec![
        NewMemorySubject::new("project:noema", "project", "Noema", SubjectRole::About),
        NewMemorySubject::new(
            "concept:scoped_graph_claims",
            "concept",
            "Scoped graph claims",
            SubjectRole::About,
        ),
        NewMemorySubject::new(
            "concept:unscoped_graph_claims",
            "concept",
            "Unscoped graph claims",
            SubjectRole::About,
        ),
    ];
    let anchor_memory = repo.append_chat_memory_candidate(&anchor).expect("anchor");

    let mut backing = NewChatMemoryCandidate::new(
        "conversation:graph_evidence",
        "Noema graph claims are scoped and memory-backed.",
        "agent:primary",
    );
    backing.status = MemoryStatus::Active;
    backing.participants = vec![NewMemoryParticipant::new(
        "human:kevin",
        ParticipantRole::HumanInScope,
    )];
    let backing_memory = repo
        .append_chat_memory_candidate(&backing)
        .expect("backing");

    let mut relationship = NewRelationshipClaim::new(
        "project:noema",
        "project:noema",
        "uses",
        "concept:scoped_graph_claims",
    );
    relationship.status = RelationshipStatus::Active;
    relationship.memory_id = Some(backing_memory.id.clone());
    repo.append_relationship_claim(&relationship)
        .expect("relationship");
    let mut unscoped_backing = NewChatMemoryCandidate::new(
        "conversation:unscoped_graph_evidence",
        "This graph claim has no participant aperture.",
        "agent:primary",
    );
    unscoped_backing.status = MemoryStatus::Active;
    let unscoped_memory = repo
        .append_chat_memory_candidate(&unscoped_backing)
        .expect("unscoped backing");
    let mut unscoped_relationship = NewRelationshipClaim::new(
        "project:noema",
        "project:noema",
        "mentions",
        "concept:unscoped_graph_claims",
    );
    unscoped_relationship.status = RelationshipStatus::Active;
    unscoped_relationship.memory_id = Some(unscoped_memory.id.clone());
    repo.append_relationship_claim(&unscoped_relationship)
        .expect("unscoped relationship");

    let mut request = request_for_kevin();
    request
        .trusted
        .active_scopes
        .push("project:noema".to_string());
    let result = repo.retrieve_memories(&request).expect("retrieve");

    assert_eq!(
        included_ids(&result),
        vec![anchor_memory.id.as_str(), backing_memory.id.as_str()]
    );
    assert!(
        result
            .included
            .iter()
            .any(|memory| memory.memory_id == backing_memory.id
                && memory.eligibility_reason == EligibilityReason::ParticipantOverlap)
    );
    assert!(result.denied_for_audit.iter().any(|denial| {
        denial.memory_id.as_deref() == Some(unscoped_memory.id.as_str())
            && denial.reason == DenialReason::OutsideSearchAperture
    }));
}

#[test]
fn persisted_retrieval_respects_validity_windows() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut expired = NewChatMemoryCandidate::new(
        "conversation:expired",
        "Kevin once preferred an expired memory.",
        "agent:primary",
    );
    expired.status = MemoryStatus::Active;
    expired.participants = vec![NewMemoryParticipant::new(
        "human:kevin",
        ParticipantRole::HumanInScope,
    )];
    let expired_memory = repo
        .append_chat_memory_candidate(&expired)
        .expect("expired memory");
    repo.conn
        .execute(
            "UPDATE memory_items SET expires_at = '2000-01-01T00:00:00Z' WHERE memory_id = ?1",
            params![expired_memory.id],
        )
        .expect("expire memory");

    let mut anchor = NewChatMemoryCandidate::new(
        "project:validity",
        "Noema has validity-windowed graph claims.",
        "agent:primary",
    );
    anchor.status = MemoryStatus::Active;
    anchor.subjects = vec![
        NewMemorySubject::new(
            "project:validity",
            "project",
            "Validity",
            SubjectRole::About,
        ),
        NewMemorySubject::new(
            "concept:expired_edge",
            "concept",
            "Expired edge",
            SubjectRole::About,
        ),
    ];
    let anchor_memory = repo.append_chat_memory_candidate(&anchor).expect("anchor");
    let mut graph_backing = NewChatMemoryCandidate::new(
        "conversation:validity_graph",
        "A public graph backing memory should be excluded by an expired edge.",
        "agent:primary",
    );
    graph_backing.status = MemoryStatus::Active;
    graph_backing.sensitivity = Sensitivity::Public;
    let graph_memory = repo
        .append_chat_memory_candidate(&graph_backing)
        .expect("graph backing");
    let mut relationship = NewRelationshipClaim::new(
        "project:validity",
        "project:validity",
        "mentions",
        "concept:expired_edge",
    );
    relationship.relationship_id = Some("rel_expired_validity".to_string());
    relationship.status = RelationshipStatus::Active;
    relationship.memory_id = Some(graph_memory.id.clone());
    repo.append_relationship_claim(&relationship)
        .expect("relationship");
    repo.conn
        .execute(
            "UPDATE relationships SET valid_to = '2000-01-01T00:00:00Z' WHERE relationship_id = 'rel_expired_validity'",
            [],
        )
        .expect("expire relationship");

    let mut request = request_for_kevin();
    request
        .trusted
        .active_scopes
        .push("project:validity".to_string());
    let expired_result = repo.retrieve_memories(&request).expect("retrieve expired");

    assert_eq!(
        included_ids(&expired_result),
        vec![anchor_memory.id.as_str()]
    );

    repo.conn
        .execute(
            "UPDATE relationships SET valid_to = NULL WHERE relationship_id = 'rel_expired_validity'",
            [],
        )
        .expect("restore relationship");
    let active_result = repo.retrieve_memories(&request).expect("retrieve active");

    assert_eq!(
        included_ids(&active_result),
        vec![anchor_memory.id.as_str(), graph_memory.id.as_str()]
    );
    assert!(
        active_result
            .included
            .iter()
            .any(|memory| memory.memory_id == graph_memory.id
                && memory.eligibility_reason == EligibilityReason::GraphExpansion)
    );
}
