//! Persisted context graph inspection view.
//!
//! This module is a read-side projection over canonical SQLite tables. It does
//! not own graph truth; memory, entity, provenance, participant, and
//! relationship claim tables remain the source of truth.

use crate::{
    memory::{
        MemoryId, MemoryStatus, ParticipantRole, PrincipalId, RelationshipStatus, ScopeId,
        Sensitivity, SubjectRole,
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
    let relationships = graph_relationship_edges(conn, limit)?;
    Ok(ContextGraphSummary {
        memories,
        entities,
        subject_edges,
        participant_edges,
        provenance_edges,
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
        .query_map(params![limit], row_to_graph_memory_node)
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

fn row_to_graph_memory_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphMemoryNode> {
    let status: String = row.get(1)?;
    let memory_type: String = row.get(2)?;
    let sensitivity: String = row.get(4)?;
    Ok(GraphMemoryNode {
        memory_id: row.get(0)?,
        status: parse_memory_status(&status).map_err(enum_to_sql_error)?,
        memory_type: parse_memory_type(&memory_type).map_err(enum_to_sql_error)?,
        home_scope_id: row.get(3)?,
        sensitivity: parse_sensitivity(&sensitivity).map_err(enum_to_sql_error)?,
        title: row.get(5)?,
        created_at: row.get(6)?,
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

const RELATIONSHIP_BY_ID_SQL: &str = r"
SELECT
  r.relationship_id,
  r.home_scope_id,
  r.subject_entity_id,
  subject.canonical_name,
  r.predicate,
  r.object_entity_id,
  object.canonical_name,
  r.memory_id,
  r.status,
  r.confidence,
  r.created_at
FROM relationships r
LEFT JOIN entities subject ON subject.entity_id = r.subject_entity_id
LEFT JOIN entities object ON object.entity_id = r.object_entity_id
WHERE r.relationship_id = ?1
";

const GRAPH_MEMORY_NODES_SQL: &str = r"
SELECT
  memory_id,
  status,
  memory_type,
  home_scope_id,
  sensitivity,
  title,
  created_at
FROM memory_items
ORDER BY created_at DESC, rowid DESC
LIMIT ?1
";

const GRAPH_ENTITY_NODES_SQL: &str = r"
SELECT DISTINCT
  e.entity_id,
  e.entity_type,
  e.home_scope_id,
  e.canonical_name,
  e.linked_principal_id
FROM entities e
WHERE e.entity_id IN (
  SELECT entity_id FROM memory_subjects
  UNION
  SELECT subject_entity_id FROM relationships
  UNION
  SELECT object_entity_id FROM relationships
)
ORDER BY e.created_at DESC, e.entity_id ASC
LIMIT ?1
";

const GRAPH_SUBJECT_EDGES_SQL: &str = r"
SELECT
  ms.memory_id,
  ms.entity_id,
  ms.role
FROM memory_subjects ms
JOIN memory_items mi ON mi.memory_id = ms.memory_id
ORDER BY mi.created_at DESC, ms.memory_id ASC, ms.entity_id ASC
LIMIT ?1
";

const GRAPH_PARTICIPANT_EDGES_SQL: &str = r"
SELECT
  mp.memory_id,
  mp.principal_id,
  mp.role
FROM memory_participants mp
JOIN memory_items mi ON mi.memory_id = mp.memory_id
ORDER BY mi.created_at DESC, mp.memory_id ASC, mp.principal_id ASC
LIMIT ?1
";

const GRAPH_PROVENANCE_EDGES_SQL: &str = r"
SELECT
  memory_id,
  source_type,
  source_id,
  relation,
  evidence_excerpt
FROM memory_provenance_edges
ORDER BY created_at DESC, edge_id ASC
LIMIT ?1
";

const GRAPH_RELATIONSHIP_EDGES_SQL: &str = r"
SELECT
  r.relationship_id,
  r.home_scope_id,
  r.subject_entity_id,
  subject.canonical_name,
  r.predicate,
  r.object_entity_id,
  object.canonical_name,
  r.memory_id,
  r.status,
  r.confidence,
  r.created_at
FROM relationships r
LEFT JOIN entities subject ON subject.entity_id = r.subject_entity_id
LEFT JOIN entities object ON object.entity_id = r.object_entity_id
ORDER BY r.created_at DESC, r.relationship_id ASC
LIMIT ?1
";
