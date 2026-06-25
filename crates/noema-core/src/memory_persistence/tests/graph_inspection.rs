use super::*;

#[test]
fn inspects_context_graph_from_canonical_tables() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate = NewChatMemoryCandidate::new(
        "conversation:inspect_graph",
        "Kevin prefers inspectable context graphs.",
        "agent:primary",
    );
    candidate.retrieval_hints = json!({"topics": ["context graph"], "keywords": ["inspect"]});
    candidate.subjects = vec![
        NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
        NewMemorySubject::new(
            "concept:context_graph",
            "concept",
            "Context graph",
            SubjectRole::Target,
        ),
    ];
    candidate.participants = vec![
        NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];
    candidate.source = Some(ChatMemorySource {
        conversation_id: "conversation:inspect_graph".to_string(),
        message_id: None,
        evidence_excerpt: Some("inspectable context graphs".to_string()),
    });
    let memory = repo
        .append_chat_memory_candidate(&candidate)
        .expect("memory");
    repo.conn
        .execute(
            r"
            UPDATE memory_items
            SET
              retrieval_policy_status = 'valid',
              retrieval_policy_version = 2,
              retrieval_policy_fingerprint = 'sha256:inspect_graph',
              retrieval_policy_extractor_principal_id = 'agent:primary',
              retrieval_policy_extractor_version = 'extractor-v1',
              retrieval_policy_validated_at = '2026-06-24 12:00:00',
              participant_visibility_policy = 'owner_only',
              external_egress_policy = 'approval_required'
            WHERE memory_id = ?1
            ",
            params![memory.id],
        )
        .expect("policy metadata");
    let mut relationship = NewRelationshipClaim::new(
        "conversation:inspect_graph",
        "human:kevin",
        "prefers",
        "concept:context_graph",
    );
    relationship.status = RelationshipStatus::Active;
    relationship.memory_id = Some(memory.id.clone());
    repo.append_relationship_claim(&relationship)
        .expect("relationship");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_object_links (
              memory_id,
              object_type,
              object_id,
              relation,
              resolver_principal_id,
              resolver_version,
              source_run_id,
              authorized_scope_id,
              created_by_principal_id
            )
            VALUES (
              ?1,
              'project',
              'project:noema',
              'active_context',
              'agent:primary',
              'resolver-v1',
              'run:inspect_graph',
              'conversation:inspect_graph',
              'agent:primary'
            )
            ",
            params![memory.id],
        )
        .expect("object link");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_purpose_rules (
              memory_id,
              purpose,
              effect,
              created_by_principal_id
            )
            VALUES
              (?1, 'answer_human_question', 'allow', 'agent:primary'),
              (?1, 'external_action', 'deny', 'agent:primary')
            ",
            params![memory.id],
        )
        .expect("purpose rule");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_access_grants (
              grant_id,
              memory_id,
              principal_id,
              permission,
              effect,
              expires_at,
              created_by_principal_id
            )
            VALUES (
              'grant_inspect_graph',
              ?1,
              'agent:primary',
              'use_for_retrieval',
              'allow',
              '2000-01-01 00:00:00',
              'human:kevin'
            )
            ",
            params![memory.id],
        )
        .expect("access grant");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_access_grants (
              grant_id,
              scope_id,
              principal_id,
              permission,
              effect,
              created_by_principal_id
            )
            VALUES (
              'grant_inspect_scope_write',
              'conversation:inspect_graph',
              'agent:primary',
              'write',
              'allow',
              'human:kevin'
            )
            ",
            [],
        )
        .expect("scope access grant");
    repo.conn
        .execute(
            r"
            INSERT INTO scopes (scope_id, scope_type, name, slug)
            VALUES (
              'conversation:unrelated',
              'conversation',
              'Unrelated conversation',
              'unrelated'
            )
            ",
            [],
        )
        .expect("unrelated scope");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_access_grants (
              grant_id,
              scope_id,
              principal_id,
              permission,
              effect,
              created_by_principal_id
            )
            VALUES (
              'grant_unrelated_scope',
              'conversation:unrelated',
              'agent:primary',
              'use_for_retrieval',
              'allow',
              'human:kevin'
            )
            ",
            [],
        )
        .expect("unrelated access grant");
    repo.conn
        .execute(
            r#"
            INSERT INTO memory_events (
              event_id,
              event_type,
              actor_principal_id,
              memory_id,
              scope_id,
              reason,
              details
            )
            VALUES (
              'event_inspect_shown',
              'shown_to_agent',
              'agent:primary',
              ?1,
              'conversation:inspect_graph',
              'context_packet',
              '{"run_id":"run:inspect_graph","stage":"shown_to_agent"}'
            )
            "#,
            params![memory.id],
        )
        .expect("memory use event");
    repo.conn
        .execute(
            r#"
            INSERT INTO memory_events (
              event_id,
              event_type,
              actor_principal_id,
              scope_id,
              reason,
              details
            )
            VALUES (
              'event_inspect_scope_retrieval',
              'retrieved',
              'agent:primary',
              'conversation:inspect_graph',
              'scope_context',
              '{"run_id":"run:inspect_graph"}'
            )
            "#,
            [],
        )
        .expect("scope event");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_events (
              event_id,
              event_type,
              actor_principal_id,
              scope_id,
              reason,
              details
            )
            VALUES (
              'event_unrelated_scope',
              'retrieved',
              'agent:primary',
              'conversation:unrelated',
              'unrelated',
              '{}'
            )
            ",
            [],
        )
        .expect("unrelated event");

    let graph = repo.inspect_context_graph(Some(20)).expect("graph");

    assert!(
        graph
            .memories
            .iter()
            .any(|node| node.memory_id == memory.id)
    );
    let memory_node = graph
        .memories
        .iter()
        .find(|node| node.memory_id == memory.id)
        .expect("memory node");
    assert_eq!(
        memory_node.retrieval_policy_status,
        RetrievalPolicyStatus::Valid
    );
    assert_eq!(
        memory_node.retrieval_policy_effective_status,
        RetrievalPolicyStatus::Stale
    );
    assert_eq!(memory_node.retrieval_policy_version, 2);
    assert_eq!(
        memory_node.retrieval_policy_fingerprint.as_deref(),
        Some("sha256:inspect_graph")
    );
    assert_eq!(
        memory_node.participant_visibility_policy,
        ParticipantVisibilityPolicy::OwnerOnly
    );
    assert_eq!(
        memory_node.external_egress_policy,
        ExternalEgressPolicy::ApprovalRequired
    );
    assert!(memory_node.retrieval_hints.contains("context graph"));
    assert!(
        graph
            .entities
            .iter()
            .any(|node| node.entity_id == "concept:context_graph")
    );
    assert!(
        graph
            .subject_edges
            .iter()
            .any(|edge| edge.memory_id == memory.id && edge.entity_id == "human:kevin")
    );
    assert!(
        graph
            .participant_edges
            .iter()
            .any(|edge| edge.memory_id == memory.id && edge.principal_id == "agent:primary")
    );
    assert!(
        graph
            .provenance_edges
            .iter()
            .any(|edge| edge.memory_id == memory.id && edge.source_type == "episode")
    );
    assert!(graph.object_link_edges.iter().any(|edge| {
        edge.memory_id == memory.id
            && edge.object_type == "project"
            && edge.object_id == "project:noema"
            && edge.relation == "active_context"
            && edge.authorized_scope_id.as_deref() == Some("conversation:inspect_graph")
            && edge.resolver_principal_id.as_deref() == Some("agent:primary")
            && edge.resolver_version.as_deref() == Some("resolver-v1")
            && edge.source_run_id.as_deref() == Some("run:inspect_graph")
    }));
    assert!(graph.purpose_rules.iter().any(|rule| {
        rule.memory_id == memory.id
            && rule.purpose == Purpose::AnswerHumanQuestion
            && rule.effect == Effect::Allow
    }));
    assert!(graph.purpose_rules.iter().any(|rule| {
        rule.memory_id == memory.id
            && rule.purpose == Purpose::ExternalAction
            && rule.effect == Effect::Deny
    }));
    assert!(graph.access_grants.iter().any(|grant| {
        grant.grant_id == "grant_inspect_graph"
            && grant.memory_id.as_deref() == Some(memory.id.as_str())
            && grant.principal_id == "agent:primary"
            && grant.permission == "use_for_retrieval"
            && grant.effect == Effect::Allow
            && grant.expires_at.as_deref() == Some("2000-01-01 00:00:00")
    }));
    assert!(graph.access_grants.iter().any(|grant| {
        grant.grant_id == "grant_inspect_scope_write"
            && grant.scope_id.as_deref() == Some("conversation:inspect_graph")
            && grant.principal_id == "agent:primary"
            && grant.permission == "write"
            && grant.effect == Effect::Allow
    }));
    assert!(
        !graph
            .access_grants
            .iter()
            .any(|grant| grant.grant_id == "grant_unrelated_scope")
    );
    assert!(graph.memory_events.iter().any(|event| {
        event.event_id == "event_inspect_shown"
            && event.event_type == "shown_to_agent"
            && event.memory_id.as_deref() == Some(memory.id.as_str())
            && event.memory_sensitivity == Some(Sensitivity::Normal)
            && event.scope_id.as_deref() == Some("conversation:inspect_graph")
            && event.reason.as_deref() == Some("context_packet")
            && event.details.contains("run:inspect_graph")
    }));
    assert!(graph.memory_events.iter().any(|event| {
        event.event_id == "event_inspect_scope_retrieval"
            && event.event_type == "retrieved"
            && event.memory_id.is_none()
            && event.scope_id.as_deref() == Some("conversation:inspect_graph")
    }));
    assert!(
        !graph
            .memory_events
            .iter()
            .any(|event| event.event_id == "event_unrelated_scope")
    );
    assert!(graph.relationships.iter().any(|edge| {
        edge.subject_entity_id == "human:kevin"
            && edge.predicate == "prefers"
            && edge.object_entity_id == "concept:context_graph"
    }));
}
