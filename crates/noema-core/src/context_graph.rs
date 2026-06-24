//! Persisted context graph inspection view.
//!
//! This module is a read-side projection over canonical SQLite tables. It does
//! not own graph truth; memory, entity, provenance, participant, and
//! relationship claim tables remain the source of truth.

use crate::{
    context_graph_rows::{
        collect_sql_rows, row_to_graph_access_grant, row_to_graph_context_packet,
        row_to_graph_context_packet_memory_edge, row_to_graph_context_packet_omission,
        row_to_graph_entity_node, row_to_graph_memory_event, row_to_graph_memory_node,
        row_to_graph_memory_use_record, row_to_graph_object_link_edge,
        row_to_graph_participant_edge, row_to_graph_provenance_edge, row_to_graph_purpose_rule,
        row_to_graph_subject_edge, row_to_relationship_summary,
    },
    context_graph_sql::{
        GRAPH_ACCESS_GRANTS_SQL, GRAPH_CONTEXT_PACKET_MEMORY_EDGES_SQL,
        GRAPH_CONTEXT_PACKET_OMISSIONS_SQL, GRAPH_CONTEXT_PACKETS_SQL, GRAPH_ENTITY_NODES_SQL,
        GRAPH_MEMORY_EVENTS_SQL, GRAPH_MEMORY_NODES_SQL, GRAPH_MEMORY_USE_RECORDS_SQL,
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
};
use rusqlite::{Connection, OptionalExtension, params};

macro_rules! graph_params {
    ($limit:expr, $filter:expr) => {
        params![
            $limit,
            $filter.run_id.as_deref(),
            $filter.context_packet_id.as_deref()
        ]
    };
}

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

/// Optional target for context graph inspection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextGraphFilter {
    /// Limit graph inspection to packets for this run id.
    pub run_id: Option<String>,
    /// Limit graph inspection to a specific context packet id.
    pub context_packet_id: Option<String>,
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
    /// Context packet manifests.
    pub context_packets: Vec<GraphContextPacket>,
    /// Context packet memory inclusion edges.
    pub context_packet_memory_edges: Vec<GraphContextPacketMemoryEdge>,
    /// Context packet omission audit edges.
    pub context_packet_omissions: Vec<GraphContextPacketOmission>,
    /// Typed memory-use records by run and packet.
    pub memory_use_records: Vec<GraphMemoryUseRecord>,
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

/// Context packet manifest in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphContextPacket {
    /// Stable context packet id.
    pub context_packet_id: String,
    /// Run that produced the packet.
    pub run_id: String,
    /// Principal that requested the packet.
    pub requesting_principal_id: PrincipalId,
    /// Retrieval or execution purpose.
    pub purpose: Purpose,
    /// Active scopes as canonical JSON.
    pub active_scopes: String,
    /// Agent-visible redacted omissions as canonical JSON.
    pub agent_visible_omissions: String,
    /// SQLite-created timestamp.
    pub created_at: String,
}

/// Context packet to memory edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphContextPacketMemoryEdge {
    /// Stable packet-memory edge id.
    pub packet_memory_id: String,
    /// Context packet id.
    pub context_packet_id: String,
    /// Included memory id.
    pub memory_id: MemoryId,
    /// Sensitivity of the included memory.
    pub memory_sensitivity: Sensitivity,
    /// Packet stage represented by this edge.
    pub stage: String,
    /// Ranking score, if recorded.
    pub rank_score: Option<i64>,
    /// Eligibility reason, if recorded.
    pub eligibility_reason: Option<String>,
    /// Ranking reasons as canonical JSON.
    pub rank_reasons: String,
    /// SQLite-created timestamp.
    pub created_at: String,
}

/// Context packet omission edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphContextPacketOmission {
    /// Stable omission id.
    pub omission_id: String,
    /// Context packet id.
    pub context_packet_id: String,
    /// Omitted memory id, if known.
    pub memory_id: Option<MemoryId>,
    /// Omitted relationship id, if known.
    pub relationship_id: Option<String>,
    /// Record-level sensitivity for omission details.
    pub omission_sensitivity: Sensitivity,
    /// Redacted reason visible to an agent.
    pub agent_visible_reason: String,
    /// Audit-only precise denial reason.
    pub audit_reason: String,
    /// SQLite-created timestamp.
    pub created_at: String,
    /// Audit details as canonical JSON.
    pub details: String,
}

