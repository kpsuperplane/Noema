use crate::memory::{
    DenialReason, EligibilityReason, MemoryRetrievalResult, MemoryStatus, MemoryUseStage,
    ParticipantRole, Purpose, RankReason, Sensitivity, SubjectRole,
};
use serde_json::{Value, json};

use super::{
    error::MemoryPersistenceError,
    models::{MemorySummary, MemoryType},
};

pub(super) const DEFAULT_LIMIT: u32 = 50;
pub(super) const MAX_LIMIT: u32 = 500;

pub(super) struct ContextMemoryUseInsert<'a> {
    pub(super) context_packet_id: &'a str,
    pub(super) run_id: &'a str,
    pub(super) memory_id: &'a str,
    pub(super) stage: MemoryUseStage,
    pub(super) agent_actor_id: &'a str,
    pub(super) context_object_type: Option<&'a str>,
    pub(super) context_object_id: Option<&'a str>,
    pub(super) purpose: Purpose,
}

pub(super) fn agent_visible_omissions_json(result: &MemoryRetrievalResult) -> Vec<Value> {
    result
        .agent_visible_omissions
        .iter()
        .map(|omission| json!({ "reason": omission.reason }))
        .collect()
}

pub(super) fn object_ref_key(object_type: &str, object_id: &str) -> String {
    let prefix = format!("{object_type}:");
    if object_id.starts_with(&prefix) {
        object_id.to_string()
    } else {
        format!("{object_type}:{object_id}")
    }
}

pub(super) fn redact_for_list(mut memory: MemorySummary) -> MemorySummary {
    if matches!(
        memory.sensitivity,
        Sensitivity::Sensitive | Sensitivity::Secret
    ) {
        memory.title = "[redacted]".to_string();
        memory.content = "[redacted]".to_string();
    }
    memory
}

pub(super) fn title_from_content(content: &str) -> String {
    let mut title: String = content.trim().chars().take(80).collect();
    if title.is_empty() {
        title.push_str("Untitled memory");
    }
    title
}

pub(super) fn sensitivity_to_db(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

pub(super) fn memory_status_to_db(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Candidate => "candidate",
        MemoryStatus::Active => "active",
        MemoryStatus::Confirmed => "confirmed",
        MemoryStatus::Inferred => "inferred",
        MemoryStatus::Stale => "stale",
        MemoryStatus::Superseded => "superseded",
        MemoryStatus::Archived => "archived",
        MemoryStatus::Deleted => "deleted",
        MemoryStatus::Disputed => "disputed",
    }
}

pub(super) fn purpose_to_db(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::AnswerHumanQuestion => "answer_human_question",
        Purpose::DraftInternalContent => "draft_internal_content",
        Purpose::GeneralPersonalization => "general_personalization",
        Purpose::ManageTask => "manage_task",
        Purpose::ManageCalendar => "manage_calendar",
        Purpose::DraftExternalContent => "draft_external_content",
        Purpose::UseTool => "use_tool",
        Purpose::ProactiveSuggestion => "proactive_suggestion",
        Purpose::ExternalAction => "external_action",
        Purpose::DebugAudit => "debug_audit",
    }
}

pub(super) fn memory_use_stage_to_db(stage: MemoryUseStage) -> &'static str {
    match stage {
        MemoryUseStage::Retrieved => "retrieved",
        MemoryUseStage::IncludedInPacket => "included_in_packet",
        MemoryUseStage::ShownToAgent => "shown_to_agent",
        MemoryUseStage::UsedInReply => "used_in_reply",
        MemoryUseStage::UsedForAction => "used_for_action",
        MemoryUseStage::UsedForProactivity => "used_for_proactivity",
    }
}

pub(super) fn eligibility_reason_to_db(reason: EligibilityReason) -> &'static str {
    match reason {
        EligibilityReason::ActiveScope => "active_scope",
        EligibilityReason::ParticipantOverlap => "participant_overlap",
        EligibilityReason::ExplicitGrant => "explicit_grant",
        EligibilityReason::TrustedObjectLink => "trusted_object_link",
        EligibilityReason::PublicHint => "public_hint",
        EligibilityReason::GraphExpansion => "graph_expansion",
    }
}

pub(super) fn rank_reason_to_db(reason: RankReason) -> &'static str {
    match reason {
        RankReason::ExplicitMemoryRequest => "explicit_memory_request",
        RankReason::TrustedObjectLink => "trusted_object_link",
        RankReason::PublicHint => "public_hint",
        RankReason::FuzzyTopic => "fuzzy_topic",
        RankReason::FuzzyKeyword => "fuzzy_keyword",
        RankReason::SameHumanParticipant => "same_human_participant",
        RankReason::GraphExpansion => "graph_expansion",
    }
}

pub(super) fn denial_reason_to_db(reason: DenialReason) -> &'static str {
    match reason {
        DenialReason::ArchivedOrDeleted => "archived_or_deleted",
        DenialReason::CandidateExcluded => "candidate_excluded",
        DenialReason::DisputedOrStale => "disputed_or_stale",
        DenialReason::ExplicitDenyGrant => "explicit_deny_grant",
        DenialReason::OutsideSearchAperture => "outside_search_aperture",
        DenialReason::SensitivityCeiling => "sensitivity_ceiling",
        DenialReason::RetrievalPolicyInvalid => "retrieval_policy_invalid",
        DenialReason::PurposeDenied => "purpose_denied",
        DenialReason::ParticipantVisibilityDenied => "participant_visibility_denied",
        DenialReason::SensitiveUnlockMissing => "sensitive_unlock_missing",
        DenialReason::SecretApprovalMissing => "secret_approval_missing",
        DenialReason::RelationshipUnsupported => "relationship_unsupported",
        DenialReason::ExternalEgressDenied => "external_egress_denied",
        DenialReason::ExternalEgressApprovalRequired => "external_egress_approval_required",
    }
}

pub(super) fn parse_sensitivity(value: &str) -> Result<Sensitivity, MemoryPersistenceError> {
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

pub(super) fn parse_memory_status(value: &str) -> Result<MemoryStatus, MemoryPersistenceError> {
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

pub(super) fn parse_memory_type(value: &str) -> Result<MemoryType, MemoryPersistenceError> {
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

pub(super) fn participant_role_to_db(role: ParticipantRole) -> &'static str {
    match role {
        ParticipantRole::HumanInScope => "human_in_scope",
        ParticipantRole::AgentInScope => "agent_in_scope",
        ParticipantRole::Originator => "originator",
        ParticipantRole::Observer => "observer",
    }
}

pub(super) fn subject_role_to_db(role: SubjectRole) -> &'static str {
    match role {
        SubjectRole::About => "about",
        SubjectRole::Claimant => "claimant",
        SubjectRole::Affected => "affected",
        SubjectRole::Owner => "owner",
        SubjectRole::Assignee => "assignee",
        SubjectRole::Source => "source",
        SubjectRole::Target => "target",
    }
}
