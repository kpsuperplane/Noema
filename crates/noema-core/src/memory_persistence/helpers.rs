use crate::memory::{
    DenialReason, EligibilityReason, MemoryRetrievalResult, MemoryStatus, MemoryUseStage,
    ParticipantRole, Purpose, RankReason, RelationshipStatus, Sensitivity, SubjectRole,
};
use rusqlite::{Connection, Transaction, params};
use serde_json::{Value, json};

use super::{
    error::MemoryPersistenceError,
    models::{MemorySummary, MemoryType},
    schema::MEMORY_SCHEMA_SQL,
};

pub(super) const DEFAULT_LIMIT: u32 = 50;
pub(super) const MAX_LIMIT: u32 = 500;
const BOOTSTRAP_SCHEMA_VERSION: i64 = 0;
const BOOTSTRAP_SCHEMA_NAME: &str = "memory_persistence_bootstrap_v0";

pub(super) fn configure_connection(conn: &Connection) -> Result<(), MemoryPersistenceError> {
    conn.execute_batch(
        r"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA synchronous = NORMAL;
        ",
    )?;
    Ok(())
}

pub(super) fn configure_read_only_connection(
    conn: &Connection,
) -> Result<(), MemoryPersistenceError> {
    conn.execute_batch(
        r"
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA query_only = ON;
        ",
    )?;
    Ok(())
}

pub(super) fn migrate(conn: &Connection) -> Result<(), MemoryPersistenceError> {
    conn.execute_batch(MEMORY_SCHEMA_SQL)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name) VALUES (?1, ?2)",
        params![BOOTSTRAP_SCHEMA_VERSION, BOOTSTRAP_SCHEMA_NAME],
    )?;
    Ok(())
}

pub(super) fn memory_has_provenance(
    tx: &Transaction<'_>,
    memory_id: &str,
) -> Result<bool, MemoryPersistenceError> {
    let count: i64 = tx
        .query_row(
            r"
            SELECT COUNT(*)
            FROM object_provenance_edges
            WHERE target_object_type = 'memory_item'
              AND target_object_id = ?1
              AND deleted_at IS NULL
            ",
            params![memory_id],
            |row| row.get(0),
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    Ok(count > 0)
}

pub(super) struct ContextMemoryUseInsert<'a> {
    pub(super) context_packet_id: &'a str,
    pub(super) run_id: &'a str,
    pub(super) memory_id: &'a str,
    pub(super) stage: MemoryUseStage,
    pub(super) agent_object_type: &'a str,
    pub(super) agent_object_id: &'a str,
    pub(super) context_object_type: Option<&'a str>,
    pub(super) context_object_id: Option<&'a str>,
    pub(super) purpose: Purpose,
}

pub(super) fn insert_memory_use_record(
    tx: &Transaction<'_>,
    record: ContextMemoryUseInsert<'_>,
) -> Result<(), MemoryPersistenceError> {
    let memory_use_id = allocate_id(tx, "memuse")?;
    tx.execute(
        r"
        INSERT INTO memory_use_records (
          memory_use_id,
          context_packet_id,
          run_id,
          memory_id,
          stage,
          agent_object_type,
          agent_object_id,
          context_object_type,
          context_object_id,
          purpose,
          details
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ",
        params![
            memory_use_id,
            record.context_packet_id,
            record.run_id,
            record.memory_id,
            memory_use_stage_to_db(record.stage),
            record.agent_object_type,
            record.agent_object_id,
            record.context_object_type,
            record.context_object_id,
            purpose_to_db(record.purpose),
            json_to_string(&json!({
                "context_packet_id": record.context_packet_id,
                "run_id": record.run_id,
                "stage": memory_use_stage_to_db(record.stage),
            }))?,
        ],
    )
    .map_err(MemoryPersistenceError::Sqlite)?;
    Ok(())
}

pub(super) fn memory_sensitivity_for_tx(
    tx: &Transaction<'_>,
    memory_id: &str,
) -> Result<String, MemoryPersistenceError> {
    tx.query_row(
        "SELECT sensitivity FROM memory_items WHERE memory_id = ?1",
        params![memory_id],
        |row| row.get(0),
    )
    .map_err(MemoryPersistenceError::Sqlite)
}

pub(super) fn agent_visible_omissions_json(result: &MemoryRetrievalResult) -> Vec<Value> {
    result
        .agent_visible_omissions
        .iter()
        .map(|omission| json!({ "reason": omission.reason }))
        .collect()
}

pub(super) fn upsert_memory_fts(
    tx: &Transaction<'_>,
    memory_id: &str,
    title: &str,
    content: &str,
    retrieval_hints: &str,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        "DELETE FROM memory_fts WHERE memory_id = ?1",
        params![memory_id],
    )?;
    tx.execute(
        r"
        INSERT INTO memory_fts (memory_id, title, content, retrieval_hints)
        VALUES (?1, ?2, ?3, ?4)
        ",
        params![memory_id, title, content, retrieval_hints],
    )?;
    Ok(())
}

pub(super) fn allocate_id(
    conn: &Connection,
    prefix: &str,
) -> Result<String, MemoryPersistenceError> {
    let hex = conn.query_row("SELECT lower(hex(randomblob(16)))", [], |row| {
        row.get::<_, String>(0)
    })?;
    Ok(format!("{prefix}_{hex}"))
}

pub(super) fn row_to_memory_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemorySummary> {
    let status: String = row.get(1)?;
    let memory_type: String = row.get(2)?;
    let owner_object_type: String = row.get(3)?;
    let owner_object_id: String = row.get(4)?;
    let sensitivity: String = row.get(5)?;
    let source_object_type: Option<String> = row.get(9)?;
    let source_object_id: Option<String> = row.get(10)?;
    Ok(MemorySummary {
        id: row.get(0)?,
        status: parse_memory_status(&status).map_err(enum_to_sql_error)?,
        memory_type: parse_memory_type(&memory_type).map_err(enum_to_sql_error)?,
        home_scope_id: object_ref_key(&owner_object_type, &owner_object_id),
        owner_object_type,
        owner_object_id,
        sensitivity: parse_sensitivity(&sensitivity).map_err(enum_to_sql_error)?,
        title: row.get(6)?,
        content: row.get(7)?,
        created_at: row.get(8)?,
        source_object_type: source_object_type.clone(),
        source_object_id: source_object_id.clone(),
        source_type: source_object_type,
        source_id: source_object_id,
        conversation_id: row.get(11)?,
    })
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

pub(super) fn enum_to_sql_error(error: MemoryPersistenceError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

pub(super) fn json_to_string(value: &Value) -> Result<String, MemoryPersistenceError> {
    Ok(serde_json::to_string(value)?)
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

pub(super) fn relationship_status_to_db(status: RelationshipStatus) -> &'static str {
    match status {
        RelationshipStatus::Candidate => "candidate",
        RelationshipStatus::Active => "active",
        RelationshipStatus::Confirmed => "confirmed",
        RelationshipStatus::Superseded => "superseded",
        RelationshipStatus::Archived => "archived",
        RelationshipStatus::Deleted => "deleted",
        RelationshipStatus::Disputed => "disputed",
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
