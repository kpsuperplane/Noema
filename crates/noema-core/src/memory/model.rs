use thiserror::Error;

/// Stable id for a graph entity.
pub type EntityId = String;
/// Stable id for a memory item.
pub type MemoryId = String;
/// Stable id for a human, agent, or other principal.
pub type PrincipalId = String;
/// Stable id for a graph relationship claim.
pub type RelationshipId = String;
/// Stable id for a memory-owning scope.
pub type ScopeId = String;

/// Lifecycle state for a memory item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryStatus {
    /// Proposed memory that is not normally retrieved.
    Candidate,
    /// Current memory.
    Active,
    /// Human or system-confirmed memory.
    Confirmed,
    /// Inferred memory eligible for use.
    Inferred,
    /// Memory known to be stale.
    Stale,
    /// Memory replaced by another memory.
    Superseded,
    /// Archived memory.
    Archived,
    /// Deleted memory.
    Deleted,
    /// Disputed memory.
    Disputed,
}

/// Sensitivity level used for retrieval gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sensitivity {
    /// Public memory can be found by public hints.
    Public,
    /// Normal memory can cross scopes through participant overlap.
    Normal,
    /// Private memory requires active scope or explicit grant.
    Private,
    /// Sensitive memory requires a deterministic unlock.
    Sensitive,
    /// Secret memory requires explicit request and approved access.
    Secret,
}

/// Validity state for typed retrieval policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetrievalPolicyStatus {
    /// Policy can authorize retrieval.
    Valid,
    /// Policy is stale and fails closed.
    Stale,
    /// Policy is invalid and fails closed.
    Invalid,
    /// Policy needs review and fails closed for private or stronger data.
    NeedsReview,
}

/// Policy for which participants can unlock a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticipantVisibilityPolicy {
    /// Any active original human participant can unlock the memory.
    AnyActiveHuman,
    /// All original human participants must be active.
    AllOriginalHumans,
    /// Only the owner can unlock the memory.
    OwnerOnly,
    /// Only an explicit grant can unlock the memory.
    ExplicitGrantOnly,
}

/// Policy for use outside Noema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalEgressPolicy {
    /// External use is allowed.
    Allow,
    /// External use requires approval.
    ApprovalRequired,
    /// External use is denied.
    Deny,
}

/// Purpose attached to a trusted retrieval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// Answer a human question.
    AnswerHumanQuestion,
    /// Draft content that stays inside Noema.
    DraftInternalContent,
    /// Personalize a response.
    GeneralPersonalization,
    /// Manage or reason about a task.
    ManageTask,
    /// Manage or reason about calendar state.
    ManageCalendar,
    /// Draft content intended to leave Noema.
    DraftExternalContent,
    /// Use a tool.
    UseTool,
    /// Produce a proactive suggestion.
    ProactiveSuggestion,
    /// Take or prepare an external action.
    ExternalAction,
    /// Inspect retrieval behavior for audit.
    DebugAudit,
}

/// Allow or deny effect for policy rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Allow the operation.
    Allow,
    /// Deny the operation.
    Deny,
}

/// Permission expressed by an access grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read the memory.
    Read,
    /// Use the memory during retrieval.
    UseForRetrieval,
    /// Use the memory for proactive behavior.
    UseForProactivity,
    /// Use the memory for external actions.
    UseForExternalAction,
}

/// Role a principal has with respect to a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticipantRole {
    /// Human participant in scope.
    HumanInScope,
    /// Agent participant in scope.
    AgentInScope,
    /// Principal that originated the memory.
    Originator,
    /// Principal that observed the memory.
    Observer,
}

/// Role an entity has in a memory or graph relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubjectRole {
    /// Entity the memory is about.
    About,
    /// Entity that made a claim.
    Claimant,
    /// Entity affected by the memory.
    Affected,
    /// Entity that owns the memory.
    Owner,
    /// Entity assigned to the memory.
    Assignee,
    /// Source entity in a relation.
    Source,
    /// Target entity in a relation.
    Target,
}

