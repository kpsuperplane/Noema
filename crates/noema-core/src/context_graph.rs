//! Persisted context graph inspection view.
//!
//! This module is a read-side projection over canonical SQLite tables. It does
//! not own graph truth; memory, entity, provenance, participant, and
//! relationship claim tables remain the source of truth.

use crate::{
    context_graph_sql::{
        GRAPH_ACCESS_GRANTS_SQL, GRAPH_ENTITY_NODES_SQL, GRAPH_MEMORY_NODES_SQL,
        GRAPH_OBJECT_LINK_EDGES_SQL, GRAPH_PARTICIPANT_EDGES_SQL, GRAPH_PROVENANCE_EDGES_SQL,
        GRAPH_PURPOSE_RULES_SQL, GRAPH_RELATIONSHIP_EDGES_SQL, GRAPH_SUBJECT_EDGES_SQL,
        RELATIONSHIP_BY_ID_SQL,
    },
    memory::{
        Effect, ExternalEgressPolicy, MemoryId, MemoryStatus, ParticipantRole,
        ParticipantVisibilityPolicy, PrincipalId, Purpose, RelationshipStatus,
        RetrievalPolicyStatus, ScopeId, Sensitivity, SubjectRole,
    },
    memory_persistence::{MemoryPersistenceError, MemoryType},
    retrieval_policy_fingerprint,
};
use rusqlite::{Connection, OptionalExtension, params};

/// Persisted relationship claim edge for graph inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipSummary {
    /// Stable relationship id.
    pub relationship_id: String,
    /// Scope that owns the relationship claim.
    pub home_scope_id: ScopeId,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Subject display name, if known.
    pub subject_name: Option<String>,
    /// Predicate label.
    pub predicate: String,
    /// Object entity id.
    pub object_entity_id: String,
    /// Object display name, if known.
    pub object_name: Option<String>,
    /// Supporting memory id, if any.
    pub memory_id: Option<MemoryId>,
    /// Relationship lifecycle status.
    pub status: RelationshipStatus,
    /// Optional confidence score.
    pub confidence: Option<f64>,
    /// SQLite-created timestamp.
    pub created_at: String,
}

/// Recent persisted context graph view for owner/admin inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextGraphSummary {
    /// Recent memory nodes.
    pub memories: Vec<GraphMemoryNode>,
    /// Entity nodes connected to recent memories or relationship claims.
    pub entities: Vec<GraphEntityNode>,
    /// Memory-to-entity subject edges.
    pub subject_edges: Vec<GraphSubjectEdge>,
    /// Memory-to-principal participant edges.
    pub participant_edges: Vec<GraphParticipantEdge>,
    /// Memory-to-source provenance edges.
    pub provenance_edges: Vec<GraphProvenanceEdge>,
    /// Memory-to-trusted-object retrieval policy edges.
    pub object_link_edges: Vec<GraphObjectLinkEdge>,
    /// Memory retrieval purpose rules.
    pub purpose_rules: Vec<GraphPurposeRule>,
    /// Memory or scope access grants.
    pub access_grants: Vec<GraphAccessGrant>,
    /// Relationship claim edges.
    pub relationships: Vec<RelationshipSummary>,
}

/// Memory node in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphMemoryNode {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Lifecycle status.
    pub status: MemoryStatus,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Scope that owns the memory.
    pub home_scope_id: ScopeId,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
    /// Display title.
    pub title: String,
    /// Non-authoritative retrieval hints as canonical JSON.
    pub retrieval_hints: String,
    /// Typed retrieval policy validity.
    pub retrieval_policy_status: RetrievalPolicyStatus,
    /// Policy status retrieval would use after fingerprint validation.
    pub retrieval_policy_effective_status: RetrievalPolicyStatus,
    /// Retrieval policy version.
    pub retrieval_policy_version: i64,
    /// Retrieval policy fingerprint, if validated.
    pub retrieval_policy_fingerprint: Option<String>,
    /// Principal that extracted or validated retrieval policy.
    pub retrieval_policy_extractor_principal_id: Option<PrincipalId>,
    /// Extractor implementation version.
    pub retrieval_policy_extractor_version: Option<String>,
    /// Timestamp when retrieval policy was validated.
    pub retrieval_policy_validated_at: Option<String>,
    /// Participant visibility rule.
    pub participant_visibility_policy: ParticipantVisibilityPolicy,
    /// External egress policy.
    pub external_egress_policy: ExternalEgressPolicy,
    /// SQLite-created timestamp.
    pub created_at: String,
}

/// Entity node in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEntityNode {
    /// Entity id.
    pub entity_id: String,
    /// Entity type.
    pub entity_type: String,
    /// Scope that owns or discovered the entity.
    pub home_scope_id: Option<ScopeId>,
    /// Canonical display name.
    pub canonical_name: String,
    /// Linked principal id, when this entity represents a principal.
    pub linked_principal_id: Option<PrincipalId>,
}

/// Memory-to-entity edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSubjectEdge {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Entity id.
    pub entity_id: String,
    /// Role the entity has in the memory.
    pub role: SubjectRole,
}

/// Memory-to-principal participant edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphParticipantEdge {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Principal id.
    pub principal_id: PrincipalId,
    /// Participant role.
    pub role: ParticipantRole,
}

/// Memory-to-source provenance edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphProvenanceEdge {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Source type.
    pub source_type: String,
    /// Source id.
    pub source_id: String,
    /// Provenance relation.
    pub relation: String,
    /// Supporting excerpt.
    pub evidence_excerpt: Option<String>,
}