/// Typed memory-use record in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphMemoryUseRecord {
    /// Stable memory-use record id.
    pub memory_use_id: String,
    /// Context packet id, if associated.
    pub context_packet_id: Option<String>,
    /// Run id.
    pub run_id: String,
    /// Memory id.
    pub memory_id: MemoryId,
    /// Sensitivity of the memory.
    pub memory_sensitivity: Sensitivity,
    /// Use stage.
    pub stage: String,
    /// Agent principal, if recorded.
    pub agent_principal_id: Option<PrincipalId>,
    /// Scope id, if recorded.
    pub scope_id: Option<ScopeId>,
    /// Retrieval or execution purpose.
    pub purpose: Purpose,
    /// Object type affected by the use, if any.
    pub used_for_object_type: Option<String>,
    /// Object id affected by the use, if any.
    pub used_for_object_id: Option<String>,
    /// Policy decision id, if any.
    pub policy_decision_id: Option<String>,
    /// SQLite-created timestamp.
    pub created_at: String,
    /// Details as canonical JSON.
    pub details: String,
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
    filter: &ContextGraphFilter,
) -> Result<ContextGraphSummary, MemoryPersistenceError> {
    let memories = graph_memory_nodes(conn, limit, filter)?;
    let entities = graph_entity_nodes(conn, limit, filter)?;
    let subject_edges = graph_subject_edges(conn, limit, filter)?;
    let participant_edges = graph_participant_edges(conn, limit, filter)?;
    let provenance_edges = graph_provenance_edges(conn, limit, filter)?;
    let object_link_edges = graph_object_link_edges(conn, limit, filter)?;
    let purpose_rules = graph_purpose_rules(conn, limit, filter)?;
    let access_grants = graph_access_grants(conn, limit, filter)?;
    let context_packets = graph_context_packets(conn, limit, filter)?;
    let context_packet_memory_edges = graph_context_packet_memory_edges(conn, limit, filter)?;
    let context_packet_omissions = graph_context_packet_omissions(conn, limit, filter)?;
    let memory_use_records = graph_memory_use_records(conn, limit, filter)?;
    let memory_events = graph_memory_events(conn, limit, filter)?;
    let relationships = graph_relationship_edges(conn, limit, filter)?;
    Ok(ContextGraphSummary {
        memories,
        entities,
        subject_edges,
        participant_edges,
        provenance_edges,
        object_link_edges,
        purpose_rules,
        access_grants,
        context_packets,
        context_packet_memory_edges,
        context_packet_omissions,
        memory_use_records,
        memory_events,
        relationships,
    })
}

fn graph_memory_nodes(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphMemoryNode>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_MEMORY_NODES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), |row| {
            row_to_graph_memory_node(conn, row)
        })
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_entity_nodes(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphEntityNode>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_ENTITY_NODES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_entity_node)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_subject_edges(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphSubjectEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_SUBJECT_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_subject_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_participant_edges(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphParticipantEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_PARTICIPANT_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_participant_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_provenance_edges(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphProvenanceEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_PROVENANCE_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_provenance_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_object_link_edges(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphObjectLinkEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_OBJECT_LINK_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_object_link_edge)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_purpose_rules(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphPurposeRule>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_PURPOSE_RULES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_purpose_rule)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_access_grants(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphAccessGrant>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_ACCESS_GRANTS_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_access_grant)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_context_packets(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphContextPacket>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_CONTEXT_PACKETS_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_context_packet)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_context_packet_memory_edges(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphContextPacketMemoryEdge>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_CONTEXT_PACKET_MEMORY_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(
            graph_params!(limit, filter),
            row_to_graph_context_packet_memory_edge,
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_context_packet_omissions(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphContextPacketOmission>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_CONTEXT_PACKET_OMISSIONS_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(
            graph_params!(limit, filter),
            row_to_graph_context_packet_omission,
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_memory_use_records(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphMemoryUseRecord>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_MEMORY_USE_RECORDS_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_memory_use_record)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_memory_events(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<GraphMemoryEvent>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_MEMORY_EVENTS_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_graph_memory_event)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}

fn graph_relationship_edges(
    conn: &Connection,
    limit: u32,
    filter: &ContextGraphFilter,
) -> Result<Vec<RelationshipSummary>, MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(GRAPH_RELATIONSHIP_EDGES_SQL)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(graph_params!(limit, filter), row_to_relationship_summary)
        .map_err(MemoryPersistenceError::Sqlite)?;
    collect_sql_rows(rows)
}
