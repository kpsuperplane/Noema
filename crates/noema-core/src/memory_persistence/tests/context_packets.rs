use super::*;

#[test]
fn records_context_packet_with_omissions_and_memory_use_records() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
    let mut included = NewChatMemoryCandidate::new(
        "conversation:packet",
        "Noema should record context packet manifests.",
        "agent:primary",
    );
    included.status = MemoryStatus::Active;
    included.participants = vec![NewMemoryParticipant::new(
        "human:kevin",
        ParticipantRole::HumanInScope,
    )];
    let included_memory = repo
        .append_chat_memory_candidate(&included)
        .expect("included memory");

    let mut denied = NewChatMemoryCandidate::new(
        "conversation:packet",
        "Sensitive packet detail should stay audit-only.",
        "agent:primary",
    );
    denied.status = MemoryStatus::Active;
    denied.sensitivity = Sensitivity::Sensitive;
    denied.participants = vec![NewMemoryParticipant::new(
        "human:kevin",
        ParticipantRole::HumanInScope,
    )];
    let denied_memory = repo
        .append_chat_memory_candidate(&denied)
        .expect("denied memory");

    let mut request = request_for_kevin();
    request
        .trusted
        .active_scopes
        .push("conversation:packet".to_string());
    let result = repo.retrieve_memories(&request).expect("retrieve");

    assert_eq!(included_ids(&result), vec![included_memory.id.as_str()]);
    assert!(
        result
            .use_records
            .iter()
            .all(|record| record.stage == MemoryUseStage::Retrieved)
    );
    assert!(result.denied_for_audit.iter().any(|denial| {
        denial.memory_id.as_deref() == Some(denied_memory.id.as_str())
            && denial.reason == DenialReason::SensitivityCeiling
    }));

    repo.record_context_packet("ctx_packet", "run:packet", &request, &result)
        .expect("record packet");
    repo.record_context_packet("ctx_other", "run:other", &request, &result)
        .expect("record other packet");
    let mut unrelated = NewChatMemoryCandidate::new(
        "conversation:unrelated_packet",
        "This memory is newer but not part of the packet.",
        "agent:primary",
    );
    unrelated.status = MemoryStatus::Active;
    unrelated.participants = vec![NewMemoryParticipant::new(
        "human:someone_else",
        ParticipantRole::HumanInScope,
    )];
    let unrelated_memory = repo
        .append_chat_memory_candidate(&unrelated)
        .expect("unrelated memory");

    let mut relationship_backing = NewChatMemoryCandidate::new(
        "conversation:packet",
        "Relationship-only omissions still need supporting memory context.",
        "agent:primary",
    );
    relationship_backing.status = MemoryStatus::Active;
    relationship_backing.participants = vec![NewMemoryParticipant::new(
        "human:kevin",
        ParticipantRole::HumanInScope,
    )];
    relationship_backing.subjects = vec![
        NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
        NewMemorySubject::new(
            "concept:packet_relationship",
            "concept",
            "Packet Relationship",
            SubjectRole::Target,
        ),
    ];
    let relationship_memory = repo
        .append_chat_memory_candidate(&relationship_backing)
        .expect("relationship memory");
    let mut relationship = NewRelationshipClaim::new(
        "conversation:packet",
        "human:kevin",
        "prefers",
        "concept:packet_relationship",
    );
    relationship.status = RelationshipStatus::Active;
    relationship.memory_id = Some(relationship_memory.id.clone());
    let relationship = repo
        .append_relationship_claim(&relationship)
        .expect("relationship");
    repo.conn
        .execute(
            r#"
            INSERT INTO context_packet_omissions (
              omission_id,
              context_packet_id,
              relationship_id,
              omission_sensitivity,
              agent_visible_reason,
              audit_reason,
              details
            )
            VALUES (
              'ctxomit_relationship_only',
              'ctx_packet',
              ?1,
              'normal',
              'policy_restricted_context',
              'outside_search_aperture',
              '{"source":"test"}'
            )
            "#,
            params![relationship.relationship_id],
        )
        .expect("relationship-only omission");
    repo.conn
        .execute(
            r"
            INSERT INTO memory_access_grants (
              grant_id,
              scope_id,
              principal_id,
              permission,
              effect
            )
            VALUES (
              'grant_same_scope_unrelated',
              'conversation:packet',
              'agent:primary',
              'use_for_retrieval',
              'allow'
            )
            ",
            [],
        )
        .expect("same-scope grant");
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
              'event_same_scope_unrelated',
              'retrieved',
              'agent:primary',
              'conversation:packet',
              'same_scope_unrelated',
              '{"run_id":"run:other"}'
            )
            "#,
            [],
        )
        .expect("same-scope event");
    let graph = repo.inspect_context_graph(Some(20)).expect("graph");

    assert!(graph.context_packets.iter().any(|packet| {
        packet.context_packet_id == "ctx_packet"
            && packet.run_id == "run:packet"
            && packet
                .agent_visible_omissions
                .contains("policy_restricted_context")
    }));
    assert!(graph.context_packet_memory_edges.iter().any(|edge| {
        edge.context_packet_id == "ctx_packet"
            && edge.memory_id == included_memory.id
            && edge.stage == "included_in_packet"
    }));
    assert!(graph.context_packet_omissions.iter().any(|omission| {
        omission.context_packet_id == "ctx_packet"
            && omission.memory_id.as_deref() == Some(denied_memory.id.as_str())
            && omission.omission_sensitivity == Sensitivity::Sensitive
            && omission.agent_visible_reason == "policy_restricted_context"
            && omission.audit_reason == "sensitivity_ceiling"
    }));
    assert!(graph.memory_use_records.iter().any(|record| {
        record.context_packet_id.as_deref() == Some("ctx_packet")
            && record.run_id == "run:packet"
            && record.memory_id == included_memory.id
            && record.stage == "retrieved"
    }));
    assert!(graph.memory_use_records.iter().any(|record| {
        record.context_packet_id.as_deref() == Some("ctx_packet")
            && record.run_id == "run:packet"
            && record.memory_id == included_memory.id
            && record.stage == "included_in_packet"
    }));

    let filtered = repo
        .inspect_context_graph_with_filter(
            &ContextGraphFilter {
                run_id: Some("run:packet".to_string()),
                context_packet_id: None,
            },
            Some(20),
        )
        .expect("filtered graph");
    assert_eq!(filtered.context_packets.len(), 1);
    assert_eq!(filtered.context_packets[0].context_packet_id, "ctx_packet");
    assert!(
        !filtered
            .memories
            .iter()
            .any(|memory| memory.memory_id == unrelated_memory.id)
    );
    assert!(
        filtered
            .memories
            .iter()
            .any(|memory| memory.memory_id == relationship_memory.id)
    );
    assert!(
        filtered
            .relationships
            .iter()
            .any(|edge| edge.relationship_id == relationship.relationship_id)
    );
    assert!(
        !filtered
            .access_grants
            .iter()
            .any(|grant| grant.grant_id == "grant_same_scope_unrelated")
    );
    assert!(
        !filtered
            .memory_events
            .iter()
            .any(|event| event.event_id == "event_same_scope_unrelated")
    );
    assert!(
        filtered
            .memory_use_records
            .iter()
            .all(|record| record.run_id == "run:packet")
    );
    assert!(
        filtered
            .context_packet_omissions
            .iter()
            .all(|omission| omission.context_packet_id == "ctx_packet")
    );

    let packet_filtered = repo
        .inspect_context_graph_with_filter(
            &ContextGraphFilter {
                run_id: None,
                context_packet_id: Some("ctx_packet".to_string()),
            },
            Some(20),
        )
        .expect("packet-filtered graph");
    assert_eq!(packet_filtered.context_packets.len(), 1);
    assert_eq!(
        packet_filtered.context_packets[0].context_packet_id,
        "ctx_packet"
    );
    assert!(
        !packet_filtered
            .context_packets
            .iter()
            .any(|packet| packet.context_packet_id == "ctx_other")
    );
}