/// Memory-to-trusted-object retrieval policy edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphObjectLinkEdge {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Trusted object type.
    pub object_type: String,
    /// Trusted object id.
    pub object_id: String,
    /// Relationship between the memory and the trusted object.
    pub relation: String,
    /// Scope that authorized this object link, when one is required.
    pub authorized_scope_id: Option<ScopeId>,
    /// Principal that resolved the object link, if recorded.
    pub resolver_principal_id: Option<PrincipalId>,
    /// Resolver implementation version, if recorded.
    pub resolver_version: Option<String>,
    /// Run that produced the object link, if recorded.
    pub source_run_id: Option<String>,
    /// SQLite-created timestamp.
    pub created_at: String,
}

/// Memory retrieval purpose allow/deny rule in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphPurposeRule {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Retrieval purpose being governed.
    pub purpose: Purpose,
    /// Allow or deny effect.
    pub effect: Effect,
    /// Principal that created the rule, if recorded.
    pub created_by_principal_id: Option<PrincipalId>,
    /// SQLite-created timestamp.
    pub created_at: String,
}

/// Memory or scope access grant in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphAccessGrant {
    /// Stable grant id.
    pub grant_id: String,
    /// Optional memory-specific grant target.
    pub memory_id: Option<MemoryId>,
    /// Optional scope-wide grant target.
    pub scope_id: Option<ScopeId>,
    /// Principal receiving the grant.
    pub principal_id: PrincipalId,
    /// Permission string from the canonical grant table.
    pub permission: String,
    /// Allow or deny effect.
    pub effect: Effect,
    /// Expiration timestamp, if any.
    pub expires_at: Option<String>,
    /// Principal that created the grant, if recorded.
    pub created_by_principal_id: Option<PrincipalId>,
    /// SQLite-created timestamp.
    pub created_at: String,
}

pub(crate) fn relationship_by_id(
    conn: &Connection,
    relationship_id: &str,
) -> Result<Option<RelationshipSummary>, MemoryPersistenceError> {
    conn.query_row(
        RELATIONSHIP_BY_ID_SQL,
        params![relationship_id],
        row_to_relationship_summary,
    )
    .optional()
    .map_err(MemoryPersistenceError::Sqlite)
}

pub(crate) fn inspect(
    conn: &Connection,
    limit: u32,
) -> Result<ContextGraphSummary, MemoryPersistenceError> {
    let memories = graph_memory_nodes(conn, limit)?;
    let entities = graph_entity_nodes(conn, limit)?;
    let subject_edges = graph_subject_edges(conn, limit)?;
    let participant_edges = graph_participant_edges(conn, limit)?;
    let provenance_edges = graph_provenance_edges(conn, limit)?;
    let object_link_edges = graph_object_link_edges(conn, limit)?;
    let purpose_rules = graph_purpose_rules(conn, limit)?;
    let access_grants = graph_access_grants(conn, limit)?;
    let relationships = graph_relationship_edges(conn, limit)?;
    Ok(ContextGraphSummary {
        memories,
        entities,
        subject_edges,
        participant_edges,
        provenance_edges,
        object_link_edges,
        purpose_rules,
        access_grants,
        relationships,
    })
}

fn graph_memory_nodes(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphMemoryNode>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_MEMORY_NODES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], |row| row_to_graph_memory_node(conn, row))
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_entity_nodes(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphEntityNode>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_ENTITY_NODES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_entity_node)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_subject_edges(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphSubjectEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_SUBJECT_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_subject_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_participant_edges(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphParticipantEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_PARTICIPANT_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_participant_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_provenance_edges(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphProvenanceEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_PROVENANCE_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_provenance_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_object_link_edges(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphObjectLinkEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_OBJECT_LINK_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_object_link_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_purpose_rules(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphPurposeRule>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_PURPOSE_RULES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_purpose_rule)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_access_grants(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphAccessGrant>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_ACCESS_GRANTS_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_access_grant)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_relationship_edges(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<RelationshipSummary>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_RELATIONSHIP_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_relationship_summary)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn row_to_relationship_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<RelationshipSummary> {
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

fn row_to_graph_memory_node(
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

fn row_to_graph_entity_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphEntityNode> {
    Ok(GraphEntityNode {
        entity_id: row.get(0)?,
        entity_type: row.get(1)?,
        home_scope_id: row.get(2)?,
        canonical_name: row.get(3)?,
        linked_principal_id: row.get(4)?,
    })
}

fn row_to_graph_subject_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphSubjectEdge> {
    let role: String = row.get(2)?;
    Ok(GraphSubjectEdge {
        memory_id: row.get(0)?,
        entity_id: row.get(1)?,
        role: parse_subject_role(&role).map_err(enum_to_sql_error)?,
    })
}

fn row_to_graph_participant_edge(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GraphParticipantEdge> {
    let role: String = row.get(2)?;
    Ok(GraphParticipantEdge {
        memory_id: row.get(0)?,
        principal_id: row.get(1)?,
        role: parse_participant_role(&role).map_err(enum_to_sql_error)?,
    })
}

fn row_to_graph_provenance_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphProvenanceEdge> {
    Ok(GraphProvenanceEdge {
        memory_id: row.get(0)?,
        source_type: row.get(1)?,
        source_id: row.get(2)?,
        relation: row.get(3)?,
        evidence_excerpt: row.get(4)?,
    })
}

fn row_to_graph_object_link_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphObjectLinkEdge> {
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

fn row_to_graph_purpose_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphPurposeRule> {
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

fn row_to_graph_access_grant(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphAccessGrant> {
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

fn collect_sql_rows<T, F>(
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