/// Lifecycle state for a graph relationship claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationshipStatus {
    /// Proposed relationship not yet usable for graph expansion.
    Candidate,
    /// Current relationship claim.
    Active,
    /// Confirmed relationship claim.
    Confirmed,
    /// Relationship replaced by another claim.
    Superseded,
    /// Archived relationship.
    Archived,
    /// Deleted relationship.
    Deleted,
    /// Disputed relationship.
    Disputed,
}

/// Non-authoritative metadata used only for ranking and public hint discovery.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetrievalHints {
    /// Topic labels used for fuzzy topic matches.
    pub topics: Vec<String>,
    /// Keywords matched against untrusted query text.
    pub keywords: Vec<String>,
    /// Short summary used for ranking.
    pub summary: Option<String>,
}

/// Canonical memory record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryItem {
    /// Stable memory id.
    pub memory_id: MemoryId,
    /// Scope that owns the memory.
    pub home_scope_id: ScopeId,
    /// Human-readable title.
    pub title: String,
    /// Memory content.
    pub content: String,
    /// Lifecycle state.
    pub status: MemoryStatus,
    /// Sensitivity gate.
    pub sensitivity: Sensitivity,
    /// Non-authoritative retrieval hints.
    pub retrieval_hints: RetrievalHints,
    /// Typed retrieval policy validity.
    pub retrieval_policy_status: RetrievalPolicyStatus,
    /// Participant-based visibility rule.
    pub participant_visibility_policy: ParticipantVisibilityPolicy,
    /// External egress policy.
    pub external_egress_policy: ExternalEgressPolicy,
    /// Optional owner principal.
    pub owner_principal_id: Option<PrincipalId>,
}

impl MemoryItem {
    /// Create an active normal memory with restrictive default policy.
    #[must_use]
    pub fn new(
        memory_id: impl Into<MemoryId>,
        home_scope_id: impl Into<ScopeId>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            memory_id: memory_id.into(),
            home_scope_id: home_scope_id.into(),
            title: title.into(),
            content: content.into(),
            status: MemoryStatus::Active,
            sensitivity: Sensitivity::Normal,
            retrieval_hints: RetrievalHints::default(),
            retrieval_policy_status: RetrievalPolicyStatus::NeedsReview,
            participant_visibility_policy: ParticipantVisibilityPolicy::ExplicitGrantOnly,
            external_egress_policy: ExternalEgressPolicy::ApprovalRequired,
            owner_principal_id: None,
        }
    }
}

/// Principal associated with a memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryParticipant {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Principal id.
    pub principal_id: PrincipalId,
    /// Participant role.
    pub role: ParticipantRole,
}

/// Entity associated with a memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySubject {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Entity id.
    pub entity_id: EntityId,
    /// Entity role.
    pub role: SubjectRole,
}

/// Purpose-specific retrieval policy for a memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeRule {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Purpose being governed.
    pub purpose: Purpose,
    /// Policy effect.
    pub effect: Effect,
}

/// Link from a memory to a trusted active object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectLink {
    /// Object type, such as `task` or `conversation`.
    pub object_type: String,
    /// Object id.
    pub object_id: String,
    /// Trusted relation between the active object and the memory.
    pub relation: Option<String>,
    /// Scope that authorized this active object link.
    pub authorized_scope_id: Option<ScopeId>,
}

impl ObjectLink {
    /// Create an object link.
    #[must_use]
    pub fn new(object_type: impl Into<String>, object_id: impl Into<String>) -> Self {
        Self {
            object_type: object_type.into(),
            object_id: object_id.into(),
            relation: None,
            authorized_scope_id: None,
        }
    }

    /// Attach a trusted relation to the object link.
    #[must_use]
    pub fn with_relation(mut self, relation: impl Into<String>) -> Self {
        self.relation = Some(relation.into());
        self
    }

    /// Attach the scope that authorized this object link.
    #[must_use]
    pub fn with_authorized_scope(mut self, scope_id: impl Into<ScopeId>) -> Self {
        self.authorized_scope_id = Some(scope_id.into());
        self
    }
}

/// Memory-to-object relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryObjectLink {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Linked object.
    pub object: ObjectLink,
}

