use super::*;

#[test]
fn inspects_context_graph_from_canonical_tables() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut candidate =
        new_conversation_memory_candidate(&mut repo, "Kevin prefers inspectable context graphs.");
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
    if let Some(source) = &mut candidate.source {
        source.evidence_excerpt = Some("inspectable context graphs".to_string());
    }
    let memory = repo.append_memory_candidate(&candidate).expect("memory");
    let conversation_id = candidate.owner.object_id.clone();
    repo.conn
        .execute(
            r"
            UPDATE memory_items
            SET
              retrieval_policy_status = 'valid',
              retrieval_policy_version = 2,
              retrieval_policy_fingerprint = 'sha256:inspect_graph',
              retrieval_policy_extractor_object_type = 'agent',
              retrieval_policy_extractor_object_id = 'agent:primary',
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
        candidate.owner.clone(),
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
              resolver_object_type,
              resolver_object_id,
              resolver_version,
              source_run_id,
              authorized_object_type,
              authorized_object_id,
              created_by_object_type,
              created_by_object_id
            )
            VALUES (
              ?1,
              'project',
              'project:noema',
              'active_context',
              'agent',
              'agent:primary',
              'resolver-v1',
              'run:inspect_graph',
              'conversation',
              ?2,
              'agent',
              'agent:primary'
            )
            ",
            params![memory.id, conversation_id.as_str()],
        )
        .expect("object link");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_retrieval_purpose_rules (
              memory_id,
              purpose,
              effect,
              created_by_object_type,
              created_by_object_id
            )
            VALUES
              (?1, 'answer_human_question', 'allow', 'agent', 'agent:primary'),
              (?1, 'external_action', 'deny', 'agent', 'agent:primary')
            ",
            params![memory.id],
        )
        .expect("purpose rule");
    repo.conn
        .execute(
            r"
            INSERT INTO object_access_grants (
              grant_id,
              target_object_type,
              target_object_id,
              grantee_object_type,
              grantee_object_id,
              permission,
              effect,
              expires_at,
              created_by_object_type,
              created_by_object_id
            )
            VALUES (
              'grant_inspect_graph',
              'memory_item',
              ?1,
              'agent',
              'agent:primary',
              'use_for_retrieval',
              'allow',
              '2000-01-01 00:00:00',
              'human',
              'human:kevin'
            )
            ",
            params![memory.id],
        )
        .expect("access grant");
    repo.conn
        .execute(
            r"
            INSERT INTO object_access_grants (
              grant_id,
              target_object_type,
              target_object_id,
              grantee_object_type,
              grantee_object_id,
              permission,
              effect,
              created_by_object_type,
              created_by_object_id
            )
            VALUES (
              'grant_inspect_scope_write',
              'conversation',
              ?1,
              'agent',
              'agent:primary',
              'write',
              'allow',
              'human',
              'human:kevin'
            )
            ",
            params![conversation_id.as_str()],
        )
        .expect("scope access grant");
    repo.conn
        .execute(
            r"
            INSERT INTO object_access_grants (
              grant_id,
              target_object_type,
              target_object_id,
              grantee_object_type,
              grantee_object_id,
              permission,
              effect,
              created_by_object_type,
              created_by_object_id
            )
            VALUES (
              'grant_unrelated_scope',
              'conversation',
              'conversation:unrelated',
              'agent',
              'agent:primary',
              'use_for_retrieval',
              'allow',
              'human',
              'human:kevin'
            )
            ",
            [],
        )
        .expect("unrelated access grant");
    repo.conn
        .execute(
            r#"
            INSERT INTO object_events (
              event_id,
              event_type,
              actor_object_type,
              actor_object_id,
              target_object_type,
              target_object_id,
              reason,
              details
            )
            VALUES (
              'event_inspect_shown',
              'shown_to_agent',
              'agent',
              'agent:primary',
              'memory_item',
              ?1,
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
            INSERT INTO object_events (
              event_id,
              event_type,
              actor_object_type,
              actor_object_id,
              target_object_type,
              target_object_id,
              reason,
              details
            )
            VALUES (
              'event_inspect_scope_retrieval',
              'retrieved',
              'agent',
              'agent:primary',
              'conversation',
              ?1,
              'scope_context',
              '{"run_id":"run:inspect_graph"}'
            )
            "#,
            params![conversation_id.as_str()],
        )
        .expect("scope event");
    repo.conn
        .execute(
            r"
            INSERT INTO object_events (
              event_id,
              event_type,
              actor_object_type,
              actor_object_id,
              target_object_type,
              target_object_id,
              reason,
              details
            )
            VALUES (
              'event_unrelated_scope',
              'retrieved',
              'agent',
              'agent:primary',
              'conversation',
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
    assert_eq!(
        memory_node.content,
        "Kevin prefers inspectable context graphs."
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
            .any(|edge| edge.memory_id == memory.id
                && edge.participant_object_type == "agent"
                && edge.participant_object_id == "agent:primary")
    );
    assert!(
        graph
            .provenance_edges
            .iter()
            .any(|edge| edge.memory_id == memory.id
                && edge.source_object_type == "conversation_item")
    );
    assert!(graph.object_link_edges.iter().any(|edge| {
        edge.memory_id == memory.id
            && edge.object_type == "project"
            && edge.object_id == "project:noema"
            && edge.relation == "active_context"
            && edge.authorized_object_type.as_deref() == Some("conversation")
            && edge.authorized_object_id.as_deref() == Some(conversation_id.as_str())
            && edge.resolver_object_type.as_deref() == Some("agent")
            && edge.resolver_object_id.as_deref() == Some("agent:primary")
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
            && grant.target_object_type == "memory_item"
            && grant.target_object_id == memory.id
            && grant.grantee_object_type == "agent"
            && grant.grantee_object_id == "agent:primary"
            && grant.permission == "use_for_retrieval"
            && grant.effect == Effect::Allow
            && grant.expires_at.as_deref() == Some("2000-01-01 00:00:00")
    }));
    assert!(graph.access_grants.iter().any(|grant| {
        grant.grant_id == "grant_inspect_scope_write"
            && grant.target_object_type == "conversation"
            && grant.target_object_id == conversation_id
            && grant.grantee_object_type == "agent"
            && grant.grantee_object_id == "agent:primary"
            && grant.permission == "write"
            && grant.effect == Effect::Allow
    }));
    assert!(
        !graph
            .access_grants
            .iter()
            .any(|grant| grant.grant_id == "grant_unrelated_scope")
    );
    assert!(graph.object_events.iter().any(|event| {
        event.event_id == "event_inspect_shown"
            && event.event_type == "shown_to_agent"
            && event.target_object_type.as_deref() == Some("memory_item")
            && event.target_object_id.as_deref() == Some(memory.id.as_str())
            && event.target_memory_sensitivity == Some(Sensitivity::Normal)
            && event.reason.as_deref() == Some("context_packet")
            && event.details.contains("run:inspect_graph")
    }));
    assert!(graph.object_events.iter().any(|event| {
        event.event_id == "event_inspect_scope_retrieval"
            && event.event_type == "retrieved"
            && event.target_object_type.as_deref() == Some("conversation")
            && event.target_object_id.as_deref() == Some(conversation_id.as_str())
    }));
    assert!(
        !graph
            .object_events
            .iter()
            .any(|event| event.event_id == "event_unrelated_scope")
    );
    assert!(graph.relationships.iter().any(|edge| {
        edge.subject_entity_id == "human:kevin"
            && edge.predicate == "prefers"
            && edge.object_entity_id == "concept:context_graph"
    }));
}
