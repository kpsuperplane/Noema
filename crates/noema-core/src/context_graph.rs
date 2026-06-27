//! Persisted context graph inspection view.
//!
//! This module defines the read-side context graph projection shape. It does
//! not own graph truth; memory, entity, provenance, participant, and
//! relationship claim tables remain the source of truth.

use crate::{
    memory::{
        Effect, ExternalEgressPolicy, MemoryId, MemoryStatus, ParticipantRole,
        ParticipantVisibilityPolicy, Purpose, RelationshipStatus, RetrievalPolicyStatus,
        Sensitivity, SubjectRole,
    },
    memory_persistence::MemoryType,
};

/// Persisted relationship claim edge for graph inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipSummary {
    /// Stable relationship id.
    pub relationship_id: String,
    /// Concrete owner object type.
    pub owner_object_type: String,
    /// Concrete owner object id.
    pub owner_object_id: String,
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
    /// Storage-created timestamp.
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
    /// Memory-to-object participant edges.
    pub participant_edges: Vec<GraphParticipantEdge>,
    /// Memory-to-source provenance edges.
    pub provenance_edges: Vec<GraphProvenanceEdge>,
    /// Memory-to-trusted-object retrieval policy edges.
    pub object_link_edges: Vec<GraphObjectLinkEdge>,
    /// Memory retrieval purpose rules.
    pub purpose_rules: Vec<GraphPurposeRule>,
    /// Object access grants.
    pub access_grants: Vec<GraphAccessGrant>,
    /// Context packet manifests.
    pub context_packets: Vec<GraphContextPacket>,
    /// Context packet memory inclusion edges.
    pub context_packet_memory_edges: Vec<GraphContextPacketMemoryEdge>,
    /// Context packet omission audit edges.
    pub context_packet_omissions: Vec<GraphContextPacketOmission>,
    /// Typed memory-use records by run and packet.
    pub memory_use_records: Vec<GraphMemoryUseRecord>,
    /// Object lifecycle and use events.
    pub object_events: Vec<GraphMemoryEvent>,
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
    /// Concrete owner object type.
    pub owner_object_type: String,
    /// Concrete owner object id.
    pub owner_object_id: String,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
    /// Display title.
    pub title: String,
    /// Durable memory content.
    pub content: String,
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
    /// Actor id that extracted or validated retrieval policy.
    pub retrieval_policy_extractor_actor_id: Option<String>,
    /// Extractor implementation version.
    pub retrieval_policy_extractor_version: Option<String>,
    /// Timestamp when retrieval policy was validated.
    pub retrieval_policy_validated_at: Option<String>,
    /// Participant visibility rule.
    pub participant_visibility_policy: ParticipantVisibilityPolicy,
    /// External egress policy.
    pub external_egress_policy: ExternalEgressPolicy,
    /// Storage-created timestamp.
    pub created_at: String,
}

/// Entity node in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEntityNode {
    /// Entity id.
    pub entity_id: String,
    /// Entity type.
    pub entity_type: String,
    /// Concrete owner object type.
    pub owner_object_type: Option<String>,
    /// Concrete owner object id.
    pub owner_object_id: Option<String>,
    /// Canonical display name.
    pub canonical_name: String,
    /// Linked object type, when this entity represents another object.
    pub linked_object_type: Option<String>,
    /// Linked object id, when this entity represents another object.
    pub linked_object_id: Option<String>,
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

/// Memory-to-object participant edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphParticipantEdge {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Participant actor id.
    pub participant_actor_id: String,
    /// Participant role.
    pub role: ParticipantRole,
}

/// Memory-to-source provenance edge in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphProvenanceEdge {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Source object type.
    pub source_object_type: String,
    /// Source object id.
    pub source_object_id: String,
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
    /// Actor id that authorized this object link, when one is required.
    pub authorized_actor_id: Option<String>,
    /// Actor id that resolved the object link, if recorded.
    pub resolver_actor_id: Option<String>,
    /// Resolver implementation version, if recorded.
    pub resolver_version: Option<String>,
    /// Run that produced the object link, if recorded.
    pub source_run_id: Option<String>,
    /// Storage-created timestamp.
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
    /// Actor id that created the rule, if recorded.
    pub created_by_actor_id: Option<String>,
    /// Storage-created timestamp.
    pub created_at: String,
}

/// Object access grant in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphAccessGrant {
    /// Stable grant id.
    pub grant_id: String,
    /// Target object type.
    pub target_object_type: String,
    /// Target object id.
    pub target_object_id: String,
    /// Grantee object type.
    pub grantee_object_type: String,
    /// Grantee object id.
    pub grantee_object_id: String,
    /// Permission string from the canonical grant table.
    pub permission: String,
    /// Allow or deny effect.
    pub effect: Effect,
    /// Expiration timestamp, if any.
    pub expires_at: Option<String>,
    /// Actor id that created the grant, if recorded.
    pub created_by_actor_id: Option<String>,
    /// Storage-created timestamp.
    pub created_at: String,
}

/// Context packet manifest in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphContextPacket {
    /// Stable context packet id.
    pub context_packet_id: String,
    /// Run that produced the packet.
    pub run_id: String,
    /// Actor id that requested the packet.
    pub requesting_actor_id: String,
    /// Retrieval or execution purpose.
    pub purpose: Purpose,
    /// Active objects as canonical JSON.
    pub active_objects: String,
    /// Agent-visible redacted omissions as canonical JSON.
    pub agent_visible_omissions: String,
    /// Storage-created timestamp.
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
    /// Storage-created timestamp.
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
    /// Storage-created timestamp.
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
    /// Agent actor id, if recorded.
    pub agent_actor_id: Option<String>,
    /// Context object type, if recorded.
    pub context_object_type: Option<String>,
    /// Context object id, if recorded.
    pub context_object_id: Option<String>,
    /// Retrieval or execution purpose.
    pub purpose: Purpose,
    /// Object type affected by the use, if any.
    pub used_for_object_type: Option<String>,
    /// Object id affected by the use, if any.
    pub used_for_object_id: Option<String>,
    /// Policy decision id, if any.
    pub policy_decision_id: Option<String>,
    /// Storage-created timestamp.
    pub created_at: String,
    /// Details as canonical JSON.
    pub details: String,
}

/// Object lifecycle or use event in a graph inspection view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphMemoryEvent {
    /// Stable event id.
    pub event_id: String,
    /// Canonical memory event type.
    pub event_type: String,
    /// Actor id, if recorded.
    pub actor_id: Option<String>,
    /// Target object type, if recorded.
    pub target_object_type: Option<String>,
    /// Target object id, if recorded.
    pub target_object_id: Option<String>,
    /// Sensitivity of the target memory, if the event is memory-specific.
    pub target_memory_sensitivity: Option<Sensitivity>,
    /// Event reason, if recorded.
    pub reason: Option<String>,
    /// Storage-created timestamp.
    pub created_at: String,
    /// Event details as canonical JSON.
    pub details: String,
}