/// Principal or scope grant over memory access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessGrant {
    /// Optional memory-specific grant target.
    pub memory_id: Option<MemoryId>,
    /// Optional scope-wide grant target.
    pub scope_id: Option<ScopeId>,
    /// Principal receiving the grant.
    pub principal_id: PrincipalId,
    /// Granted permission.
    pub permission: Permission,
    /// Grant effect.
    pub effect: Effect,
}

/// Graph relationship claim backed by a memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    /// Stable relationship id.
    pub relationship_id: RelationshipId,
    /// Scope that owns the relationship.
    pub home_scope_id: ScopeId,
    /// Subject entity id.
    pub subject_entity_id: EntityId,
    /// Predicate label.
    pub predicate: String,
    /// Object entity id.
    pub object_entity_id: EntityId,
    /// Supporting memory id.
    pub memory_id: Option<MemoryId>,
    /// Relationship lifecycle state.
    pub status: RelationshipStatus,
}

/// Provenance link proving where a memory came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceEdge {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Source object id.
    pub source_id: String,
}

/// Trusted retrieval fields supplied by Noema, not by untrusted prompt text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedRetrievalContext {
    /// Active human principals.
    pub active_human_ids: Vec<PrincipalId>,
    /// Active agent principals.
    pub active_agent_ids: Vec<PrincipalId>,
    /// Active scopes.
    pub active_scopes: Vec<ScopeId>,
    /// Retrieval purpose.
    pub purpose: Purpose,
    /// Whether the human explicitly asked to search memory.
    pub explicit_memory_request: bool,
    /// Whether secret access was approved.
    pub approved_secret_access: bool,
    /// Trusted active object links.
    pub active_object_links: Vec<ObjectLink>,
    /// Maximum sensitivity this request may include.
    pub sensitivity_ceiling: Sensitivity,
    /// Whether candidate memories may be included.
    pub include_candidate_memories: bool,
}

impl TrustedRetrievalContext {
    /// Build a basic context for a single active human.
    #[must_use]
    pub fn for_human(human_id: impl Into<PrincipalId>, purpose: Purpose) -> Self {
        Self {
            active_human_ids: vec![human_id.into()],
            active_agent_ids: Vec::new(),
            active_scopes: Vec::new(),
            purpose,
            explicit_memory_request: false,
            approved_secret_access: false,
            active_object_links: Vec::new(),
            sensitivity_ceiling: Sensitivity::Normal,
            include_candidate_memories: false,
        }
    }
}

/// Prompt-derived hints that cannot authorize private retrieval by themselves.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UntrustedHints {
    /// Raw query text.
    pub query_text: String,
    /// Fuzzy topic hints.
    pub fuzzy_topics: Vec<String>,
    /// Fuzzy entity hints.
    pub fuzzy_entities: Vec<String>,
}

/// Complete memory retrieval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRetrievalRequest {
    /// Principal requesting memory context.
    pub requesting_principal_id: PrincipalId,
    /// Trusted retrieval context.
    pub trusted: TrustedRetrievalContext,
    /// Untrusted prompt-derived hints.
    pub untrusted_hints: UntrustedHints,
}

/// Memory selected for inclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievedMemory {
    /// Included memory id.
    pub memory_id: MemoryId,
    /// Ranking score after policy eligibility is established.
    pub rank_score: u32,
    /// Why the memory was eligible.
    pub eligibility_reason: EligibilityReason,
    /// Why the memory ranked where it did.
    pub rank_reasons: Vec<RankReason>,
}

/// Eligibility source for a retrieved memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EligibilityReason {
    /// Memory belongs to an active scope.
    ActiveScope,
    /// Memory overlaps active human participants.
    ParticipantOverlap,
    /// Memory matched an explicit grant.
    ExplicitGrant,
    /// Memory matched a trusted active object link.
    TrustedObjectLink,
    /// Public memory matched untrusted hints.
    PublicHint,
    /// Memory was found through one-hop graph expansion.
    GraphExpansion,
}

