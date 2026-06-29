use serde_json::Value;
use surrealdb::types::{Datetime, SurrealValue};

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, record_fragment},
};

/// Closed graph entity type vocabulary stored in the embedded graph tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// A human owner or collaborator.
    Human,
    /// An agent principal.
    Agent,
    /// A person who is not necessarily a Noema human.
    Person,
    /// An organization.
    Organization,
    /// A project.
    Project,
    /// A workspace.
    Workspace,
    /// A conversation.
    Conversation,
    /// A document.
    Document,
    /// A tool.
    Tool,
    /// A place.
    Place,
    /// A task.
    Task,
    /// A goal.
    Goal,
    /// A concept.
    Concept,
    /// A fallback entity type.
    Other,
}

impl EntityType {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Conversation => "conversation",
            Self::Document => "document",
            Self::Tool => "tool",
            Self::Place => "place",
            Self::Task => "task",
            Self::Goal => "goal",
            Self::Concept => "concept",
            Self::Other => "other",
        }
    }
}

/// Candidate entity to upsert before creating or reinforcing a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCandidate {
    /// Stable graph entity id.
    pub entity_id: String,
    /// Closed entity type.
    pub entity_type: EntityType,
    /// Human-readable canonical name.
    pub canonical_name: String,
}

impl EntityCandidate {
    /// Return the local human entity used by the default single-user setup.
    #[must_use]
    pub fn local_human() -> Self {
        Self {
            entity_id: "human:local".to_string(),
            entity_type: EntityType::Human,
            canonical_name: "Local human".to_string(),
        }
    }

    /// Build a concept entity from a stable caller-provided id fragment.
    #[must_use]
    pub fn concept(id_fragment: &str, name: &str) -> Self {
        Self {
            entity_id: format!("concept:{}", super::ids::record_fragment(id_fragment)),
            entity_type: EntityType::Concept,
            canonical_name: name.to_string(),
        }
    }
}

/// Predicate proposal to persist for later review.
#[derive(Debug, Clone, PartialEq)]
pub struct PredicateProposalCandidate {
    /// Human-readable proposed predicate label.
    pub label: String,
    /// Human-readable proposed predicate description.
    pub description: String,
    /// Proposed predicate catalog fields.
    pub proposed_predicate: Value,
    /// Optional conversation item that produced the proposal.
    pub source_item_id: Option<String>,
    /// Claim candidate that could not be persisted without the proposed predicate.
    pub proposed_claim: Value,
}

/// Filters for predicate proposal inspection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PredicateProposalFilter {
    /// Optional proposal status filter.
    pub status: Option<String>,
    /// Maximum number of proposals to return.
    pub limit: Option<usize>,
}

/// Persisted predicate proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct PredicateProposalRecord {
    /// Stable proposal id.
    pub proposal_id: String,
    /// Human-readable proposed predicate label.
    pub label: String,
    /// Human-readable proposed predicate description.
    pub description: String,
    /// Proposed predicate catalog fields.
    pub proposed_predicate: Value,
    /// Claim candidate preserved alongside the proposed predicate.
    pub proposed_claim: Value,
    /// Proposal lifecycle status.
    pub status: String,
    /// Optional conversation item that produced the proposal.
    pub source_item_id: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, SurrealValue)]
struct PredicateProposalRow {
    proposal_id: String,
    label: String,
    description: String,
    proposed_predicate: Value,
    status: String,
    source_item_id: Option<String>,
    created_at: Datetime,
    updated_at: Datetime,
}

/// Persisted predicate fields needed by graph-memory write and canonicalization APIs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, SurrealValue)]
pub struct PredicateRecord {
    /// Stable predicate id.
    pub predicate_id: String,
    /// Human-readable label.
    pub label: String,
    /// Human-readable description.
    pub description: String,
    /// Entity types allowed as claim subjects.
    pub allowed_subject_types: Vec<String>,
    /// Entity types allowed as claim objects.
    pub allowed_object_types: Vec<String>,
    /// Default sensitivity stored as the schema vocabulary string.
    pub default_sensitivity: String,
    /// Use modes that may retrieve claims for this predicate.
    pub allowed_use_modes: Vec<String>,
    /// Conflict handling policy.
    pub conflict_policy: String,
    /// Review policy for incoming claims.
    pub review_policy: String,
    /// Inverse relation behavior.
    pub inverse_behavior: String,
    /// Optional inverse predicate id.
    pub inverse_predicate_id: Option<String>,
    /// Default proactivity level for memories using this predicate.
    pub proactivity_default: i64,
    /// Canonicalizer synonym hints.
    pub synonym_hints: Vec<String>,
}

