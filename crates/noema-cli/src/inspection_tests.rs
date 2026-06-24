use super::*;
use noema_core::{
    GraphAccessGrant, GraphContextPacket, GraphContextPacketMemoryEdge, GraphContextPacketOmission,
    GraphEntityNode, GraphMemoryEvent, GraphMemoryNode, GraphMemoryUseRecord, GraphObjectLinkEdge,
    GraphParticipantEdge, GraphProvenanceEdge, GraphPurposeRule, GraphSubjectEdge,
    memory_persistence::MemoryType,
};

#[test]
fn redacts_non_public_memory_list_titles() {
    let mut memory = MemorySummary {
        id: "mem_123".to_string(),
        status: MemoryStatus::Confirmed,
        memory_type: MemoryType::Note,
        home_scope_id: "conversation:conversation_1".to_string(),
        sensitivity: Sensitivity::Normal,
        title: "my API key is sk-test1234567890".to_string(),
        content: "my API key is sk-test1234567890".to_string(),
        created_at: "2026-06-24 12:00:00".to_string(),
        source_type: Some("episode".to_string()),
        source_id: Some("conversation:conversation_1".to_string()),
        conversation_id: Some("conversation:conversation_1".to_string()),
    };

    assert_eq!(
        redacted_list_title(&memory),
        "[redacted; use memory show <id>]"
    );

    memory.sensitivity = Sensitivity::Public;
    memory.title = "Public project note".to_string();
    assert_eq!(redacted_list_title(&memory), "Public project note");
}