/// Ranking signal for an eligible memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankReason {
    /// Human explicitly requested memory.
    ExplicitMemoryRequest,
    /// Trusted object link matched.
    TrustedObjectLink,
    /// Public memory matched the durable hint/search index.
    PublicHint,
    /// Fuzzy topic matched.
    FuzzyTopic,
    /// Fuzzy keyword matched.
    FuzzyKeyword,
    /// Human participant overlapped.
    SameHumanParticipant,
    /// Graph expansion found the memory.
    GraphExpansion,
}

/// Audit-only reason a candidate was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenialReason {
    /// Memory is archived or deleted.
    ArchivedOrDeleted,
    /// Candidate memories are excluded for this request.
    CandidateExcluded,
    /// Memory is disputed or stale.
    DisputedOrStale,
    /// Explicit deny grant matched.
    ExplicitDenyGrant,
    /// Memory is outside the deterministic search aperture.
    OutsideSearchAperture,
    /// Memory exceeds the request sensitivity ceiling.
    SensitivityCeiling,
    /// Typed retrieval policy is not valid.
    RetrievalPolicyInvalid,
    /// Requested purpose is denied or not allowed.
    PurposeDenied,
    /// Participant visibility policy denied access.
    ParticipantVisibilityDenied,
    /// Sensitive memory lacked an explicit unlock.
    SensitiveUnlockMissing,
    /// Secret memory lacked approved access.
    SecretApprovalMissing,
    /// Relationship expansion lacked support or provenance.
    RelationshipUnsupported,
    /// External egress policy denied this purpose.
    ExternalEgressDenied,
    /// External egress requires an approval Noema has not modeled yet.
    ExternalEgressApprovalRequired,
}

/// Detailed denial record visible to audit, not directly to agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditDenial {
    /// Denied memory id, when applicable.
    pub memory_id: Option<MemoryId>,
    /// Denied relationship id, when applicable.
    pub relationship_id: Option<RelationshipId>,
    /// Denial reason.
    pub reason: DenialReason,
}

/// Redacted omission record visible to agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentVisibleOmission {
    /// Redacted omission reason.
    pub reason: &'static str,
}

/// Stage at which a memory was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryUseStage {
    /// Memory was retrieved by policy.
    Retrieved,
    /// Memory was included in the context packet.
    IncludedInPacket,
    /// Memory was shown to an agent executor.
    ShownToAgent,
    /// Memory influenced a human-visible reply.
    UsedInReply,
    /// Memory influenced a tool call or external effect.
    UsedForAction,
    /// Memory influenced a proactive suggestion or action.
    UsedForProactivity,
}

/// Audit record for memory use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryUseRecord {
    /// Memory id.
    pub memory_id: MemoryId,
    /// Use stage.
    pub stage: MemoryUseStage,
}

/// Result of memory retrieval.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryRetrievalResult {
    /// Memories included in context.
    pub included: Vec<RetrievedMemory>,
    /// Detailed audit-only denials.
    pub denied_for_audit: Vec<AuditDenial>,
    /// Redacted omissions visible to agents.
    pub agent_visible_omissions: Vec<AgentVisibleOmission>,
    /// Memory use audit records.
    pub use_records: Vec<MemoryUseRecord>,
}

/// Errors produced while mutating the memory store.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryStoreError {
    /// A referenced memory id does not exist.
    #[error("unknown memory id: {0}")]
    UnknownMemory(MemoryId),

    /// A non-candidate relationship did not include supporting memory.
    #[error("active or confirmed relationship {relationship_id} requires supporting memory")]
    RelationshipRequiresMemory {
        /// Relationship id.
        relationship_id: RelationshipId,
    },

    /// A relationship referenced a missing memory.
    #[error("relationship {relationship_id} references unknown supporting memory {memory_id}")]
    RelationshipReferencesUnknownMemory {
        /// Relationship id.
        relationship_id: RelationshipId,
        /// Missing memory id.
        memory_id: MemoryId,
    },

    /// A relationship's supporting memory lacks provenance.
    #[error("relationship {relationship_id} requires provenance on supporting memory {memory_id}")]
    RelationshipRequiresProvenance {
        /// Relationship id.
        relationship_id: RelationshipId,
        /// Supporting memory id.
        memory_id: MemoryId,
    },
}
