use crate::memory::{MemoryId, MemoryStatus, ParticipantRole, ScopeId, Sensitivity, SubjectRole};
use serde_json::{Value, json};

use super::objects::{ActorRef, ObjectRef};

/// Memory type stored in `memory_items.memory_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryType {
    /// Durable fact.
    Fact,
    /// User or scope preference.
    Preference,
    /// Person-related memory.
    Person,
    /// Organization-related memory.
    Organization,
    /// Project memory.
    Project,
    /// Place memory.
    Place,
    /// Routine or recurring behavior.
    Routine,
    /// Goal memory.
    Goal,
    /// Open loop or follow-up.
    OpenLoop,
    /// Procedure or workflow.
    Procedure,
    /// Constraint.
    Constraint,
    /// Trigger memory.
    Trigger,
    /// Decision.
    Decision,
    /// Agent skill memory.
    Skill,
    /// Policy memory.
    Policy,
    /// General note.
    Note,
    /// Other memory type.
    Other,
}

impl MemoryType {
    /// Storage representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Place => "place",
            Self::Routine => "routine",
            Self::Goal => "goal",
            Self::OpenLoop => "open_loop",
            Self::Procedure => "procedure",
            Self::Constraint => "constraint",
            Self::Trigger => "trigger",
            Self::Decision => "decision",
            Self::Skill => "skill",
            Self::Policy => "policy",
            Self::Note => "note",
            Self::Other => "other",
        }
    }
}

/// Authority level attached to a created memory candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryAuthorityLevel {
    /// Human corrected prior state.
    HumanCorrection,
    /// Explicit human statement.
    ExplicitHumanStatement,
    /// Workspace policy.
    WorkspacePolicy,
    /// Project decision.
    ProjectDecision,
    /// Document source.
    DocumentSource,
    /// Repeated observation.
    RepeatedObservation,
    /// Agent inference.
    AgentInference,
    /// Weak inference.
    WeakInference,
    /// System rule.
    SystemRule,
}

impl MemoryAuthorityLevel {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::HumanCorrection => "human_correction",
            Self::ExplicitHumanStatement => "explicit_human_statement",
            Self::WorkspacePolicy => "workspace_policy",
            Self::ProjectDecision => "project_decision",
            Self::DocumentSource => "document_source",
            Self::RepeatedObservation => "repeated_observation",
            Self::AgentInference => "agent_inference",
            Self::WeakInference => "weak_inference",
            Self::SystemRule => "system_rule",
        }
    }
}

/// Extraction method attached to a created memory candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryExtractionMethod {
    /// Direct explicit human instruction.
    ExplicitHuman,
    /// LLM-extracted memory candidate.
    LlmExtracted,
    /// Deterministic rule.
    DeterministicRule,
    /// Imported memory.
    Imported,
    /// Human-edited memory.
    HumanEdited,
    /// Agent summary.
    AgentSummary,
    /// System-generated memory.
    SystemGenerated,
}

impl MemoryExtractionMethod {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitHuman => "explicit_human",
            Self::LlmExtracted => "llm_extracted",
            Self::DeterministicRule => "deterministic_rule",
            Self::Imported => "imported",
            Self::HumanEdited => "human_edited",
            Self::AgentSummary => "agent_summary",
            Self::SystemGenerated => "system_generated",
        }
    }
}

/// Source information for a memory candidate backed by a concrete object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectProvenanceSource {
    /// Concrete source object that supports the candidate.
    pub source: ObjectRef,
    /// Short supporting excerpt to show during inspection, if available.
    pub evidence_excerpt: Option<String>,
}

/// Conversion helper accepted by [`NewMemoryParticipant::new`].
pub trait IntoMemoryParticipantRef {
    /// Convert to the typed actor reference stored for memory participants.
    fn into_memory_participant_ref(self) -> ActorRef;
}

impl IntoMemoryParticipantRef for ActorRef {
    fn into_memory_participant_ref(self) -> ActorRef {
        self
    }
}

impl IntoMemoryParticipantRef for &str {
    fn into_memory_participant_ref(self) -> ActorRef {
        inferred_actor_ref(self)
    }
}

impl IntoMemoryParticipantRef for String {
    fn into_memory_participant_ref(self) -> ActorRef {
        inferred_actor_ref(&self)
    }
}

impl IntoMemoryParticipantRef for &String {
    fn into_memory_participant_ref(self) -> ActorRef {
        inferred_actor_ref(self)
    }
}

fn inferred_actor_ref(object_id: &str) -> ActorRef {
    if object_id.starts_with("agent:") {
        ActorRef::agent(object_id)
    } else {
        ActorRef::human(object_id)
    }
}

