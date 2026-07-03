/// Stable id for a memory item.
pub type MemoryId = String;

/// Lifecycle state for a memory proposal or graph claim.
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

/// Deterministic graph-claim use mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UseMode {
    /// Answer a human question.
    Answer,
    /// Personalize a response.
    Personalize,
    /// Plan future work.
    Plan,
    /// Take or prepare an action.
    Act,
    /// Send a proactive notification.
    Notify,
    /// Inspect or audit state.
    Inspect,
    /// Export data outside the active runtime.
    Export,
}

/// Purpose vocabulary accepted by the local `search_memory` tool arguments.
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

/// Claim lifecycle vocabulary used by graph retrieval policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimStatusForPolicy {
    /// Proposed claim not normally retrievable.
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

/// Trusted graph-claim retrieval request supplied by Noema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRetrievalRequest {
    /// Agent requesting graph claims.
    pub requesting_agent_id: String,
    /// Human entity ids active in this request.
    pub active_human_ids: Vec<String>,
    /// Object entity ids active in this request.
    pub active_object_ids: Vec<String>,
    /// Requested use mode.
    pub use_mode: UseMode,
    /// Whether the human explicitly requested memory.
    pub explicit_memory_request: bool,
    /// Maximum sensitivity this request may include.
    pub sensitivity_ceiling: Sensitivity,
    /// Whether secret access was approved.
    pub approved_secret_access: bool,
}

/// Minimal claim fields needed by deterministic retrieval policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyClaim {
    /// Stable claim id.
    pub claim_id: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Object entity id.
    pub object_entity_id: String,
    /// Use modes allowed by the predicate.
    pub predicate_allowed_use_modes: Vec<UseMode>,
    /// Claim lifecycle state.
    pub status: ClaimStatusForPolicy,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
}

/// Audit-only reason graph claim retrieval was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimDenialReason {
    /// Claim status is not retrievable.
    StatusDenied,
    /// Predicate policy does not allow the requested use mode.
    UseModeDenied,
    /// Claim exceeds the request sensitivity ceiling.
    SensitivityCeiling,
    /// Normal claim did not match an active human context.
    OutsideActiveHumanContext,
    /// Sensitive/private claim lacked an explicit or object-based unlock.
    SensitiveUnlockMissing,
    /// Secret claim lacked explicit request or approved access.
    SecretApprovalMissing,
}

/// Apply deterministic graph-claim retrieval policy.
///
/// # Errors
///
/// Returns a [`ClaimDenialReason`] explaining the first policy gate that denied
/// the claim.
pub fn claim_policy_allows(
    claim: &PolicyClaim,
    request: &ClaimRetrievalRequest,
) -> Result<(), ClaimDenialReason> {
    if !matches!(
        claim.status,
        ClaimStatusForPolicy::Active | ClaimStatusForPolicy::Confirmed
    ) {
        return Err(ClaimDenialReason::StatusDenied);
    }

    if !claim
        .predicate_allowed_use_modes
        .contains(&request.use_mode)
    {
        return Err(ClaimDenialReason::UseModeDenied);
    }

    if claim.sensitivity > request.sensitivity_ceiling {
        return Err(ClaimDenialReason::SensitivityCeiling);
    }

    let active_human_matches = request.active_human_ids.iter().any(|entity_id| {
        entity_id == &claim.subject_entity_id || entity_id == &claim.object_entity_id
    });
    let active_object_matches = request.active_object_ids.iter().any(|entity_id| {
        entity_id == &claim.subject_entity_id || entity_id == &claim.object_entity_id
    });

    match claim.sensitivity {
        Sensitivity::Public => Ok(()),
        Sensitivity::Normal => active_human_matches
            .then_some(())
            .ok_or(ClaimDenialReason::OutsideActiveHumanContext),
        Sensitivity::Private => (request.explicit_memory_request || active_object_matches)
            .then_some(())
            .ok_or(ClaimDenialReason::OutsideActiveHumanContext),
        Sensitivity::Sensitive => (request.explicit_memory_request || active_object_matches)
            .then_some(())
            .ok_or(ClaimDenialReason::SensitiveUnlockMissing),
        Sensitivity::Secret => {
            if request.explicit_memory_request && request.approved_secret_access {
                Ok(())
            } else {
                Err(ClaimDenialReason::SecretApprovalMissing)
            }
        }
    }
}
