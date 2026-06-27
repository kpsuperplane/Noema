use super::*;

fn normal_memory(id: &str, scope: &str, title: &str) -> MemoryItem {
    let mut memory = MemoryItem::new(id, scope, title, title);
    memory.status = MemoryStatus::Confirmed;
    memory.retrieval_policy_status = RetrievalPolicyStatus::Valid;
    memory.participant_visibility_policy = ParticipantVisibilityPolicy::AnyActiveHuman;
    memory
}

fn sensitive_memory(id: &str, scope: &str, title: &str, owner: &str) -> MemoryItem {
    let mut memory = MemoryItem::new(id, scope, title, title);
    memory.status = MemoryStatus::Confirmed;
    memory.sensitivity = Sensitivity::Sensitive;
    memory.retrieval_policy_status = RetrievalPolicyStatus::Valid;
    memory.participant_visibility_policy = ParticipantVisibilityPolicy::OwnerOnly;
    memory.owner_principal_id = Some(owner.to_string());
    memory
}

fn request_for_kevin() -> MemoryRetrievalRequest {
    MemoryRetrievalRequest {
        requesting_principal_id: "agent_primary".to_string(),
        trusted: TrustedRetrievalContext::for_human("human_kevin", Purpose::AnswerHumanQuestion),
        untrusted_hints: UntrustedHints::default(),
    }
}

#[test]
fn retrieves_normal_memory_across_same_human_participant() {
    let mut store = MemoryStore::default();
    let mut memory = normal_memory(
        "memory_pref",
        "conversation_old",
        "Current conversation should rank first",
    );
    memory.retrieval_hints.topics = vec!["memory".to_string()];
    store.insert_memory(memory);
    store
        .add_participant("memory_pref", "human_kevin", ParticipantRole::HumanInScope)
        .expect("participant");

    let mut request = request_for_kevin();
    request.untrusted_hints.fuzzy_topics = vec!["memory".to_string()];

    let result = store.retrieve(&request);

    assert_eq!(included_ids(&result), vec!["memory_pref"]);
    assert_eq!(
        result.included[0].eligibility_reason,
        EligibilityReason::ParticipantOverlap
    );
    assert!(result.denied_for_audit.is_empty());
}

#[test]
fn active_scope_ranks_above_older_same_human_memory() {
    let mut store = MemoryStore::default();
    let mut old = normal_memory("memory_old", "conversation_old", "Older preference");
    old.retrieval_hints.topics = vec!["preference".to_string()];
    store.insert_memory(old);
    store
        .add_participant("memory_old", "human_kevin", ParticipantRole::HumanInScope)
        .expect("old participant");

    let mut current = normal_memory("memory_current", "conversation_current", "Current fact");
    current.retrieval_hints.topics = vec!["preference".to_string()];
    store.insert_memory(current);

    let mut request = request_for_kevin();
    request
        .trusted
        .active_scopes
        .push("conversation_current".to_string());
    request.untrusted_hints.fuzzy_topics = vec!["preference".to_string()];

    let result = store.retrieve(&request);

    assert_eq!(included_ids(&result), vec!["memory_current", "memory_old"]);
    assert_eq!(
        result.included[0].eligibility_reason,
        EligibilityReason::ActiveScope
    );
}