/// Participant to attach to a new memory candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemoryParticipant {
    /// Participant actor.
    pub participant: ActorRef,
    /// Participant role.
    pub role: ParticipantRole,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewMemoryParticipant {
    /// Create a participant with empty metadata.
    #[must_use]
    pub fn new(participant: impl IntoMemoryParticipantRef, role: ParticipantRole) -> Self {
        Self {
            participant: participant.into_memory_participant_ref(),
            role,
            metadata: json!({}),
        }
    }
}

/// Subject entity to bind to a new memory candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemorySubject {
    /// Entity id.
    pub entity_id: String,
    /// Entity type string from the canonical storage vocabulary.
    pub entity_type: String,
    /// Canonical entity display name.
    pub canonical_name: String,
    /// Role the entity has in the memory.
    pub role: SubjectRole,
    /// Alternate names for the entity.
    pub aliases: Vec<String>,
    /// Concrete object linked to the entity, if this entity represents one.
    pub linked_object: Option<ObjectRef>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewMemorySubject {
    /// Create a subject entity binding with empty aliases and metadata.
    #[must_use]
    pub fn new(
        entity_id: impl Into<String>,
        entity_type: impl Into<String>,
        canonical_name: impl Into<String>,
        role: SubjectRole,
    ) -> Self {
        Self {
            entity_id: entity_id.into(),
            entity_type: entity_type.into(),
            canonical_name: canonical_name.into(),
            role,
            aliases: Vec::new(),
            linked_object: None,
            metadata: json!({}),
        }
    }
}

/// New memory candidate using concrete object ownership and provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMemoryCandidate {
    /// Concrete object that owns the memory.
    pub owner: ObjectRef,
    /// Type of memory.
    pub memory_type: MemoryType,
    /// Optional display title. A short title is derived from content when this
    /// is not supplied.
    pub title: Option<String>,
    /// Durable memory content.
    pub content: String,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
    /// Initial lifecycle status.
    pub status: MemoryStatus,
    /// Actor that created the candidate.
    pub created_by: ActorRef,
    /// Optional actor the memory is about or owned by in actor terms.
    pub owner_actor: Option<ActorRef>,
    /// Authority level behind the candidate.
    pub authority_level: MemoryAuthorityLevel,
    /// Extraction method.
    pub extraction_method: MemoryExtractionMethod,
    /// Optional confidence score from extraction.
    pub confidence: Option<f64>,
    /// Non-authoritative retrieval hints used for ranking.
    pub retrieval_hints: Value,
    /// Optional observed-at timestamp in canonical text form.
    pub observed_at: Option<String>,
    /// Optional concrete source provenance.
    pub source: Option<ObjectProvenanceSource>,
    /// Participants in scope when the candidate was formed.
    pub participants: Vec<NewMemoryParticipant>,
    /// Subject entity bindings for the memory.
    pub subjects: Vec<NewMemorySubject>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewMemoryCandidate {
    /// Create a confirmed note with one concrete source object.
    #[must_use]
    pub fn confirmed_note(
        owner: ObjectRef,
        content: impl Into<String>,
        created_by: ActorRef,
        source: ObjectRef,
    ) -> Self {
        Self {
            owner,
            memory_type: MemoryType::Note,
            title: None,
            content: content.into(),
            sensitivity: Sensitivity::Normal,
            status: MemoryStatus::Confirmed,
            created_by,
            owner_actor: None,
            authority_level: MemoryAuthorityLevel::AgentInference,
            extraction_method: MemoryExtractionMethod::LlmExtracted,
            confidence: None,
            retrieval_hints: json!({}),
            observed_at: None,
            source: Some(ObjectProvenanceSource {
                source,
                evidence_excerpt: None,
            }),
            participants: Vec::new(),
            subjects: Vec::new(),
            metadata: json!({}),
        }
    }
}

/// Recent memory row suitable for CLI inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySummary {
    /// Memory id.
    pub id: MemoryId,
    /// Lifecycle status.
    pub status: MemoryStatus,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Concrete owner object type.
    pub owner_object_type: String,
    /// Concrete owner object id.
    pub owner_object_id: String,
    /// Scope that owns the memory.
    pub home_scope_id: ScopeId,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
    /// Display title.
    pub title: String,
    /// Memory content.
    pub content: String,
    /// Subject entity ids bound to the memory.
    pub subject_entity_ids: Vec<String>,
    /// Canonical exact-dedupe fingerprint, if this row was created after the
    /// fingerprinting integration.
    pub dedupe_fingerprint: Option<String>,
    /// Storage-created timestamp.
    pub created_at: String,
    /// Concrete source object type, if available.
    pub source_object_type: Option<String>,
    /// Concrete source object id, if available.
    pub source_object_id: Option<String>,
    /// Provenance source type, if available.
    pub source_type: Option<String>,
    /// Provenance source id, if available.
    pub source_id: Option<String>,
    /// Source conversation id, if available.
    pub conversation_id: Option<String>,
}
