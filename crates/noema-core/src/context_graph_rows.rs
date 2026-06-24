//! Row decoding helpers for persisted context graph inspection.

use crate::{
    context_graph::{
        GraphAccessGrant, GraphEntityNode, GraphMemoryEvent, GraphMemoryNode, GraphObjectLinkEdge,
        GraphParticipantEdge, GraphProvenanceEdge, GraphPurposeRule, GraphSubjectEdge,
        RelationshipSummary,
    },
    memory::{
        Effect, ExternalEgressPolicy, MemoryId, MemoryStatus, ParticipantRole,
        ParticipantVisibilityPolicy, Purpose, RelationshipStatus, RetrievalPolicyStatus,
        Sensitivity, SubjectRole,
    },
    memory_persistence::{MemoryPersistenceError, MemoryType},
    retrieval_policy_fingerprint,
};
use rusqlite::Connection;

pub(crate) fn row_to_relationship_summary(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RelationshipSummary> {
    let status: String = row.get(8)?;
    Ok(RelationshipSummary {
        relationship_id: row.get(0)?,
        home_scope_id: row.get(1)?,
        subject_entity_id: row.get(2)?,
        subject_name: row.get(3)?,
        predicate: row.get(4)?,
        object_entity_id: row.get(5)?,
        object_name: row.get(6)?,
        memory_id: row.get(7)?,
        status: parse_relationship_status(&status).map_err(enum_to_sql_error)?,
        confidence: row.get(9)?,
        created_at: row.get(10)?,
    })
}

pub(crate) fn row_to_graph_memory_node(
    conn: &Connection,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphMemoryNode> {
    let memory_id: MemoryId = row.get(0)?;
    let status: String = row.get(1)?;
    let memory_type: String = row.get(2)?;
    let sensitivity: String = row.get(4)?;
    let retrieval_policy_status: String = row.get(7)?;
    let participant_visibility_policy: String = row.get(13)?;
    let external_egress_policy: String = row.get(14)?;
    let stored_policy_status =
        parse_retrieval_policy_status(&retrieval_policy_status).map_err(enum_to_sql_error)?;
    let retrieval_policy_fingerprint: Option<String> = row.get(9)?;
    let effective_policy_status = effective_retrieval_policy_status(
        conn,
        &memory_id,
        stored_policy_status,
        retrieval_policy_fingerprint.as_deref(),
    )
    .map_err(enum_to_sql_error)?;
    Ok(GraphMemoryNode {
        memory_id,
        status: parse_memory_status(&status).map_err(enum_to_sql_error)?,
        memory_type: parse_memory_type(&memory_type).map_err(enum_to_sql_error)?,
        home_scope_id: row.get(3)?,
        sensitivity: parse_sensitivity(&sensitivity).map_err(enum_to_sql_error)?,
        title: row.get(5)?,
        retrieval_hints: row.get(6)?,
        retrieval_policy_status: stored_policy_status,
        retrieval_policy_effective_status: effective_policy_status,
        retrieval_policy_version: row.get(8)?,
        retrieval_policy_fingerprint,
        retrieval_policy_extractor_principal_id: row.get(10)?,
        retrieval_policy_extractor_version: row.get(11)?,
        retrieval_policy_validated_at: row.get(12)?,
        participant_visibility_policy: parse_participant_visibility_policy(
            &participant_visibility_policy,
        )
        .map_err(enum_to_sql_error)?,
        external_egress_policy: parse_external_egress_policy(&external_egress_policy)
            .map_err(enum_to_sql_error)?,
        created_at: row.get(15)?,
    })
}

pub(crate) fn row_to_graph_entity_node(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphEntityNode> {
    Ok(GraphEntityNode {
        entity_id: row.get(0)?,
        entity_type: row.get(1)?,
        home_scope_id: row.get(2)?,
        canonical_name: row.get(3)?,
        linked_principal_id: row.get(4)?,
    })
}

pub(crate) fn row_to_graph_subject_edge(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphSubjectEdge> {
    let role: String = row.get(2)?;
    Ok(GraphSubjectEdge {
        memory_id: row.get(0)?,
        entity_id: row.get(1)?,
        role: parse_subject_role(&role).map_err(enum_to_sql_error)?,
    })
}

pub(crate) fn row_to_graph_participant_edge(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphParticipantEdge> {
    let role: String = row.get(2)?;
    Ok(GraphParticipantEdge {
        memory_id: row.get(0)?,
        principal_id: row.get(1)?,
        role: parse_participant_role(&role).map_err(enum_to_sql_error)?,
    })
}

pub(crate) fn row_to_graph_provenance_edge(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphProvenanceEdge> {
    Ok(GraphProvenanceEdge {
        memory_id: row.get(0)?,
        source_type: row.get(1)?,
        source_id: row.get(2)?,
        relation: row.get(3)?,
        evidence_excerpt: row.get(4)?,
    })
}

pub(crate) fn row_to_graph_object_link_edge(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphObjectLinkEdge> {
    Ok(GraphObjectLinkEdge {
        memory_id: row.get(0)?,
        object_type: row.get(1)?,
        object_id: row.get(2)?,
        relation: row.get(3)?,
        authorized_scope_id: row.get(4)?,
        resolver_principal_id: row.get(5)?,
        resolver_version: row.get(6)?,
        source_run_id: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub(crate) fn row_to_graph_purpose_rule(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphPurposeRule> {
    let purpose: String = row.get(1)?;
    let effect: String = row.get(2)?;
    Ok(GraphPurposeRule {
        memory_id: row.get(0)?,
        purpose: parse_purpose(&purpose).map_err(enum_to_sql_error)?,
        effect: parse_effect(&effect).map_err(enum_to_sql_error)?,
        created_by_principal_id: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub(crate) fn row_to_graph_access_grant(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphAccessGrant> {
    let effect: String = row.get(5)?;
    Ok(GraphAccessGrant {
        grant_id: row.get(0)?,
        memory_id: row.get(1)?,
        scope_id: row.get(2)?,
        principal_id: row.get(3)?,
        permission: row.get(4)?,
        effect: parse_effect(&effect).map_err(enum_to_sql_error)?,
        expires_at: row.get(6)?,
        created_by_principal_id: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub(crate) fn row_to_graph_memory_event(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphMemoryEvent> {
    let sensitivity: Option<String> = row.get(4)?;
    Ok(GraphMemoryEvent {
        event_id: row.get(0)?,
        event_type: row.get(1)?,
        actor_principal_id: row.get(2)?,
        memory_id: row.get(3)?,
        memory_sensitivity: sensitivity
            .as_deref()
            .map(parse_sensitivity)
            .transpose()
            .map_err(enum_to_sql_error)?,
        scope_id: row.get(5)?,
        reason: row.get(6)?,
        created_at: row.get(7)?,
        details: row.get(8)?,
    })
}

pub(crate) fn collect_sql_rows<T, F>(
    rows: rusqlite::MappedRows<'_, F>,
) -> Result<Vec<T>, MemoryPersistenceError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(MemoryPersistenceError::Sqlite)?);
    }
    Ok(values)
}

fn effective_retrieval_policy_status(
    conn: &Connection,
    memory_id: &str,
    stored_status: RetrievalPolicyStatus,
    stored_fingerprint: Option<&str>,
) -> Result<RetrievalPolicyStatus, MemoryPersistenceError> {
    if stored_status != RetrievalPolicyStatus::Valid {
        return Ok(stored_status);
    }

    let Some(stored_fingerprint) = stored_fingerprint else {
        return Ok(RetrievalPolicyStatus::Stale);
    };

    if retrieval_policy_fingerprint::current_fingerprint(conn, memory_id)? == stored_fingerprint {
        Ok(RetrievalPolicyStatus::Valid)
    } else {
        Ok(RetrievalPolicyStatus::Stale)
    }
}

fn enum_to_sql_error(error: MemoryPersistenceError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn parse_sensitivity(value: &str) -> Result<Sensitivity, MemoryPersistenceError> {
    match value {
        "public" => Ok(Sensitivity::Public),
        "normal" => Ok(Sensitivity::Normal),
        "private" => Ok(Sensitivity::Private),
        "sensitive" => Ok(Sensitivity::Sensitive),
        "secret" => Ok(Sensitivity::Secret),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "sensitivity",
            value: value.to_string(),
        }),
    }
}

fn parse_retrieval_policy_status(
    value: &str,
) -> Result<RetrievalPolicyStatus, MemoryPersistenceError> {
    match value {
        "valid" => Ok(RetrievalPolicyStatus::Valid),
        "stale" => Ok(RetrievalPolicyStatus::Stale),
        "invalid" => Ok(RetrievalPolicyStatus::Invalid),
        "needs_review" => Ok(RetrievalPolicyStatus::NeedsReview),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "retrieval policy status",
            value: value.to_string(),
        }),
    }
}

fn parse_participant_visibility_policy(
    value: &str,
) -> Result<ParticipantVisibilityPolicy, MemoryPersistenceError> {
    match value {
        "any_active_human" => Ok(ParticipantVisibilityPolicy::AnyActiveHuman),
        "all_original_humans" => Ok(ParticipantVisibilityPolicy::AllOriginalHumans),
        "owner_only" => Ok(ParticipantVisibilityPolicy::OwnerOnly),
        "explicit_grant_only" => Ok(ParticipantVisibilityPolicy::ExplicitGrantOnly),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "participant visibility policy",
            value: value.to_string(),
        }),
    }
}

fn parse_external_egress_policy(
    value: &str,
) -> Result<ExternalEgressPolicy, MemoryPersistenceError> {
    match value {
        "allow" => Ok(ExternalEgressPolicy::Allow),
        "approval_required" => Ok(ExternalEgressPolicy::ApprovalRequired),
        "deny" => Ok(ExternalEgressPolicy::Deny),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "external egress policy",
            value: value.to_string(),
        }),
    }
}

fn parse_memory_status(value: &str) -> Result<MemoryStatus, MemoryPersistenceError> {
    match value {
        "candidate" => Ok(MemoryStatus::Candidate),
        "active" => Ok(MemoryStatus::Active),
        "confirmed" => Ok(MemoryStatus::Confirmed),
        "inferred" => Ok(MemoryStatus::Inferred),
        "stale" => Ok(MemoryStatus::Stale),
        "superseded" => Ok(MemoryStatus::Superseded),
        "archived" => Ok(MemoryStatus::Archived),
        "deleted" => Ok(MemoryStatus::Deleted),
        "disputed" => Ok(MemoryStatus::Disputed),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "memory status",
            value: value.to_string(),
        }),
    }
}

fn parse_memory_type(value: &str) -> Result<MemoryType, MemoryPersistenceError> {
    match value {
        "fact" => Ok(MemoryType::Fact),
        "preference" => Ok(MemoryType::Preference),
        "person" => Ok(MemoryType::Person),
        "organization" => Ok(MemoryType::Organization),
        "project" => Ok(MemoryType::Project),
        "place" => Ok(MemoryType::Place),
        "routine" => Ok(MemoryType::Routine),
        "goal" => Ok(MemoryType::Goal),
        "open_loop" => Ok(MemoryType::OpenLoop),
        "procedure" => Ok(MemoryType::Procedure),
        "constraint" => Ok(MemoryType::Constraint),
        "trigger" => Ok(MemoryType::Trigger),
        "decision" => Ok(MemoryType::Decision),
        "skill" => Ok(MemoryType::Skill),
        "policy" => Ok(MemoryType::Policy),
        "note" => Ok(MemoryType::Note),
        "other" => Ok(MemoryType::Other),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "memory type",
            value: value.to_string(),
        }),
    }
}

fn parse_purpose(value: &str) -> Result<Purpose, MemoryPersistenceError> {
    match value {
        "answer_human_question" => Ok(Purpose::AnswerHumanQuestion),
        "draft_internal_content" => Ok(Purpose::DraftInternalContent),
        "general_personalization" => Ok(Purpose::GeneralPersonalization),
        "manage_task" => Ok(Purpose::ManageTask),
        "manage_calendar" => Ok(Purpose::ManageCalendar),
        "draft_external_content" => Ok(Purpose::DraftExternalContent),
        "use_tool" => Ok(Purpose::UseTool),
        "proactive_suggestion" => Ok(Purpose::ProactiveSuggestion),
        "external_action" => Ok(Purpose::ExternalAction),
        "debug_audit" => Ok(Purpose::DebugAudit),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "retrieval purpose",
            value: value.to_string(),
        }),
    }
}

fn parse_effect(value: &str) -> Result<Effect, MemoryPersistenceError> {
    match value {
        "allow" => Ok(Effect::Allow),
        "deny" => Ok(Effect::Deny),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "policy effect",
            value: value.to_string(),
        }),
    }
}

fn parse_relationship_status(value: &str) -> Result<RelationshipStatus, MemoryPersistenceError> {
    match value {
        "candidate" => Ok(RelationshipStatus::Candidate),
        "active" => Ok(RelationshipStatus::Active),
        "confirmed" => Ok(RelationshipStatus::Confirmed),
        "superseded" => Ok(RelationshipStatus::Superseded),
        "archived" => Ok(RelationshipStatus::Archived),
        "deleted" => Ok(RelationshipStatus::Deleted),
        "disputed" => Ok(RelationshipStatus::Disputed),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "relationship status",
            value: value.to_string(),
        }),
    }
}

fn parse_participant_role(value: &str) -> Result<ParticipantRole, MemoryPersistenceError> {
    match value {
        "human_in_scope" => Ok(ParticipantRole::HumanInScope),
        "agent_in_scope" => Ok(ParticipantRole::AgentInScope),
        "originator" => Ok(ParticipantRole::Originator),
        "observer" => Ok(ParticipantRole::Observer),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "participant role",
            value: value.to_string(),
        }),
    }
}

fn parse_subject_role(value: &str) -> Result<SubjectRole, MemoryPersistenceError> {
    match value {
        "about" => Ok(SubjectRole::About),
        "claimant" => Ok(SubjectRole::Claimant),
        "affected" => Ok(SubjectRole::Affected),
        "owner" => Ok(SubjectRole::Owner),
        "assignee" => Ok(SubjectRole::Assignee),
        "source" => Ok(SubjectRole::Source),
        "target" => Ok(SubjectRole::Target),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "subject role",
            value: value.to_string(),
        }),
    }
}