#[test]
fn fuzzy_hints_do_not_unlock_sensitive_memory() {
    let mut store = MemoryStore::default();
    let mut memory = sensitive_memory(
        "memory_health",
        "conversation_health",
        "Doctor appointment follow-up",
        "human_kevin",
    );
    memory.retrieval_hints.topics = vec!["health".to_string(), "doctor".to_string()];
    store.insert_memory(memory);
    store
        .add_participant(
            "memory_health",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("participant");
    store
        .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
        .expect("purpose");

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
    request.untrusted_hints.fuzzy_topics = vec!["health".to_string(), "doctor".to_string()];

    let result = store.retrieve(&request);

    assert!(result.included.is_empty());
    assert_eq!(
        result.denied_for_audit[0].reason,
        DenialReason::SensitiveUnlockMissing
    );
    assert_eq!(
        result.agent_visible_omissions,
        vec![AgentVisibleOmission {
            reason: "policy_restricted_context"
        }]
    );
}

#[test]
fn trusted_object_link_unlocks_sensitive_memory() {
    let mut store = MemoryStore::default();
    let memory = sensitive_memory(
        "memory_health",
        "conversation_health",
        "Doctor appointment follow-up",
        "human_kevin",
    );
    store.insert_memory(memory);
    store
        .add_participant(
            "memory_health",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("participant");
    store
        .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
        .expect("purpose");
    store
        .add_object_link(
            "memory_health",
            ObjectLink::new("task", "task_schedule_checkup"),
            "open_loop_for",
            None,
        )
        .expect("object link");

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
    request
        .trusted
        .active_object_links
        .push(ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for"));

    let result = store.retrieve(&request);

    assert_eq!(included_ids(&result), vec!["memory_health"]);
    assert_eq!(
        result.included[0].eligibility_reason,
        EligibilityReason::TrustedObjectLink
    );
}

#[test]
fn trusted_object_link_requires_matching_relation() {
    let mut store = MemoryStore::default();
    let memory = sensitive_memory(
        "memory_health",
        "conversation_health",
        "Doctor appointment follow-up",
        "human_kevin",
    );
    store.insert_memory(memory);
    store
        .add_participant(
            "memory_health",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("participant");
    store
        .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
        .expect("purpose");
    store
        .add_object_link(
            "memory_health",
            ObjectLink::new("task", "task_schedule_checkup"),
            "open_loop_for",
            None,
        )
        .expect("object link");

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
    request
        .trusted
        .active_object_links
        .push(ObjectLink::new("task", "task_schedule_checkup"));
    let missing_relation = store.retrieve(&request);
    assert!(missing_relation.included.is_empty());
    assert_eq!(
        missing_relation.denied_for_audit[0].reason,
        DenialReason::SensitiveUnlockMissing
    );

    request.trusted.active_object_links =
        vec![ObjectLink::new("task", "task_schedule_checkup").with_relation("relevant_to")];
    let wrong_relation = store.retrieve(&request);
    assert!(wrong_relation.included.is_empty());
    assert_eq!(
        wrong_relation.denied_for_audit[0].reason,
        DenialReason::SensitiveUnlockMissing
    );

    request.trusted.active_object_links =
        vec![ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for")];
    let allowed = store.retrieve(&request);
    assert_eq!(included_ids(&allowed), vec!["memory_health"]);
}

#[test]
fn trusted_object_link_requires_authorized_actor_when_policy_sets_one() {
    let mut store = MemoryStore::default();
    let memory = sensitive_memory(
        "memory_health",
        "conversation_health",
        "Doctor appointment follow-up",
        "human_kevin",
    );
    store.insert_memory(memory);
    store
        .add_participant(
            "memory_health",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("participant");
    store
        .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
        .expect("purpose");
    store
        .add_object_link(
            "memory_health",
            ObjectLink::new("task", "task_schedule_checkup"),
            "open_loop_for",
            Some("agent_authorizer".to_string()),
        )
        .expect("object link");

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
    request.trusted.active_object_links =
        vec![ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for")];
    let missing_actor = store.retrieve(&request);
    assert!(missing_actor.included.is_empty());
    assert_eq!(
        missing_actor.denied_for_audit[0].reason,
        DenialReason::SensitiveUnlockMissing
    );

    request.trusted.active_object_links = vec![
        ObjectLink::new("task", "task_schedule_checkup")
            .with_relation("open_loop_for")
            .with_authorized_actor("other_agent"),
    ];
    let wrong_actor = store.retrieve(&request);
    assert!(wrong_actor.included.is_empty());
    assert_eq!(
        wrong_actor.denied_for_audit[0].reason,
        DenialReason::SensitiveUnlockMissing
    );

    request.trusted.active_object_links =
        vec![ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for")];
    request.trusted.active_agent_ids = vec!["agent_authorizer".to_string()];
    let active_agent_allowed = store.retrieve(&request);
    assert_eq!(included_ids(&active_agent_allowed), vec!["memory_health"]);

    request.trusted.active_agent_ids.clear();
    request.requesting_principal_id = "agent_authorizer".to_string();
    let requesting_principal_allowed = store.retrieve(&request);
    assert_eq!(
        included_ids(&requesting_principal_allowed),
        vec!["memory_health"]
    );
}

#[test]
fn stale_sensitive_policy_fails_closed() {
    let mut store = MemoryStore::default();
    let mut memory = sensitive_memory(
        "memory_health",
        "conversation_health",
        "Doctor appointment follow-up",
        "human_kevin",
    );
    memory.retrieval_policy_status = RetrievalPolicyStatus::Stale;
    store.insert_memory(memory);
    store
        .add_participant(
            "memory_health",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("participant");
    store
        .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
        .expect("purpose");
    store
        .add_object_link(
            "memory_health",
            ObjectLink::new("task", "task_schedule_checkup"),
            "open_loop_for",
            None,
        )
        .expect("object link");

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
    request
        .trusted
        .active_object_links
        .push(ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for"));

    let result = store.retrieve(&request);

    assert!(result.included.is_empty());
    assert_eq!(
        result.denied_for_audit[0].reason,
        DenialReason::RetrievalPolicyInvalid
    );
}

#[test]
fn private_memory_requires_active_scope_or_explicit_grant() {
    let mut store = MemoryStore::default();
    let mut memory = normal_memory("memory_private", "conversation_private", "Private note");
    memory.sensitivity = Sensitivity::Private;
    memory.participant_visibility_policy = ParticipantVisibilityPolicy::ExplicitGrantOnly;
    store.insert_memory(memory);
    store
        .add_participant(
            "memory_private",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("participant");
    store
        .add_purpose_rule(
            "memory_private",
            Purpose::AnswerHumanQuestion,
            Effect::Allow,
        )
        .expect("purpose");

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Private;

    let denied = store.retrieve(&request);
    assert!(denied.included.is_empty());

    store.add_access_grant(AccessGrant {
        memory_id: Some("memory_private".to_string()),
        scope_id: None,
        principal_id: "agent_primary".to_string(),
        permission: Permission::UseForRetrieval,
        effect: Effect::Allow,
    });

    let allowed = store.retrieve(&request);
    assert_eq!(included_ids(&allowed), vec!["memory_private"]);
}

#[test]
fn private_memory_applies_participant_visibility_policy() {
    let mut store = MemoryStore::default();
    let mut memory = normal_memory("memory_private", "conversation_private", "Private note");
    memory.sensitivity = Sensitivity::Private;
    memory.participant_visibility_policy = ParticipantVisibilityPolicy::OwnerOnly;
    memory.owner_principal_id = Some("human_alex".to_string());
    store.insert_memory(memory);
    store
        .add_participant(
            "memory_private",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("participant");
    store
        .add_purpose_rule(
            "memory_private",
            Purpose::AnswerHumanQuestion,
            Effect::Allow,
        )
        .expect("purpose");

    let mut request = request_for_kevin();
    request.trusted.sensitivity_ceiling = Sensitivity::Private;
    request
        .trusted
        .active_scopes
        .push("conversation_private".to_string());
    let denied = store.retrieve(&request);

    assert!(denied.included.is_empty());
    assert_eq!(
        denied.denied_for_audit[0].reason,
        DenialReason::ParticipantVisibilityDenied
    );
}

#[test]
fn external_egress_policy_denies_external_purposes() {
    let mut store = MemoryStore::default();
    let mut memory = normal_memory("memory_external", "project_noema", "External draft note");
    memory.external_egress_policy = ExternalEgressPolicy::ApprovalRequired;
    store.insert_memory(memory);
    let mut request = MemoryRetrievalRequest {
        requesting_principal_id: "agent_primary".to_string(),
        trusted: TrustedRetrievalContext::for_human("human_kevin", Purpose::DraftExternalContent),
        untrusted_hints: UntrustedHints::default(),
    };
    request
        .trusted
        .active_scopes
        .push("project_noema".to_string());

    let approval_required = store.retrieve(&request);
    assert!(approval_required.included.is_empty());
    assert_eq!(
        approval_required.denied_for_audit[0].reason,
        DenialReason::ExternalEgressApprovalRequired
    );

    let mut allowed = normal_memory("memory_allowed_external", "project_noema", "Allowed");
    allowed.external_egress_policy = ExternalEgressPolicy::Allow;
    store.insert_memory(allowed);
    let allowed = store.retrieve(&request);
    assert!(
        allowed
            .included
            .iter()
            .any(|memory| memory.memory_id == "memory_allowed_external")
    );

    let mut denied = normal_memory("memory_denied_external", "project_noema", "Denied");
    denied.external_egress_policy = ExternalEgressPolicy::Deny;
    store.insert_memory(denied);
    let denied = store.retrieve(&request);
    assert!(denied.denied_for_audit.iter().any(|denial| {
        denial.memory_id.as_deref() == Some("memory_denied_external")
            && denial.reason == DenialReason::ExternalEgressDenied
    }));
}

#[test]
fn active_relationship_requires_supporting_memory_and_provenance() {
    let mut store = MemoryStore::default();
    store.insert_memory(normal_memory(
        "memory_relation",
        "project_noema",
        "Graph decision",
    ));

    let missing_memory = Relationship {
        relationship_id: "rel_missing".to_string(),
        home_scope_id: "project_noema".to_string(),
        subject_entity_id: "project_noema".to_string(),
        predicate: "uses".to_string(),
        object_entity_id: "concept_memory".to_string(),
        memory_id: None,
        status: RelationshipStatus::Active,
    };
    assert_eq!(
        store.add_relationship(missing_memory),
        Err(MemoryStoreError::RelationshipRequiresMemory {
            relationship_id: "rel_missing".to_string()
        })
    );

    let no_provenance = Relationship {
        relationship_id: "rel_no_provenance".to_string(),
        home_scope_id: "project_noema".to_string(),
        subject_entity_id: "project_noema".to_string(),
        predicate: "uses".to_string(),
        object_entity_id: "concept_memory".to_string(),
        memory_id: Some("memory_relation".to_string()),
        status: RelationshipStatus::Active,
    };
    assert_eq!(
        store.add_relationship(no_provenance),
        Err(MemoryStoreError::RelationshipRequiresProvenance {
            relationship_id: "rel_no_provenance".to_string(),
            memory_id: "memory_relation".to_string()
        })
    );

    store
        .add_provenance("memory_relation", "message_1")
        .expect("provenance");
    let supported = Relationship {
        relationship_id: "rel_supported".to_string(),
        home_scope_id: "project_noema".to_string(),
        subject_entity_id: "project_noema".to_string(),
        predicate: "uses".to_string(),
        object_entity_id: "concept_memory".to_string(),
        memory_id: Some("memory_relation".to_string()),
        status: RelationshipStatus::Active,
    };
    assert!(store.add_relationship(supported).is_ok());
}

#[test]
fn one_hop_graph_expansion_returns_backing_memory_only_after_policy() {
    let mut store = MemoryStore::default();
    let anchor = normal_memory("memory_anchor", "project_noema", "Noema memory system");
    store.insert_memory(anchor);
    store
        .add_subject("memory_anchor", "project_noema", SubjectRole::About)
        .expect("anchor subject");

    let backing = normal_memory(
        "memory_relationship",
        "relationship_scope",
        "Noema uses scoped graph claims",
    );
    store.insert_memory(backing);
    store
        .add_participant(
            "memory_relationship",
            "human_kevin",
            ParticipantRole::HumanInScope,
        )
        .expect("relationship participant");
    store
        .add_provenance("memory_relationship", "message_graph")
        .expect("provenance");
    store
        .add_relationship(Relationship {
            relationship_id: "rel_graph".to_string(),
            home_scope_id: "project_noema".to_string(),
            subject_entity_id: "project_noema".to_string(),
            predicate: "uses".to_string(),
            object_entity_id: "concept_scoped_graph_claims".to_string(),
            memory_id: Some("memory_relationship".to_string()),
            status: RelationshipStatus::Active,
        })
        .expect("relationship");
    store.insert_memory(normal_memory(
        "memory_unscoped_relationship",
        "relationship_scope",
        "Unscoped graph claim",
    ));
    store
        .add_provenance("memory_unscoped_relationship", "message_graph_unscoped")
        .expect("unscoped provenance");
    store
        .add_relationship(Relationship {
            relationship_id: "rel_unscoped_graph".to_string(),
            home_scope_id: "project_noema".to_string(),
            subject_entity_id: "project_noema".to_string(),
            predicate: "mentions".to_string(),
            object_entity_id: "concept_unscoped".to_string(),
            memory_id: Some("memory_unscoped_relationship".to_string()),
            status: RelationshipStatus::Active,
        })
        .expect("unscoped relationship");

    let mut request = request_for_kevin();
    request
        .trusted
        .active_scopes
        .push("project_noema".to_string());

    let result = store.retrieve(&request);

    assert_eq!(
        included_ids(&result),
        vec!["memory_anchor", "memory_relationship"]
    );
    assert!(
        result
            .included
            .iter()
            .any(|memory| memory.memory_id == "memory_relationship"
                && memory.eligibility_reason == EligibilityReason::ParticipantOverlap)
    );
    assert!(result.denied_for_audit.iter().any(|denial| {
        denial.memory_id.as_deref() == Some("memory_unscoped_relationship")
            && denial.reason == DenialReason::OutsideSearchAperture
    }));
}

#[test]
fn denied_graph_edge_does_not_become_agent_visible_detail() {
    let mut store = MemoryStore::default();
    let anchor = normal_memory("memory_anchor", "project_noema", "Noema memory system");
    store.insert_memory(anchor);
    store
        .add_subject("memory_anchor", "project_noema", SubjectRole::About)
        .expect("anchor subject");

    let mut backing = normal_memory("memory_relationship", "secret_scope", "Private edge");
    backing.sensitivity = Sensitivity::Private;
    backing.retrieval_policy_status = RetrievalPolicyStatus::Invalid;
    store.insert_memory(backing);
    store
        .add_provenance("memory_relationship", "message_graph")
        .expect("provenance");
    store
        .add_relationship(Relationship {
            relationship_id: "rel_private".to_string(),
            home_scope_id: "secret_scope".to_string(),
            subject_entity_id: "project_noema".to_string(),
            predicate: "reveals_private_edge".to_string(),
            object_entity_id: "concept_private".to_string(),
            memory_id: Some("memory_relationship".to_string()),
            status: RelationshipStatus::Active,
        })
        .expect("relationship");

    let mut request = request_for_kevin();
    request
        .trusted
        .active_scopes
        .push("project_noema".to_string());
    request.trusted.sensitivity_ceiling = Sensitivity::Private;

    let result = store.retrieve(&request);

    assert_eq!(included_ids(&result), vec!["memory_anchor"]);
    assert_eq!(
        result.agent_visible_omissions,
        vec![AgentVisibleOmission {
            reason: "policy_restricted_context"
        }]
    );
    assert_eq!(
        result.denied_for_audit[0].memory_id.as_deref(),
        Some("memory_relationship")
    );
    assert_eq!(result.denied_for_audit[0].relationship_id, None);
}

fn included_ids(result: &MemoryRetrievalResult) -> Vec<&str> {
    result
        .included
        .iter()
        .map(|memory| memory.memory_id.as_str())
        .collect()
}
