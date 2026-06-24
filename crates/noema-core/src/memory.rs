//! V1 memory storage and deterministic retrieval policy model.
//!
//! The store in this module is intentionally in-memory. It mirrors the V1
//! SQLite schema and policy rules so retrieval behavior can be developed and
//! tested before wiring persistent storage into the daemon.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};
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

/// In-memory implementation of the V1 retrieval policy model.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    memories: HashMap<MemoryId, MemoryItem>,
    participants: Vec<MemoryParticipant>,
    subjects: Vec<MemorySubject>,
    purpose_rules: Vec<PurposeRule>,
    object_links: Vec<MemoryObjectLink>,
    access_grants: Vec<AccessGrant>,
    relationships: HashMap<RelationshipId, Relationship>,
    provenance: Vec<ProvenanceEdge>,
}

impl MemoryStore {
    /// Insert or replace a memory.
    pub fn insert_memory(&mut self, memory: MemoryItem) {
        self.memories.insert(memory.memory_id.clone(), memory);
    }

    /// Add a participant to a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_participant(
        &mut self,
        memory_id: impl Into<MemoryId>,
        principal_id: impl Into<PrincipalId>,
        role: ParticipantRole,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.participants.push(MemoryParticipant {
            memory_id,
            principal_id: principal_id.into(),
            role,
        });
        Ok(())
    }

    /// Add a subject entity to a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_subject(
        &mut self,
        memory_id: impl Into<MemoryId>,
        entity_id: impl Into<EntityId>,
        role: SubjectRole,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.subjects.push(MemorySubject {
            memory_id,
            entity_id: entity_id.into(),
            role,
        });
        Ok(())
    }

    /// Add or replace a purpose-specific rule for a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_purpose_rule(
        &mut self,
        memory_id: impl Into<MemoryId>,
        purpose: Purpose,
        effect: Effect,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.purpose_rules
            .retain(|rule| rule.memory_id != memory_id || rule.purpose != purpose);
        self.purpose_rules.push(PurposeRule {
            memory_id,
            purpose,
            effect,
        });
        Ok(())
    }

    /// Link a memory to a trusted object.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_object_link(
        &mut self,
        memory_id: impl Into<MemoryId>,
        mut object: ObjectLink,
        relation: impl Into<String>,
        authorized_scope_id: Option<ScopeId>,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        object.relation = Some(relation.into());
        object.authorized_scope_id = authorized_scope_id;
        self.object_links
            .push(MemoryObjectLink { memory_id, object });
        Ok(())
    }

    /// Add an access grant.
    pub fn add_access_grant(&mut self, grant: AccessGrant) {
        self.access_grants.push(grant);
    }

    /// Add provenance for a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_provenance(
        &mut self,
        memory_id: impl Into<MemoryId>,
        source_id: impl Into<String>,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.provenance.push(ProvenanceEdge {
            memory_id,
            source_id: source_id.into(),
        });
        Ok(())
    }

    /// Add a graph relationship claim.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError`] when an active or confirmed relationship is
    /// missing supporting memory, references an unknown memory, or references a
    /// memory without provenance.
    pub fn add_relationship(&mut self, relationship: Relationship) -> Result<(), MemoryStoreError> {
        if relationship.status != RelationshipStatus::Candidate {
            let memory_id = relationship.memory_id.as_ref().ok_or_else(|| {
                MemoryStoreError::RelationshipRequiresMemory {
                    relationship_id: relationship.relationship_id.clone(),
                }
            })?;
            self.require_memory(memory_id).map_err(|_| {
                MemoryStoreError::RelationshipReferencesUnknownMemory {
                    relationship_id: relationship.relationship_id.clone(),
                    memory_id: memory_id.clone(),
                }
            })?;
            if !self.memory_has_provenance(memory_id) {
                return Err(MemoryStoreError::RelationshipRequiresProvenance {
                    relationship_id: relationship.relationship_id,
                    memory_id: memory_id.clone(),
                });
            }
        }

        self.relationships
            .insert(relationship.relationship_id.clone(), relationship);
        Ok(())
    }

    /// Retrieve memories using deterministic policy gates and ranking.
    #[must_use]
    pub fn retrieve(&self, request: &MemoryRetrievalRequest) -> MemoryRetrievalResult {
        let mut result = MemoryRetrievalResult::default();
        let mut candidates = HashMap::<MemoryId, CandidateSource>::new();

        for memory in self.memories.values() {
            if let Some(source) = self.direct_candidate_source(memory, request) {
                candidates.insert(memory.memory_id.clone(), source);
            }
        }

        let mut included_ids = HashSet::new();
        for (memory_id, source) in candidates {
            self.evaluate_and_push(memory_id, source, request, &mut included_ids, &mut result);
        }

        let start_entities = self.graph_start_entities(request, &included_ids);
        for relationship in self.relationships.values() {
            if !relationship_is_current(relationship.status) {
                continue;
            }

            let traversed = start_entities.contains(&relationship.subject_entity_id)
                || start_entities.contains(&relationship.object_entity_id);
            if !traversed {
                continue;
            }

            let Some(memory_id) = relationship.memory_id.clone() else {
                result.denied_for_audit.push(AuditDenial {
                    memory_id: None,
                    relationship_id: Some(relationship.relationship_id.clone()),
                    reason: DenialReason::RelationshipUnsupported,
                });
                continue;
            };

            if !self.memory_has_provenance(&memory_id) {
                result.denied_for_audit.push(AuditDenial {
                    memory_id: Some(memory_id),
                    relationship_id: Some(relationship.relationship_id.clone()),
                    reason: DenialReason::RelationshipUnsupported,
                });
                continue;
            }

            self.evaluate_and_push(
                memory_id,
                CandidateSource::GraphExpansion,
                request,
                &mut included_ids,
                &mut result,
            );
        }

        result
            .included
            .sort_by_key(|memory| Reverse(memory.rank_score));

        if !result.denied_for_audit.is_empty() {
            result.agent_visible_omissions.push(AgentVisibleOmission {
                reason: "policy_restricted_context",
            });
        }

        result
    }

    fn direct_candidate_source(
        &self,
        memory: &MemoryItem,
        request: &MemoryRetrievalRequest,
    ) -> Option<CandidateSource> {
        if Self::home_scope_active(memory, request) {
            return Some(CandidateSource::ActiveScope);
        }

        if self.has_explicit_allow(memory, request) {
            return Some(CandidateSource::ExplicitGrant);
        }

        if self.memory_object_link_matches(&memory.memory_id, request) {
            return Some(CandidateSource::TrustedObjectLink);
        }

        if self.same_human_participant(memory, request) {
            return Some(CandidateSource::ParticipantOverlap);
        }

        if memory.sensitivity == Sensitivity::Public
            && hint_score(memory, &request.untrusted_hints).score > 0
        {
            return Some(CandidateSource::PublicHint);
        }

        None
    }

    fn evaluate_and_push(
        &self,
        memory_id: MemoryId,
        source: CandidateSource,
        request: &MemoryRetrievalRequest,
        included_ids: &mut HashSet<MemoryId>,
        result: &mut MemoryRetrievalResult,
    ) {
        if included_ids.contains(&memory_id) {
            return;
        }

        match self.evaluate_memory(&memory_id, source, request) {
            Ok(retrieved) => {
                included_ids.insert(memory_id.clone());
                result.use_records.push(MemoryUseRecord {
                    memory_id,
                    stage: MemoryUseStage::Retrieved,
                });
                result.included.push(retrieved);
            }
            Err(reason) => result.denied_for_audit.push(AuditDenial {
                memory_id: Some(memory_id),
                relationship_id: None,
                reason,
            }),
        }
    }

    fn evaluate_memory(
        &self,
        memory_id: &MemoryId,
        source: CandidateSource,
        request: &MemoryRetrievalRequest,
    ) -> Result<RetrievedMemory, DenialReason> {
        let memory = self
            .memories
            .get(memory_id)
            .expect("candidate memory should exist");

        if self.has_explicit_deny(memory, request) {
            return Err(DenialReason::ExplicitDenyGrant);
        }

        if memory.sensitivity > request.trusted.sensitivity_ceiling {
            return Err(DenialReason::SensitivityCeiling);
        }

        if !Self::status_allows(memory.status, request) {
            return Err(match memory.status {
                MemoryStatus::Candidate => DenialReason::CandidateExcluded,
                MemoryStatus::Stale | MemoryStatus::Disputed => DenialReason::DisputedOrStale,
                _ => DenialReason::ArchivedOrDeleted,
            });
        }

        let purpose = self.purpose_effect(memory_id, request.trusted.purpose);
        if purpose == Some(Effect::Deny) {
            return Err(DenialReason::PurposeDenied);
        }
        Self::require_egress_policy(memory, request.trusted.purpose)?;

        match memory.sensitivity {
            Sensitivity::Public => {}
            Sensitivity::Normal => {
                if !self.aperture_matches(memory, request) {
                    return Err(DenialReason::OutsideSearchAperture);
                }
            }
            Sensitivity::Private => {
                if !Self::home_scope_active(memory, request)
                    && !self.has_explicit_allow(memory, request)
                {
                    return Err(DenialReason::OutsideSearchAperture);
                }
                Self::require_valid_policy(memory)?;
                self.require_allowed_purpose(memory_id, request)?;
                if !self.participant_visibility_allows(memory, request) {
                    return Err(DenialReason::ParticipantVisibilityDenied);
                }
            }
            Sensitivity::Sensitive => {
                if !self.aperture_matches(memory, request) {
                    return Err(DenialReason::OutsideSearchAperture);
                }
                Self::require_valid_policy(memory)?;
                self.require_allowed_purpose(memory_id, request)?;
                if !self.participant_visibility_allows(memory, request) {
                    return Err(DenialReason::ParticipantVisibilityDenied);
                }
                if !request.trusted.explicit_memory_request
                    && !self.memory_object_link_matches(memory_id, request)
                {
                    return Err(DenialReason::SensitiveUnlockMissing);
                }
            }
            Sensitivity::Secret => {
                Self::require_valid_policy(memory)?;
                self.require_allowed_purpose(memory_id, request)?;
                if !request.trusted.explicit_memory_request
                    || !request.trusted.approved_secret_access
                {
                    return Err(DenialReason::SecretApprovalMissing);
                }
                if !self.participant_visibility_allows(memory, request) {
                    return Err(DenialReason::ParticipantVisibilityDenied);
                }
            }
        }

        let ranked = self.rank_memory(memory, source, request);
        Ok(RetrievedMemory {
            memory_id: memory_id.clone(),
            rank_score: ranked.score,
            eligibility_reason: source.eligibility_reason(),
            rank_reasons: ranked.reasons,
        })
    }

    fn require_valid_policy(memory: &MemoryItem) -> Result<(), DenialReason> {
        if memory.retrieval_policy_status == RetrievalPolicyStatus::Valid {
            Ok(())
        } else {
            Err(DenialReason::RetrievalPolicyInvalid)
        }
    }

    fn require_allowed_purpose(
        &self,
        memory_id: &MemoryId,
        request: &MemoryRetrievalRequest,
    ) -> Result<(), DenialReason> {
        if self.purpose_effect(memory_id, request.trusted.purpose) == Some(Effect::Allow) {
            Ok(())
        } else {
            Err(DenialReason::PurposeDenied)
        }
    }

    fn require_egress_policy(memory: &MemoryItem, purpose: Purpose) -> Result<(), DenialReason> {
        if !matches!(
            purpose,
            Purpose::DraftExternalContent | Purpose::ExternalAction | Purpose::UseTool
        ) {
            return Ok(());
        }

        match memory.external_egress_policy {
            ExternalEgressPolicy::Allow => Ok(()),
            ExternalEgressPolicy::ApprovalRequired => {
                Err(DenialReason::ExternalEgressApprovalRequired)
            }
            ExternalEgressPolicy::Deny => Err(DenialReason::ExternalEgressDenied),
        }
    }

    fn rank_memory(
        &self,
        memory: &MemoryItem,
        source: CandidateSource,
        request: &MemoryRetrievalRequest,
    ) -> RankedMemory {
        let mut ranked = hint_score(memory, &request.untrusted_hints);

        match source {
            CandidateSource::ActiveScope => ranked.score += 80,
            CandidateSource::TrustedObjectLink => ranked.score += 70,
            CandidateSource::ExplicitGrant => ranked.score += 60,
            CandidateSource::ParticipantOverlap => ranked.score += 20,
            CandidateSource::GraphExpansion => ranked.score += 10,
            CandidateSource::PublicHint => {}
        }

        if request.trusted.explicit_memory_request {
            ranked.score += 100;
            ranked.reasons.push(RankReason::ExplicitMemoryRequest);
        }
        if self.memory_object_link_matches(&memory.memory_id, request) {
            ranked.score += 80;
            ranked.reasons.push(RankReason::TrustedObjectLink);
        }
        if self.same_human_participant(memory, request) {
            ranked.score += 20;
            ranked.reasons.push(RankReason::SameHumanParticipant);
        }
        if source == CandidateSource::GraphExpansion {
            ranked.score += 15;
            ranked.reasons.push(RankReason::GraphExpansion);
        }

        ranked
    }

    fn aperture_matches(&self, memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        Self::home_scope_active(memory, request)
            || self.same_human_participant(memory, request)
            || self.has_explicit_allow(memory, request)
            || self.memory_object_link_matches(&memory.memory_id, request)
    }

    fn home_scope_active(memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        request
            .trusted
            .active_scopes
            .iter()
            .any(|scope| scope == &memory.home_scope_id)
    }

    fn same_human_participant(
        &self,
        memory: &MemoryItem,
        request: &MemoryRetrievalRequest,
    ) -> bool {
        self.participants.iter().any(|participant| {
            participant.memory_id == memory.memory_id
                && participant.role == ParticipantRole::HumanInScope
                && request
                    .trusted
                    .active_human_ids
                    .contains(&participant.principal_id)
        })
    }

    fn participant_visibility_allows(
        &self,
        memory: &MemoryItem,
        request: &MemoryRetrievalRequest,
    ) -> bool {
        match memory.participant_visibility_policy {
            ParticipantVisibilityPolicy::AnyActiveHuman => {
                self.same_human_participant(memory, request)
            }
            ParticipantVisibilityPolicy::AllOriginalHumans => {
                let humans: Vec<_> = self
                    .participants
                    .iter()
                    .filter(|participant| {
                        participant.memory_id == memory.memory_id
                            && participant.role == ParticipantRole::HumanInScope
                    })
                    .collect();
                !humans.is_empty()
                    && humans.iter().all(|participant| {
                        request
                            .trusted
                            .active_human_ids
                            .contains(&participant.principal_id)
                    })
            }
            ParticipantVisibilityPolicy::OwnerOnly => memory
                .owner_principal_id
                .as_ref()
                .is_some_and(|owner| request.trusted.active_human_ids.contains(owner)),
            ParticipantVisibilityPolicy::ExplicitGrantOnly => {
                self.has_explicit_allow(memory, request)
            }
        }
    }

    fn has_explicit_allow(&self, memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        self.access_grants.iter().any(|grant| {
            grant.effect == Effect::Allow
                && grant.principal_id == request.requesting_principal_id
                && matches!(
                    grant.permission,
                    Permission::Read | Permission::UseForRetrieval
                )
                && grant_matches_memory(grant, memory)
        })
    }

    fn has_explicit_deny(&self, memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        self.access_grants.iter().any(|grant| {
            grant.effect == Effect::Deny
                && grant.principal_id == request.requesting_principal_id
                && matches!(
                    grant.permission,
                    Permission::Read | Permission::UseForRetrieval
                )
                && grant_matches_memory(grant, memory)
        })
    }

    fn memory_object_link_matches(
        &self,
        memory_id: &MemoryId,
        request: &MemoryRetrievalRequest,
    ) -> bool {
        self.object_links.iter().any(|link| {
            &link.memory_id == memory_id
                && request
                    .trusted
                    .active_object_links
                    .iter()
                    .any(|active| object_link_matches(&link.object, active, request))
        })
    }

    fn purpose_effect(&self, memory_id: &MemoryId, purpose: Purpose) -> Option<Effect> {
        self.purpose_rules
            .iter()
            .find(|rule| &rule.memory_id == memory_id && rule.purpose == purpose)
            .map(|rule| rule.effect)
    }

    fn status_allows(status: MemoryStatus, request: &MemoryRetrievalRequest) -> bool {
        match status {
            MemoryStatus::Active | MemoryStatus::Confirmed | MemoryStatus::Inferred => true,
            MemoryStatus::Candidate => request.trusted.include_candidate_memories,
            MemoryStatus::Stale
            | MemoryStatus::Superseded
            | MemoryStatus::Archived
            | MemoryStatus::Deleted
            | MemoryStatus::Disputed => false,
        }
    }

    fn graph_start_entities(
        &self,
        request: &MemoryRetrievalRequest,
        included_ids: &HashSet<MemoryId>,
    ) -> HashSet<EntityId> {
        let mut entities = HashSet::new();
        for link in &request.trusted.active_object_links {
            entities.insert(link.object_id.clone());
        }
        for subject in &self.subjects {
            if included_ids.contains(&subject.memory_id) {
                entities.insert(subject.entity_id.clone());
            }
        }
        entities
    }

    fn require_memory(&self, memory_id: &MemoryId) -> Result<(), MemoryStoreError> {
        if self.memories.contains_key(memory_id) {
            Ok(())
        } else {
            Err(MemoryStoreError::UnknownMemory(memory_id.clone()))
        }
    }

    fn memory_has_provenance(&self, memory_id: &MemoryId) -> bool {
        self.provenance
            .iter()
            .any(|provenance| &provenance.memory_id == memory_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateSource {
    ActiveScope,
    ParticipantOverlap,
    ExplicitGrant,
    TrustedObjectLink,
    PublicHint,
    GraphExpansion,
}

impl CandidateSource {
    fn eligibility_reason(self) -> EligibilityReason {
        match self {
            Self::ActiveScope => EligibilityReason::ActiveScope,
            Self::ParticipantOverlap => EligibilityReason::ParticipantOverlap,
            Self::ExplicitGrant => EligibilityReason::ExplicitGrant,
            Self::TrustedObjectLink => EligibilityReason::TrustedObjectLink,
            Self::PublicHint => EligibilityReason::PublicHint,
            Self::GraphExpansion => EligibilityReason::GraphExpansion,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct RankedMemory {
    score: u32,
    reasons: Vec<RankReason>,
}

fn hint_score(memory: &MemoryItem, hints: &UntrustedHints) -> RankedMemory {
    let mut ranked = RankedMemory::default();
    let query = normalize(&hints.query_text);
    let fuzzy_topics = normalized_set(&hints.fuzzy_topics);
    let fuzzy_entities = normalized_set(&hints.fuzzy_entities);

    for topic in &memory.retrieval_hints.topics {
        let topic = normalize(topic);
        if fuzzy_topics.contains(&topic) {
            ranked.score += 40;
            ranked.reasons.push(RankReason::FuzzyTopic);
        }
    }

    for keyword in &memory.retrieval_hints.keywords {
        let keyword = normalize(keyword);
        if !keyword.is_empty() && query.contains(&keyword) {
            ranked.score += 25;
            ranked.reasons.push(RankReason::FuzzyKeyword);
        }
    }

    if let Some(summary) = &memory.retrieval_hints.summary {
        let summary = normalize(summary);
        if fuzzy_entities
            .iter()
            .any(|entity| !entity.is_empty() && summary.contains(entity))
        {
            ranked.score += 10;
            ranked.reasons.push(RankReason::FuzzyKeyword);
        }
    }

    ranked
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalized_set(values: &[String]) -> HashSet<String> {
    values.iter().map(|value| normalize(value)).collect()
}

fn relationship_is_current(status: RelationshipStatus) -> bool {
    matches!(
        status,
        RelationshipStatus::Active | RelationshipStatus::Confirmed
    )
}

fn object_link_matches(
    policy: &ObjectLink,
    active: &ObjectLink,
    request: &MemoryRetrievalRequest,
) -> bool {
    if policy.object_type != active.object_type || policy.object_id != active.object_id {
        return false;
    }

    if policy
        .relation
        .as_ref()
        .is_some_and(|relation| active.relation.as_deref() != Some(relation.as_str()))
    {
        return false;
    }

    match &policy.authorized_scope_id {
        Some(scope_id) => {
            active
                .authorized_scope_id
                .as_ref()
                .is_some_and(|active_scope_id| active_scope_id == scope_id)
                || request
                    .trusted
                    .active_scopes
                    .iter()
                    .any(|active_scope_id| active_scope_id == scope_id)
        }
        None => true,
    }
}

fn grant_matches_memory(grant: &AccessGrant, memory: &MemoryItem) -> bool {
    grant
        .memory_id
        .as_ref()
        .is_some_and(|memory_id| memory_id == &memory.memory_id)
        || grant
            .scope_id
            .as_ref()
            .is_some_and(|scope_id| scope_id == &memory.home_scope_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal_memory(id: &str, scope: &str, title: &str) -> MemoryItem {
        let mut memory = MemoryItem::new(id, scope, title, title);
        memory.status = MemoryStatus::Confirmed;
        memory.retrieval_policy_status = RetrievalPolicyStatus::Valid;
        memory.participant_visibility_policy = ParticipantVisibilityPolicy::AnyActiveHuman;
        memory
    }

    fn sensitive_memory(id: &str, scope: &str, title: &str, owner: &str) -> MemoryItem {
        let mut memory = MemoryItem::new(id, scope, title, title);
        memory.status = MemoryStatus::Confirmed;
        memory.sensitivity = Sensitivity::Sensitive;
        memory.retrieval_policy_status = RetrievalPolicyStatus::Valid;
        memory.participant_visibility_policy = ParticipantVisibilityPolicy::OwnerOnly;
        memory.owner_principal_id = Some(owner.to_string());
        memory
    }

    fn request_for_kevin() -> MemoryRetrievalRequest {
        MemoryRetrievalRequest {
            requesting_principal_id: "agent_primary".to_string(),
            trusted: TrustedRetrievalContext::for_human(
                "human_kevin",
                Purpose::AnswerHumanQuestion,
            ),
            untrusted_hints: UntrustedHints::default(),
        }
    }

    #[test]
    fn retrieves_normal_memory_across_same_human_participant() {
        let mut store = MemoryStore::default();
        let mut memory = normal_memory(
            "memory_pref",
            "conversation_old",
            "Current conversation should rank first",
        );
        memory.retrieval_hints.topics = vec!["memory".to_string()];
        store.insert_memory(memory);
        store
            .add_participant("memory_pref", "human_kevin", ParticipantRole::HumanInScope)
            .expect("participant");

        let mut request = request_for_kevin();
        request.untrusted_hints.fuzzy_topics = vec!["memory".to_string()];

        let result = store.retrieve(&request);

        assert_eq!(included_ids(&result), vec!["memory_pref"]);
        assert_eq!(
            result.included[0].eligibility_reason,
            EligibilityReason::ParticipantOverlap
        );
        assert!(result.denied_for_audit.is_empty());
    }

    #[test]
    fn active_scope_ranks_above_older_same_human_memory() {
        let mut store = MemoryStore::default();
        let mut old = normal_memory("memory_old", "conversation_old", "Older preference");
        old.retrieval_hints.topics = vec!["preference".to_string()];
        store.insert_memory(old);
        store
            .add_participant("memory_old", "human_kevin", ParticipantRole::HumanInScope)
            .expect("old participant");

        let mut current = normal_memory("memory_current", "conversation_current", "Current fact");
        current.retrieval_hints.topics = vec!["preference".to_string()];
        store.insert_memory(current);

        let mut request = request_for_kevin();
        request
            .trusted
            .active_scopes
            .push("conversation_current".to_string());
        request.untrusted_hints.fuzzy_topics = vec!["preference".to_string()];

        let result = store.retrieve(&request);

        assert_eq!(included_ids(&result), vec!["memory_current", "memory_old"]);
        assert_eq!(
            result.included[0].eligibility_reason,
            EligibilityReason::ActiveScope
        );
    }

    #[test]
    fn fuzzy_hints_do_not_unlock_sensitive_memory() {
        let mut store = MemoryStore::default();
        let mut memory = sensitive_memory(
            "memory_health",
            "conversation_health",
            "Doctor appointment follow-up",
            "human_kevin",
        );
        memory.retrieval_hints.topics = vec!["health".to_string(), "doctor".to_string()];
        store.insert_memory(memory);
        store
            .add_participant(
                "memory_health",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("participant");
        store
            .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
            .expect("purpose");

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
        request.untrusted_hints.fuzzy_topics = vec!["health".to_string(), "doctor".to_string()];

        let result = store.retrieve(&request);

        assert!(result.included.is_empty());
        assert_eq!(
            result.denied_for_audit[0].reason,
            DenialReason::SensitiveUnlockMissing
        );
        assert_eq!(
            result.agent_visible_omissions,
            vec![AgentVisibleOmission {
                reason: "policy_restricted_context"
            }]
        );
    }

    #[test]
    fn trusted_object_link_unlocks_sensitive_memory() {
        let mut store = MemoryStore::default();
        let memory = sensitive_memory(
            "memory_health",
            "conversation_health",
            "Doctor appointment follow-up",
            "human_kevin",
        );
        store.insert_memory(memory);
        store
            .add_participant(
                "memory_health",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("participant");
        store
            .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
            .expect("purpose");
        store
            .add_object_link(
                "memory_health",
                ObjectLink::new("task", "task_schedule_checkup"),
                "open_loop_for",
                None,
            )
            .expect("object link");

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
        request
            .trusted
            .active_object_links
            .push(ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for"));

        let result = store.retrieve(&request);

        assert_eq!(included_ids(&result), vec!["memory_health"]);
        assert_eq!(
            result.included[0].eligibility_reason,
            EligibilityReason::TrustedObjectLink
        );
    }

    #[test]
    fn trusted_object_link_requires_matching_relation() {
        let mut store = MemoryStore::default();
        let memory = sensitive_memory(
            "memory_health",
            "conversation_health",
            "Doctor appointment follow-up",
            "human_kevin",
        );
        store.insert_memory(memory);
        store
            .add_participant(
                "memory_health",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("participant");
        store
            .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
            .expect("purpose");
        store
            .add_object_link(
                "memory_health",
                ObjectLink::new("task", "task_schedule_checkup"),
                "open_loop_for",
                None,
            )
            .expect("object link");

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
        request
            .trusted
            .active_object_links
            .push(ObjectLink::new("task", "task_schedule_checkup"));
        let missing_relation = store.retrieve(&request);
        assert!(missing_relation.included.is_empty());
        assert_eq!(
            missing_relation.denied_for_audit[0].reason,
            DenialReason::SensitiveUnlockMissing
        );

        request.trusted.active_object_links =
            vec![ObjectLink::new("task", "task_schedule_checkup").with_relation("relevant_to")];
        let wrong_relation = store.retrieve(&request);
        assert!(wrong_relation.included.is_empty());
        assert_eq!(
            wrong_relation.denied_for_audit[0].reason,
            DenialReason::SensitiveUnlockMissing
        );

        request.trusted.active_object_links =
            vec![ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for")];
        let allowed = store.retrieve(&request);
        assert_eq!(included_ids(&allowed), vec!["memory_health"]);
    }

    #[test]
    fn trusted_object_link_requires_authorized_scope_when_policy_sets_one() {
        let mut store = MemoryStore::default();
        let memory = sensitive_memory(
            "memory_health",
            "conversation_health",
            "Doctor appointment follow-up",
            "human_kevin",
        );
        store.insert_memory(memory);
        store
            .add_participant(
                "memory_health",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("participant");
        store
            .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
            .expect("purpose");
        store
            .add_object_link(
                "memory_health",
                ObjectLink::new("task", "task_schedule_checkup"),
                "open_loop_for",
                Some("project_noema".to_string()),
            )
            .expect("object link");

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
        request.trusted.active_object_links =
            vec![ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for")];
        let missing_scope = store.retrieve(&request);
        assert!(missing_scope.included.is_empty());
        assert_eq!(
            missing_scope.denied_for_audit[0].reason,
            DenialReason::SensitiveUnlockMissing
        );

        request.trusted.active_object_links = vec![
            ObjectLink::new("task", "task_schedule_checkup")
                .with_relation("open_loop_for")
                .with_authorized_scope("project_noema"),
        ];
        let allowed = store.retrieve(&request);
        assert_eq!(included_ids(&allowed), vec!["memory_health"]);
    }

    #[test]
    fn stale_sensitive_policy_fails_closed() {
        let mut store = MemoryStore::default();
        let mut memory = sensitive_memory(
            "memory_health",
            "conversation_health",
            "Doctor appointment follow-up",
            "human_kevin",
        );
        memory.retrieval_policy_status = RetrievalPolicyStatus::Stale;
        store.insert_memory(memory);
        store
            .add_participant(
                "memory_health",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("participant");
        store
            .add_purpose_rule("memory_health", Purpose::AnswerHumanQuestion, Effect::Allow)
            .expect("purpose");
        store
            .add_object_link(
                "memory_health",
                ObjectLink::new("task", "task_schedule_checkup"),
                "open_loop_for",
                None,
            )
            .expect("object link");

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
        request
            .trusted
            .active_object_links
            .push(ObjectLink::new("task", "task_schedule_checkup").with_relation("open_loop_for"));

        let result = store.retrieve(&request);

        assert!(result.included.is_empty());
        assert_eq!(
            result.denied_for_audit[0].reason,
            DenialReason::RetrievalPolicyInvalid
        );
    }

    #[test]
    fn private_memory_requires_active_scope_or_explicit_grant() {
        let mut store = MemoryStore::default();
        let mut memory = normal_memory("memory_private", "conversation_private", "Private note");
        memory.sensitivity = Sensitivity::Private;
        memory.participant_visibility_policy = ParticipantVisibilityPolicy::ExplicitGrantOnly;
        store.insert_memory(memory);
        store
            .add_participant(
                "memory_private",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("participant");
        store
            .add_purpose_rule(
                "memory_private",
                Purpose::AnswerHumanQuestion,
                Effect::Allow,
            )
            .expect("purpose");

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Private;

        let denied = store.retrieve(&request);
        assert!(denied.included.is_empty());

        store.add_access_grant(AccessGrant {
            memory_id: Some("memory_private".to_string()),
            scope_id: None,
            principal_id: "agent_primary".to_string(),
            permission: Permission::UseForRetrieval,
            effect: Effect::Allow,
        });

        let allowed = store.retrieve(&request);
        assert_eq!(included_ids(&allowed), vec!["memory_private"]);
    }

    #[test]
    fn private_memory_applies_participant_visibility_policy() {
        let mut store = MemoryStore::default();
        let mut memory = normal_memory("memory_private", "conversation_private", "Private note");
        memory.sensitivity = Sensitivity::Private;
        memory.participant_visibility_policy = ParticipantVisibilityPolicy::OwnerOnly;
        memory.owner_principal_id = Some("human_alex".to_string());
        store.insert_memory(memory);
        store
            .add_participant(
                "memory_private",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("participant");
        store
            .add_purpose_rule(
                "memory_private",
                Purpose::AnswerHumanQuestion,
                Effect::Allow,
            )
            .expect("purpose");

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Private;
        request
            .trusted
            .active_scopes
            .push("conversation_private".to_string());
        let denied = store.retrieve(&request);

        assert!(denied.included.is_empty());
        assert_eq!(
            denied.denied_for_audit[0].reason,
            DenialReason::ParticipantVisibilityDenied
        );
    }

    #[test]
    fn external_egress_policy_denies_external_purposes() {
        let mut store = MemoryStore::default();
        let mut memory = normal_memory("memory_external", "project_noema", "External draft note");
        memory.external_egress_policy = ExternalEgressPolicy::ApprovalRequired;
        store.insert_memory(memory);
        let mut request = MemoryRetrievalRequest {
            requesting_principal_id: "agent_primary".to_string(),
            trusted: TrustedRetrievalContext::for_human(
                "human_kevin",
                Purpose::DraftExternalContent,
            ),
            untrusted_hints: UntrustedHints::default(),
        };
        request
            .trusted
            .active_scopes
            .push("project_noema".to_string());

        let approval_required = store.retrieve(&request);
        assert!(approval_required.included.is_empty());
        assert_eq!(
            approval_required.denied_for_audit[0].reason,
            DenialReason::ExternalEgressApprovalRequired
        );

        let mut allowed = normal_memory("memory_allowed_external", "project_noema", "Allowed");
        allowed.external_egress_policy = ExternalEgressPolicy::Allow;
        store.insert_memory(allowed);
        let allowed = store.retrieve(&request);
        assert!(
            allowed
                .included
                .iter()
                .any(|memory| memory.memory_id == "memory_allowed_external")
        );

        let mut denied = normal_memory("memory_denied_external", "project_noema", "Denied");
        denied.external_egress_policy = ExternalEgressPolicy::Deny;
        store.insert_memory(denied);
        let denied = store.retrieve(&request);
        assert!(denied.denied_for_audit.iter().any(|denial| {
            denial.memory_id.as_deref() == Some("memory_denied_external")
                && denial.reason == DenialReason::ExternalEgressDenied
        }));
    }

    #[test]
    fn active_relationship_requires_supporting_memory_and_provenance() {
        let mut store = MemoryStore::default();
        store.insert_memory(normal_memory(
            "memory_relation",
            "project_noema",
            "Graph decision",
        ));

        let missing_memory = Relationship {
            relationship_id: "rel_missing".to_string(),
            home_scope_id: "project_noema".to_string(),
            subject_entity_id: "project_noema".to_string(),
            predicate: "uses".to_string(),
            object_entity_id: "concept_memory".to_string(),
            memory_id: None,
            status: RelationshipStatus::Active,
        };
        assert_eq!(
            store.add_relationship(missing_memory),
            Err(MemoryStoreError::RelationshipRequiresMemory {
                relationship_id: "rel_missing".to_string()
            })
        );

        let no_provenance = Relationship {
            relationship_id: "rel_no_provenance".to_string(),
            home_scope_id: "project_noema".to_string(),
            subject_entity_id: "project_noema".to_string(),
            predicate: "uses".to_string(),
            object_entity_id: "concept_memory".to_string(),
            memory_id: Some("memory_relation".to_string()),
            status: RelationshipStatus::Active,
        };
        assert_eq!(
            store.add_relationship(no_provenance),
            Err(MemoryStoreError::RelationshipRequiresProvenance {
                relationship_id: "rel_no_provenance".to_string(),
                memory_id: "memory_relation".to_string()
            })
        );

        store
            .add_provenance("memory_relation", "message_1")
            .expect("provenance");
        let supported = Relationship {
            relationship_id: "rel_supported".to_string(),
            home_scope_id: "project_noema".to_string(),
            subject_entity_id: "project_noema".to_string(),
            predicate: "uses".to_string(),
            object_entity_id: "concept_memory".to_string(),
            memory_id: Some("memory_relation".to_string()),
            status: RelationshipStatus::Active,
        };
        assert!(store.add_relationship(supported).is_ok());
    }

    #[test]
    fn one_hop_graph_expansion_returns_backing_memory_only_after_policy() {
        let mut store = MemoryStore::default();
        let anchor = normal_memory("memory_anchor", "project_noema", "Noema memory system");
        store.insert_memory(anchor);
        store
            .add_subject("memory_anchor", "project_noema", SubjectRole::About)
            .expect("anchor subject");

        let backing = normal_memory(
            "memory_relationship",
            "relationship_scope",
            "Noema uses scoped graph claims",
        );
        store.insert_memory(backing);
        store
            .add_participant(
                "memory_relationship",
                "human_kevin",
                ParticipantRole::HumanInScope,
            )
            .expect("relationship participant");
        store
            .add_provenance("memory_relationship", "message_graph")
            .expect("provenance");
        store
            .add_relationship(Relationship {
                relationship_id: "rel_graph".to_string(),
                home_scope_id: "project_noema".to_string(),
                subject_entity_id: "project_noema".to_string(),
                predicate: "uses".to_string(),
                object_entity_id: "concept_scoped_graph_claims".to_string(),
                memory_id: Some("memory_relationship".to_string()),
                status: RelationshipStatus::Active,
            })
            .expect("relationship");
        store.insert_memory(normal_memory(
            "memory_unscoped_relationship",
            "relationship_scope",
            "Unscoped graph claim",
        ));
        store
            .add_provenance("memory_unscoped_relationship", "message_graph_unscoped")
            .expect("unscoped provenance");
        store
            .add_relationship(Relationship {
                relationship_id: "rel_unscoped_graph".to_string(),
                home_scope_id: "project_noema".to_string(),
                subject_entity_id: "project_noema".to_string(),
                predicate: "mentions".to_string(),
                object_entity_id: "concept_unscoped".to_string(),
                memory_id: Some("memory_unscoped_relationship".to_string()),
                status: RelationshipStatus::Active,
            })
            .expect("unscoped relationship");

        let mut request = request_for_kevin();
        request
            .trusted
            .active_scopes
            .push("project_noema".to_string());

        let result = store.retrieve(&request);

        assert_eq!(
            included_ids(&result),
            vec!["memory_anchor", "memory_relationship"]
        );
        assert!(
            result
                .included
                .iter()
                .any(|memory| memory.memory_id == "memory_relationship"
                    && memory.eligibility_reason == EligibilityReason::ParticipantOverlap)
        );
        assert!(result.denied_for_audit.iter().any(|denial| {
            denial.memory_id.as_deref() == Some("memory_unscoped_relationship")
                && denial.reason == DenialReason::OutsideSearchAperture
        }));
    }

    #[test]
    fn denied_graph_edge_does_not_become_agent_visible_detail() {
        let mut store = MemoryStore::default();
        let anchor = normal_memory("memory_anchor", "project_noema", "Noema memory system");
        store.insert_memory(anchor);
        store
            .add_subject("memory_anchor", "project_noema", SubjectRole::About)
            .expect("anchor subject");

        let mut backing = normal_memory("memory_relationship", "secret_scope", "Private edge");
        backing.sensitivity = Sensitivity::Private;
        backing.retrieval_policy_status = RetrievalPolicyStatus::Invalid;
        store.insert_memory(backing);
        store
            .add_provenance("memory_relationship", "message_graph")
            .expect("provenance");
        store
            .add_relationship(Relationship {
                relationship_id: "rel_private".to_string(),
                home_scope_id: "secret_scope".to_string(),
                subject_entity_id: "project_noema".to_string(),
                predicate: "reveals_private_edge".to_string(),
                object_entity_id: "concept_private".to_string(),
                memory_id: Some("memory_relationship".to_string()),
                status: RelationshipStatus::Active,
            })
            .expect("relationship");

        let mut request = request_for_kevin();
        request
            .trusted
            .active_scopes
            .push("project_noema".to_string());
        request.trusted.sensitivity_ceiling = Sensitivity::Private;

        let result = store.retrieve(&request);

        assert_eq!(included_ids(&result), vec!["memory_anchor"]);
        assert_eq!(
            result.agent_visible_omissions,
            vec![AgentVisibleOmission {
                reason: "policy_restricted_context"
            }]
        );
        assert_eq!(
            result.denied_for_audit[0].memory_id.as_deref(),
            Some("memory_relationship")
        );
        assert_eq!(result.denied_for_audit[0].relationship_id, None);
    }

    fn included_ids(result: &MemoryRetrievalResult) -> Vec<&str> {
        result
            .included
            .iter()
            .map(|memory| memory.memory_id.as_str())
            .collect()
    }
}
