use serde_json::Value;

use crate::memory::Sensitivity;
use crate::store::{StoreError, ontology::EntityCandidate};

/// Claim lifecycle status stored in the embedded graph store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimStatus {
    /// Proposed claim not normally retrieved.
    Candidate,
    /// Current active claim.
    Active,
    /// Human or system-confirmed claim.
    Confirmed,
    /// Claim has unresolved contradictory evidence.
    Disputed,
    /// Claim was replaced by a newer claim.
    Superseded,
    /// Claim is retained but not active.
    Archived,
    /// Claim is deleted.
    Deleted,
}

/// Result category for a graph claim write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimWriteOutcome {
    /// A new claim row was inserted.
    Created,
    /// An existing claim row was reinforced with new evidence.
    Reinforced,
}

impl ClaimStatus {
    /// Canonical lowercase wire label for this claim status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Active => "active",
            Self::Confirmed => "confirmed",
            Self::Disputed => "disputed",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "active" => Ok(Self::Active),
            "confirmed" => Ok(Self::Confirmed),
            "disputed" => Ok(Self::Disputed),
            "superseded" => Ok(Self::Superseded),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(StoreError::InvalidEnum {
                kind: "claim status",
                value: value.to_string(),
            }),
        }
    }
}

/// Evidence authority vocabulary for claim support rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceAuthority {
    /// Human explicitly corrected a prior claim.
    HumanCorrection,
    /// Human explicitly stated the claim.
    ExplicitHumanStatement,
    /// Claim came from a document source.
    DocumentSource,
    /// Claim was observed repeatedly.
    RepeatedObservation,
    /// Claim was inferred by an agent.
    AgentInference,
    /// Claim was weakly inferred by an agent.
    WeakInference,
    /// Claim follows from a system rule.
    SystemRule,
}

impl EvidenceAuthority {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::HumanCorrection => "human_correction",
            Self::ExplicitHumanStatement => "explicit_human_statement",
            Self::DocumentSource => "document_source",
            Self::RepeatedObservation => "repeated_observation",
            Self::AgentInference => "agent_inference",
            Self::WeakInference => "weak_inference",
            Self::SystemRule => "system_rule",
        }
    }
}

/// Candidate evidence row supporting a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceCandidate {
    /// Source conversation item id.
    pub source_item_id: String,
    /// Authority class for this evidence.
    pub authority: EvidenceAuthority,
    /// Optional short source excerpt.
    pub excerpt: Option<String>,
}

/// Candidate claim plus one supporting evidence row.
#[derive(Debug, Clone, PartialEq)]
pub struct NewClaimCandidate {
    /// Subject entity to upsert.
    pub subject: EntityCandidate,
    /// Object entity to upsert.
    pub object: EntityCandidate,
    /// Existing predicate id.
    pub predicate_id: String,
    /// Human-readable fact text.
    pub fact: String,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Claim lifecycle status.
    pub status: ClaimStatus,
    /// Optional confidence in `[0, 1]`.
    pub confidence: Option<f64>,
    /// Supporting source evidence.
    pub evidence: EvidenceCandidate,
    /// Retrieval hints stored with the claim.
    pub retrieval_hints: Value,
    /// Caller metadata stored with the claim.
    pub metadata: Value,
}

/// Typed summary returned after creating or reinforcing a claim.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimSummary {
    /// Stable claim id.
    pub claim_id: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Object entity id.
    pub object_entity_id: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Fact text.
    pub fact: String,
    /// Claim status.
    pub status: ClaimStatus,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Count of support evidence rows.
    pub evidence_count: i64,
    /// Whether this write inserted a claim or reinforced an existing one.
    pub write_outcome: ClaimWriteOutcome,
}

/// Candidate describing a replacement claim superseding an existing claim.
#[derive(Debug, Clone, PartialEq)]
pub struct SupersedeClaimCandidate {
    /// Replacement claim candidate to create or reinforce.
    pub replacement: NewClaimCandidate,
    /// Existing claim id that is being superseded.
    pub superseded_claim_id: String,
    /// Optional machine-readable metadata for the supersession edge.
    pub metadata: Value,
}

/// Read-only filters for memory-management claim inspection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryClaimFilter {
    /// Optional text query matched against facts, predicate labels, and entity names.
    pub query: Option<String>,
    /// Optional claim lifecycle status.
    pub status: Option<ClaimStatus>,
    /// Optional predicate id.
    pub predicate_id: Option<String>,
    /// Optional bounded result limit.
    pub limit: Option<usize>,
}