impl NoemaStore {
    /// Return the promoted predicate catalog for canonicalization.
    pub async fn predicate_catalog(&self) -> Result<Vec<PredicateRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT predicate_id, label, description, allowed_subject_types,
                  allowed_object_types, allowed_use_modes, default_sensitivity,
                  conflict_policy, review_policy, inverse_behavior,
                  inverse_predicate_id, proactivity_default, synonym_hints
                FROM predicates
                ORDER BY predicate_id ASC;
                "#,
            )
            .await?;
        let rows: Vec<PredicateRecord> = response.take(0)?;
        Ok(rows)
    }

    /// Persist a candidate predicate proposal for later review.
    pub async fn create_predicate_proposal(
        &self,
        candidate: PredicateProposalCandidate,
    ) -> Result<PredicateProposalRecord, StoreError> {
        if let Some(source_item_id) = candidate.source_item_id.as_deref() {
            self.require_source_item(source_item_id).await?;
        }

        let proposal_id = allocate_id("predicate_proposal");
        let mut proposed_predicate = candidate.proposed_predicate;
        if let Some(object) = proposed_predicate.as_object_mut() {
            object.insert("proposed_claim".to_string(), candidate.proposed_claim);
        }

        self.db
            .query(
                r#"
                CREATE type::record('predicate_proposals', $record_id) SET
                  proposal_id = $proposal_id,
                  label = $label,
                  description = $description,
                  proposed_predicate = $proposed_predicate,
                  status = 'candidate',
                  source_item_id = $source_item_id,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&proposal_id)))
            .bind(("proposal_id", proposal_id.clone()))
            .bind(("label", candidate.label))
            .bind(("description", candidate.description))
            .bind(("proposed_predicate", proposed_predicate))
            .bind(("source_item_id", candidate.source_item_id))
            .await?
            .check()?;

        self.get_predicate_proposal(&proposal_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing predicate proposal after write: {proposal_id}"
                ))
            })
    }

    /// List predicate proposals, newest first.
    pub async fn list_predicate_proposals(
        &self,
        filter: PredicateProposalFilter,
    ) -> Result<Vec<PredicateProposalRecord>, StoreError> {
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let mut sql = String::from(
            r#"
            SELECT proposal_id, label, description, proposed_predicate, status,
              source_item_id, created_at, updated_at
            FROM predicate_proposals
            "#,
        );
        if filter.status.is_some() {
            sql.push_str("WHERE status = $status\n");
        }
        sql.push_str("ORDER BY created_at DESC, proposal_id ASC LIMIT $limit;");

        let mut statement = self.db.query(sql).bind(("limit", limit));
        if let Some(status) = filter.status {
            statement = statement.bind(("status", status));
        }
        let mut response = statement.await?;
        let rows: Vec<PredicateProposalRow> = response.take(0)?;
        rows.into_iter().map(predicate_proposal_record).collect()
    }

    /// Fetch one predicate proposal by stable id.
    pub async fn get_predicate_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<PredicateProposalRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT proposal_id, label, description, proposed_predicate, status,
                  source_item_id, created_at, updated_at
                FROM predicate_proposals
                WHERE proposal_id = $proposal_id
                LIMIT 1;
                "#,
            )
            .bind(("proposal_id", proposal_id.to_string()))
            .await?;
        let rows: Vec<PredicateProposalRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(predicate_proposal_record)
            .transpose()
    }
}

fn predicate_proposal_record(
    row: PredicateProposalRow,
) -> Result<PredicateProposalRecord, StoreError> {
    let proposed_claim = row
        .proposed_predicate
        .get("proposed_claim")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(PredicateProposalRecord {
        proposal_id: row.proposal_id,
        label: row.label,
        description: row.description,
        proposed_predicate: row.proposed_predicate,
        proposed_claim,
        status: row.status,
        source_item_id: row.source_item_id,
        created_at: super::claims::format_datetime(row.created_at),
        updated_at: super::claims::format_datetime(row.updated_at),
    })
}
