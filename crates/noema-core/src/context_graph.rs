//! Persisted context graph inspection view.
//!
//! This module is a read-side projection over canonical SQLite tables. It does
//! not own graph truth; memory, entity, provenance, participant, and
//! relationship claim tables remain the source of truth.

use crate::{
    context_graph_rows::{
        collect_sql_rows, row_to_graph_access_grant, row_to_graph_entity_node,
        row_to_graph_memory_event, row_to_graph_memory_node, row_to_graph_object_link_edge,
        row_to_graph_participant_edge, row_to_graph_provenance_edge, row_to_graph_purpose_rule,
        row_to_graph_subject_edge, row_to_relationship_summary,
    },
    context_graph_sql::{
        GRAPH_ACCESS_GRANTS_SQL, GRAPH_ENTITY_NODES_SQL, GRAPH_MEMORY_EVENTS_SQL,
        GRAPH_MEMORY_NODES_SQL, GRAPH_OBJECT_LINK_EDGES_SQL, GRAPH_PARTICIPANT_EDGES_SQL,
        GRAPH_PROVENANCE_EDGES_SQL, GRAPH_PURPOSE_RULES_SQL, GRAPH_RELATIONSHIP_EDGES_SQL,
        GRAPH_SUBJECT_EDGES_SQL, RELATIONSHIP_BY_ID_SQL,
    },
    memory::{
        Effect, ExternalEgressPolicy, MemoryId, MemoryStatus, ParticipantRole,
        ParticipantVisibilityPolicy, PrincipalId, Purpose, RelationshipStatus,
        RetrievalPolicyStatus, ScopeId, Sensitivity, SubjectRole,
    },
    memory_persistence::{MemoryPersistenceError, MemoryType},
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
    /// Memory lifecycle and use events.
    pub memory_events: Vec<GraphMemoryEvent>,
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

/// Memory lifecycle or use event in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphMemoryEvent {
    /// Stable event id.
    pub event_id: String,
    /// Canonical memory event type.
    pub event_type: String,
    /// Actor principal, if recorded.
    pub actor_principal_id: Option<PrincipalId>,
    /// Memory affected by the event, if any.
    pub memory_id: Option<MemoryId>,
    /// Sensitivity of the memory, if the event is memory-specific.
    pub memory_sensitivity: Option<Sensitivity>,
    /// Scope affected by the event, if any.
    pub scope_id: Option<ScopeId>,
    /// Event reason, if recorded.
    pub reason: Option<String>,
    /// SQLite-created timestamp.
    pub created_at: String,
    /// Event details as canonical JSON.
    pub details: String,
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
    let memory_events = graph_memory_events(conn, limit)?;
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
        memory_events,
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

fn graph_memory_events(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<GraphMemoryEvent>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_MEMORY_EVENTS_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![limit], row_to_graph_memory_event)
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