/// Read-only graph-claim projection for memory-management inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryClaimRecord {
    /// Stable claim id.
    pub claim_id: String,
    /// Human-readable fact text.
    pub fact: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Subject entity display name.
    pub subject_entity_name: String,
    /// Subject entity type.
    pub subject_entity_type: String,
    /// Object entity id when present.
    pub object_entity_id: Option<String>,
    /// Object entity display name when present.
    pub object_entity_name: Option<String>,
    /// Object entity type when present.
    pub object_entity_type: Option<String>,
    /// Claim status.
    pub status: ClaimStatus,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Read-only graph-claim detail projection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryClaimDetail {
    /// Claim projection.
    pub claim: MemoryClaimRecord,
    /// Supporting evidence rows.
    pub evidence: Vec<MemoryClaimEvidence>,
}

/// Read-only evidence projection for memory-management claim inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryClaimEvidence {
    /// Stable evidence relation id if available.
    pub evidence_id: Option<String>,
    /// Source conversation item id if this evidence came from a transcript item.
    pub source_item_id: Option<String>,
    /// Evidence authority string.
    pub authority: String,
    /// Optional excerpt.
    pub excerpt: Option<String>,
    /// Observation timestamp if available.
    pub observed_at: Option<String>,
    /// Evidence creation timestamp.
    pub created_at: String,
}

/// Request for bounded consolidation candidate matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidationMatchRequest {
    /// Candidate subject entity id.
    pub subject_entity_id: String,
    /// Candidate predicate id.
    pub predicate_id: String,
    /// Candidate object entity id if available.
    pub object_entity_id: Option<String>,
    /// Query terms used to post-filter fact text when object identity differs.
    pub query_terms: Vec<String>,
    /// Candidate sensitivity.
    pub sensitivity: Sensitivity,
    /// Requested result limit, clamped by the store.
    pub limit: usize,
}

/// Existing claim that may consolidate with a candidate memory write.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsolidationMatch {
    /// Stable claim id.
    pub claim_id: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Object entity id if present.
    pub object_entity_id: Option<String>,
    /// Predicate id.
    pub predicate_id: String,
    /// Canonical fact text.
    pub fact: String,
    /// Claim lifecycle status.
    pub status: ClaimStatus,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Claim confidence.
    pub confidence: Option<f64>,
}

/// Candidate relation linking two related claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedClaimCandidate {
    /// Primary claim id.
    pub claim_id: String,
    /// Related claim id.
    pub related_claim_id: String,
    /// Caller-defined relation kind.
    pub relation_kind: String,
    /// Human-readable rationale for the relation.
    pub rationale: String,
}

/// Stored relation linking two related claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedClaimRecord {
    /// Stable relation id.
    pub relation_id: String,
    /// Primary claim id.
    pub claim_id: String,
    /// Related claim id.
    pub related_claim_id: String,
    /// Caller-defined relation kind.
    pub relation_kind: String,
    /// Human-readable rationale for the relation.
    pub rationale: String,
    /// Creation timestamp.
    pub created_at: String,
}

/// Read-only filters for the bounded memory graph read model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryGraphFilter {
    /// Optional text query matched against predicate labels and public content.
    pub query: Option<String>,
    /// Optional claim lifecycle statuses. Defaults to candidate, active, and confirmed.
    pub statuses: Option<Vec<ClaimStatus>>,
    /// Optional predicate id.
    pub predicate_id: Option<String>,
    /// Optional exact sensitivity filter.
    pub sensitivity: Option<Sensitivity>,
    /// Optional bounded result limit.
    pub limit: Option<usize>,
}

/// Bounded graph-memory projection for memory-management inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryGraph {
    /// Entity nodes incident to the returned claim edges.
    pub nodes: Vec<MemoryGraphNode>,
    /// Claim edges connecting returned entity nodes.
    pub edges: Vec<MemoryGraphEdge>,
    /// Summary metadata for the bounded result.
    pub summary: MemoryGraphSummary,
}

/// Entity node in the memory graph read model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGraphNode {
    /// Stable graph node id derived from the entity id.
    pub node_id: String,
    /// Canonical entity id.
    pub entity_id: String,
    /// Human-readable entity label.
    pub label: String,
    /// Entity type string.
    pub entity_type: String,
    /// Strictest sensitivity among returned incident edges.
    pub max_sensitivity: Sensitivity,
    /// Count of returned incident claim edges.
    pub claim_count: i64,
}

/// Claim edge in the memory graph read model.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryGraphEdge {
    /// Stable claim id.
    pub claim_id: String,
    /// Source entity node id.
    pub source_node_id: String,
    /// Target entity node id.
    pub target_node_id: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Canonical fact text.
    pub fact: String,
    /// Claim status.
    pub status: ClaimStatus,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Summary metadata for a bounded memory graph result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryGraphSummary {
    /// Number of claim edges returned.
    pub returned_claim_count: i64,
    /// Number of entity nodes returned.
    pub returned_node_count: i64,
    /// Effective result limit after clamping.
    pub limit: usize,
    /// Whether the bounded candidate read window hit the effective limit.
    pub truncated: bool,
}