#[test]
fn context_graph_output_includes_object_links_and_redacts_memory_titles() {
    let graph = ContextGraphSummary {
        memories: vec![GraphMemoryNode {
            memory_id: "mem_sensitive".to_string(),
            status: MemoryStatus::Confirmed,
            memory_type: MemoryType::OpenLoop,
            home_scope_id: "conversation:health".to_string(),
            sensitivity: Sensitivity::Sensitive,
            title: "Doctor follow-up detail".to_string(),
            retrieval_hints: r#"{"topics":["doctor"]}"#.to_string(),
            retrieval_policy_status: RetrievalPolicyStatus::Valid,
            retrieval_policy_effective_status: RetrievalPolicyStatus::Stale,
            retrieval_policy_version: 2,
            retrieval_policy_fingerprint: Some("sha256:abcdef1234567890".to_string()),
            retrieval_policy_extractor_principal_id: Some("agent:primary".to_string()),
            retrieval_policy_extractor_version: Some("extractor-v1".to_string()),
            retrieval_policy_validated_at: Some("2026-06-24 12:00:00".to_string()),
            participant_visibility_policy: ParticipantVisibilityPolicy::OwnerOnly,
            external_egress_policy: ExternalEgressPolicy::ApprovalRequired,
            created_at: "2026-06-24 12:00:00".to_string(),
        }],
        entities: Vec::<GraphEntityNode>::new(),
        subject_edges: Vec::<GraphSubjectEdge>::new(),
        participant_edges: Vec::<GraphParticipantEdge>::new(),
        provenance_edges: Vec::<GraphProvenanceEdge>::new(),
        object_link_edges: vec![GraphObjectLinkEdge {
            memory_id: "mem_sensitive".to_string(),
            object_type: "task".to_string(),
            object_id: "task:schedule_checkup".to_string(),
            relation: "open_loop_for".to_string(),
            authorized_scope_id: Some("conversation:health".to_string()),
            resolver_principal_id: Some("agent:primary".to_string()),
            resolver_version: Some("resolver-v1".to_string()),
            source_run_id: Some("run:health".to_string()),
            created_at: "2026-06-24 12:00:00".to_string(),
        }],
        purpose_rules: vec![
            GraphPurposeRule {
                memory_id: "mem_sensitive".to_string(),
                purpose: Purpose::AnswerHumanQuestion,
                effect: Effect::Allow,
                created_by_principal_id: Some("agent:primary".to_string()),
                created_at: "2026-06-24 12:00:00".to_string(),
            },
            GraphPurposeRule {
                memory_id: "mem_sensitive".to_string(),
                purpose: Purpose::ExternalAction,
                effect: Effect::Deny,
                created_by_principal_id: Some("agent:primary".to_string()),
                created_at: "2026-06-24 12:00:00".to_string(),
            },
        ],
        access_grants: vec![GraphAccessGrant {
            grant_id: "grant_1".to_string(),
            memory_id: Some("mem_sensitive".to_string()),
            scope_id: None,
            principal_id: "agent:primary".to_string(),
            permission: "use_for_retrieval".to_string(),
            effect: Effect::Allow,
            expires_at: Some("2000-01-01 00:00:00".to_string()),
            created_by_principal_id: Some("human:kevin".to_string()),
            created_at: "2026-06-24 12:00:00".to_string(),
        }],
        context_packets: vec![GraphContextPacket {
            context_packet_id: "ctx_health".to_string(),
            run_id: "run:health".to_string(),
            requesting_principal_id: "agent:primary".to_string(),
            purpose: Purpose::AnswerHumanQuestion,
            active_scopes: r#"["conversation:health"]"#.to_string(),
            agent_visible_omissions: r#"[{"reason":"policy_restricted_context"}]"#.to_string(),
            created_at: "2026-06-24 12:00:00".to_string(),
        }],
        context_packet_memory_edges: vec![GraphContextPacketMemoryEdge {
            packet_memory_id: "ctxmem_health".to_string(),
            context_packet_id: "ctx_health".to_string(),
            memory_id: "mem_sensitive".to_string(),
            memory_sensitivity: Sensitivity::Sensitive,
            stage: "included_in_packet".to_string(),
            rank_score: Some(180),
            eligibility_reason: Some("trusted_object_link".to_string()),
            rank_reasons: r#"["doctor"]"#.to_string(),
            created_at: "2026-06-24 12:00:00".to_string(),
        }],
        context_packet_omissions: vec![GraphContextPacketOmission {
            omission_id: "ctxomit_health".to_string(),
            context_packet_id: "ctx_health".to_string(),
            memory_id: Some("mem_sensitive".to_string()),
            relationship_id: Some("rel_health".to_string()),
            omission_sensitivity: Sensitivity::Sensitive,
            agent_visible_reason: "policy_restricted_context".to_string(),
            audit_reason: "sensitive_unlock_missing".to_string(),
            created_at: "2026-06-24 12:00:00".to_string(),
            details: r#"{"denied_topic":"doctor"}"#.to_string(),
        }],
        memory_use_records: vec![GraphMemoryUseRecord {
            memory_use_id: "memuse_health".to_string(),
            context_packet_id: Some("ctx_health".to_string()),
            run_id: "run:health".to_string(),
            memory_id: "mem_sensitive".to_string(),
            memory_sensitivity: Sensitivity::Sensitive,
            stage: "included_in_packet".to_string(),
            agent_principal_id: Some("agent:primary".to_string()),
            scope_id: Some("conversation:health".to_string()),
            purpose: Purpose::AnswerHumanQuestion,
            used_for_object_type: None,
            used_for_object_id: None,
            policy_decision_id: None,
            created_at: "2026-06-24 12:00:00".to_string(),
            details: r#"{"context_packet_id":"ctx_health"}"#.to_string(),
        }],
        memory_events: vec![GraphMemoryEvent {
            event_id: "event_memory_shown".to_string(),
            event_type: "shown_to_agent".to_string(),
            actor_principal_id: Some("agent:primary".to_string()),
            memory_id: Some("mem_sensitive".to_string()),
            memory_sensitivity: Some(Sensitivity::Sensitive),
            scope_id: Some("conversation:health".to_string()),
            reason: Some("context_packet".to_string()),
            created_at: "2026-06-24 12:00:00".to_string(),
            details: r#"{"run_id":"run:health","quote":"doctor"}"#.to_string(),
        }],
        relationships: Vec::new(),
    };
    let mut output = Vec::new();

    write_context_graph(&mut output, &graph).expect("write graph");
    let output = String::from_utf8(output).expect("utf8 output");

    assert!(output.contains("object_link_edges=1"));
    assert!(output.contains("purpose_rules=2"));
    assert!(output.contains("access_grants=1"));
    assert!(output.contains("context_packets=1"));
    assert!(output.contains("packet_memory_edges=1"));
    assert!(output.contains("packet_omissions=1"));
    assert!(output.contains("memory_use_records=1"));
    assert!(output.contains("memory_events=1"));
    assert!(output.contains("Retrieval policy"));
    assert!(output.contains("Trusted object links"));
    assert!(output.contains("Purpose rules"));
    assert!(output.contains("Access grants"));
    assert!(output.contains("Context packets"));
    assert!(output.contains("Context packet memories"));
    assert!(output.contains("Context packet omissions"));
    assert!(output.contains("Memory use records"));
    assert!(output.contains("Memory events"));
    assert!(output.contains("owner_only"));
    assert!(output.contains("stale"));
    assert!(output.contains("agent:primary"));
    assert!(output.contains("extractor-v1"));
    assert!(output.contains("approval_required"));
    assert!(output.contains("answer_human_question"));
    assert!(output.contains("external_action"));
    assert!(output.contains("deny"));
    assert!(output.contains("use_for_retrieval"));
    assert!(output.contains("2000-01-01 00:00:00"));
    assert!(output.contains("ctx_health"));
    assert!(output.contains("included_in_packet"));
    assert!(output.contains("policy_restricted_context"));
    assert!(output.contains("sensitive_unlock_missing"));
    assert!(output.contains("shown_to_agent"));
    assert!(output.contains("context_packet"));
    assert!(output.contains("task:schedule_checkup"));
    assert!(output.contains("open_loop_for"));
    assert!(output.contains("conversation:health"));
    assert!(output.contains("[redacted; use memory show <id>]"));
    assert!(!output.contains("Doctor follow-up detail"));
    assert!(!output.contains(r#"{"topics":["doctor"]}"#));
    assert!(!output.contains(r#"["doctor"]"#));
    assert!(!output.contains(r#"{"denied_topic":"doctor"}"#));
    assert!(!output.contains(r#"{"context_packet_id":"ctx_health"}"#));
    assert!(!output.contains(r#"{"run_id":"run:health","quote":"doctor"}"#));
}
