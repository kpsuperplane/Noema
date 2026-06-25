use crate::memory::{
    MemoryId, MemoryStatus, ParticipantRole, PrincipalId, RelationshipStatus, ScopeId, Sensitivity,
    SubjectRole,
};
use serde_json::{Value, json};

use super::helpers::chat_message_id;

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
    /// SQLite representation.
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

/// Source information for a memory candidate extracted from a chat turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMemorySource {
    /// Conversation id that produced the candidate.
    pub conversation_id: String,
    /// Message id that directly supports the candidate, if known.
    pub message_id: Option<String>,
    /// Short supporting excerpt to show during inspection, if available.
    pub evidence_excerpt: Option<String>,
}

/// Participant to attach to a new memory candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemoryParticipant {
    /// Principal id.
    pub principal_id: PrincipalId,
    /// Participant role.
    pub role: ParticipantRole,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewMemoryParticipant {
    /// Create a participant with empty metadata.
    #[must_use]
    pub fn new(principal_id: impl Into<PrincipalId>, role: ParticipantRole) -> Self {
        Self {
            principal_id: principal_id.into(),
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
    /// Entity type string from the canonical SQLite vocabulary.
    pub entity_type: String,
    /// Canonical entity display name.
    pub canonical_name: String,
    /// Role the entity has in the memory.
    pub role: SubjectRole,
    /// Alternate names for the entity.
    pub aliases: Vec<String>,
    /// Principal linked to the entity, if this entity represents one.
    pub linked_principal_id: Option<PrincipalId>,
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
            linked_principal_id: None,
            metadata: json!({}),
        }
    }
}

/// New chat turn to persist as message provenance for extracted memories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChatTurn {
    /// Conversation episode id and scope id.
    pub conversation_id: String,
    /// Zero- or one-based turn index assigned by the chat runtime.
    pub turn_index: u64,
    /// Stable user message id.
    pub user_message_id: String,
    /// Stable assistant message id.
    pub assistant_message_id: String,
    /// Principal id for the human/user side of the turn.
    pub user_principal_id: PrincipalId,
    /// Principal id for the assistant side of the turn.
    pub assistant_principal_id: PrincipalId,
    /// User message content.
    pub user_content: String,
    /// Assistant message content.
    pub assistant_content: String,
    /// Optional occurred-at timestamp in canonical text form.
    pub occurred_at: Option<String>,
    /// Additional structured metadata for both message rows.
    pub metadata: Value,
}

impl NewChatTurn {
    /// Create a chat turn with deterministic message ids.
    #[must_use]
    pub fn new(
        conversation_id: impl Into<String>,
        turn_index: u64,
        user_principal_id: impl Into<PrincipalId>,
        assistant_principal_id: impl Into<PrincipalId>,
        user_content: impl Into<String>,
        assistant_content: impl Into<String>,
    ) -> Self {
        let conversation_id = conversation_id.into();
        Self {
            user_message_id: chat_message_id(&conversation_id, "user", turn_index),
            assistant_message_id: chat_message_id(&conversation_id, "assistant", turn_index),
            conversation_id,
            turn_index,
            user_principal_id: user_principal_id.into(),
            assistant_principal_id: assistant_principal_id.into(),
            user_content: user_content.into(),
            assistant_content: assistant_content.into(),
            occurred_at: None,
            metadata: json!({}),
        }
    }
}

/// New memory candidate extracted from a chat turn.
#[derive(Debug, Clone, PartialEq)]
pub struct NewChatMemoryCandidate {
    /// Scope that owns the candidate.
    pub home_scope_id: ScopeId,
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
    /// Principal that created the candidate.
    pub created_by_principal_id: PrincipalId,
    /// Optional owner principal.
    pub owner_principal_id: Option<PrincipalId>,
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
    /// Optional chat provenance.
    pub source: Option<ChatMemorySource>,
    /// Participants in scope when the candidate was formed.
    pub participants: Vec<NewMemoryParticipant>,
    /// Subject entity bindings for the memory.
    pub subjects: Vec<NewMemorySubject>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewChatMemoryCandidate {
    /// Create a default chat memory candidate.
    #[must_use]
    pub fn new(
        home_scope_id: impl Into<ScopeId>,
        content: impl Into<String>,
        created_by_principal_id: impl Into<PrincipalId>,
    ) -> Self {
        Self {
            home_scope_id: home_scope_id.into(),
            memory_type: MemoryType::Note,
            title: None,
            content: content.into(),
            sensitivity: Sensitivity::Normal,
            status: MemoryStatus::Candidate,
            created_by_principal_id: created_by_principal_id.into(),
            owner_principal_id: None,
            authority_level: MemoryAuthorityLevel::AgentInference,
            extraction_method: MemoryExtractionMethod::LlmExtracted,
            confidence: None,
            retrieval_hints: json!({}),
            observed_at: None,
            source: None,
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
    /// Scope that owns the memory.
    pub home_scope_id: ScopeId,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
    /// Display title.
    pub title: String,
    /// Memory content.
    pub content: String,
    /// SQLite-created timestamp.
    pub created_at: String,
    /// Provenance source type, if available.
    pub source_type: Option<String>,
    /// Provenance source id, if available.
    pub source_id: Option<String>,
    /// Source conversation id, if available.
    pub conversation_id: Option<String>,
}

/// Relationship claim edge to insert into the persisted context graph.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRelationshipClaim {
    /// Optional stable relationship id. A `rel_` id is allocated when omitted.
    pub relationship_id: Option<String>,
    /// Scope that owns the relationship claim.
    pub home_scope_id: ScopeId,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Predicate label.
    pub predicate: String,
    /// Object entity id.
    pub object_entity_id: String,
    /// Supporting memory id, required for active or confirmed relationships.
    pub memory_id: Option<MemoryId>,
    /// Relationship lifecycle status.
    pub status: RelationshipStatus,
    /// Optional confidence score from extraction or curation.
    pub confidence: Option<f64>,
    /// Optional start of validity window.
    pub valid_from: Option<String>,
    /// Optional end of validity window.
    pub valid_to: Option<String>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewRelationshipClaim {
    /// Create a candidate relationship claim with no supporting memory yet.
    #[must_use]
    pub fn new(
        home_scope_id: impl Into<ScopeId>,
        subject_entity_id: impl Into<String>,
        predicate: impl Into<String>,
        object_entity_id: impl Into<String>,
    ) -> Self {
        Self {
            relationship_id: None,
            home_scope_id: home_scope_id.into(),
            subject_entity_id: subject_entity_id.into(),
            predicate: predicate.into(),
            object_entity_id: object_entity_id.into(),
            memory_id: None,
            status: RelationshipStatus::Candidate,
            confidence: None,
            valid_from: None,
            valid_to: None,
            metadata: json!({}),
        }
    }
}
