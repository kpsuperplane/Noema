use crate::memory::{
    DenialReason, EligibilityReason, MemoryRetrievalResult, MemoryStatus, MemoryUseStage,
    ParticipantRole, Purpose, RankReason, RelationshipStatus, Sensitivity, SubjectRole,
};
use rusqlite::{Connection, Transaction, params};
use serde_json::{Value, json};

use super::{
    error::MemoryPersistenceError,
    models::{ChatMemorySource, MemorySummary, MemoryType, NewChatTurn, NewMemorySubject},
    schema::MEMORY_SCHEMA_SQL,
};

pub(super) const CHAT_SOURCE_ID: &str = "source:chat";
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
            "SELECT COUNT(*) FROM memory_provenance_edges WHERE memory_id = ?1",
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
    pub(super) agent_principal_id: &'a str,
    pub(super) scope_id: Option<&'a str>,
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
          agent_principal_id,
          scope_id,
          purpose,
          details
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            memory_use_id,
            record.context_packet_id,
            record.run_id,
            record.memory_id,
            memory_use_stage_to_db(record.stage),
            record.agent_principal_id,
            record.scope_id,
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

pub(super) fn ensure_principal(
    tx: &Transaction<'_>,
    principal_id: &str,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO principals (principal_id, principal_type, display_name, handle)
        VALUES (?1, ?2, ?3, ?1)
        ON CONFLICT(principal_id) DO UPDATE SET
          updated_at = CURRENT_TIMESTAMP
        ",
        params![
            principal_id,
            infer_principal_type(principal_id),
            display_name(principal_id),
        ],
    )?;
    Ok(())
}

pub(super) fn ensure_scope(
    tx: &Transaction<'_>,
    scope_id: &str,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO scopes (scope_id, scope_type, name, slug)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(scope_id) DO UPDATE SET
          updated_at = CURRENT_TIMESTAMP
        ",
        params![
            scope_id,
            infer_scope_type(scope_id),
            display_name(scope_id),
            slug_for_id(scope_id),
        ],
    )?;
    Ok(())
}

pub(super) fn ensure_source(tx: &Transaction<'_>) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO sources (source_id, source_type, source_name)
        VALUES (?1, 'chat', 'Noema chat')
        ON CONFLICT(source_id) DO UPDATE SET
          updated_at = CURRENT_TIMESTAMP
        ",
        params![CHAT_SOURCE_ID],
    )?;
    Ok(())
}

pub(super) fn ensure_conversation_episode(
    tx: &Transaction<'_>,
    source: &ChatMemorySource,
) -> Result<(), MemoryPersistenceError> {
    ensure_conversation_episode_id(tx, &source.conversation_id)
}

pub(super) fn ensure_conversation_episode_id(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO episodes (
          episode_id,
          home_scope_id,
          source_id,
          episode_type,
          title,
          raw_ref,
          occurred_at,
          metadata
        )
        VALUES (?1, ?1, ?2, 'conversation', ?3, ?1, CURRENT_TIMESTAMP, ?4)
        ON CONFLICT(episode_id) DO UPDATE SET
          home_scope_id = COALESCE(episodes.home_scope_id, excluded.home_scope_id),
          source_id = excluded.source_id
        ",
        params![
            conversation_id,
            CHAT_SOURCE_ID,
            display_name(conversation_id),
            json_to_string(&json!({"source": "chat"}))?,
        ],
    )?;
    Ok(())
}

pub(super) fn upsert_subject_entity(
    tx: &Transaction<'_>,
    home_scope_id: &str,
    subject: &NewMemorySubject,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO entities (
          entity_id,
          home_scope_id,
          entity_type,
          canonical_name,
          aliases,
          linked_principal_id,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(entity_id) DO UPDATE SET
          home_scope_id = COALESCE(entities.home_scope_id, excluded.home_scope_id),
          entity_type = excluded.entity_type,
          canonical_name = excluded.canonical_name,
          aliases = CASE
            WHEN excluded.aliases = '[]' THEN entities.aliases
            ELSE excluded.aliases
          END,
          linked_principal_id = COALESCE(excluded.linked_principal_id, entities.linked_principal_id),
          metadata = CASE
            WHEN excluded.metadata = '{}' THEN entities.metadata
            ELSE excluded.metadata
          END,
          updated_at = CURRENT_TIMESTAMP
        ",
        params![
            subject.entity_id,
            home_scope_id,
            subject.entity_type,
            subject.canonical_name,
            json_to_string(&json!(subject.aliases))?,
            subject.linked_principal_id,
            json_to_string(&subject.metadata)?,
        ],
    )?;
    Ok(())
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

pub(super) struct ChatMessageRecord<'a> {
    pub(super) message_id: &'a str,
    pub(super) episode_id: &'a str,
    pub(super) author_principal_id: &'a str,
    pub(super) role: &'a str,
    pub(super) content: &'a str,
    pub(super) occurred_at: &'a str,
    pub(super) metadata: Value,
}

pub(super) fn insert_chat_message(
    tx: &Transaction<'_>,
    message: ChatMessageRecord<'_>,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT OR IGNORE INTO messages (
          message_id,
          episode_id,
          author_principal_id,
          role,
          content,
          occurred_at,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            message.message_id,
            message.episode_id,
            message.author_principal_id,
            message.role,
            message.content,
            message.occurred_at,
            json_to_string(&message.metadata)?,
        ],
    )?;
    Ok(())
}

pub(super) fn insert_provenance_edge(
    tx: &Transaction<'_>,
    memory_id: &str,
    source_type: &str,
    source_id: &str,
    evidence_excerpt: Option<&str>,
    created_by_principal_id: &str,
    metadata: Value,
) -> Result<(), MemoryPersistenceError> {
    let edge_id = allocate_id(tx, "edge")?;
    tx.execute(
        r"
        INSERT INTO memory_provenance_edges (
          edge_id,
          memory_id,
          source_type,
          source_id,
          relation,
          evidence_excerpt,
          created_by_principal_id,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, 'derived_from', ?5, ?6, ?7)
        ",
        params![
            edge_id,
            memory_id,
            source_type,
            source_id,
            evidence_excerpt,
            created_by_principal_id,
            json_to_string(&metadata)?,
        ],
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
    let sensitivity: String = row.get(4)?;
    Ok(MemorySummary {
        id: row.get(0)?,
        status: parse_memory_status(&status).map_err(enum_to_sql_error)?,
        memory_type: parse_memory_type(&memory_type).map_err(enum_to_sql_error)?,
        home_scope_id: row.get(3)?,
        sensitivity: parse_sensitivity(&sensitivity).map_err(enum_to_sql_error)?,
        title: row.get(5)?,
        content: row.get(6)?,
        created_at: row.get(7)?,
        source_type: row.get(8)?,
        source_id: row.get(9)?,
        conversation_id: row.get(10)?,
    })
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

pub(super) fn chat_message_id(conversation_id: &str, role: &str, turn_index: u64) -> String {
    format!("message:{conversation_id}:{role}:{turn_index}")
}

pub(super) fn chat_message_metadata(turn: &NewChatTurn, message_role: &str) -> Value {
    json!({
        "source": "chat",
        "turn_index": turn.turn_index,
        "message_role": message_role,
        "turn_metadata": turn.metadata,
    })
}

pub(super) fn title_from_content(content: &str) -> String {
    let mut title: String = content.trim().chars().take(80).collect();
    if title.is_empty() {
        title.push_str("Untitled memory");
    }
    title
}

pub(super) fn display_name(id: &str) -> String {
    id.rsplit(':')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(id)
        .replace('_', " ")
}

pub(super) fn slug_for_id(id: &str) -> String {
    let slug = id.replace([':', '/', ' '], "_");
    if slug.is_empty() {
        "scope".to_string()
    } else {
        slug
    }
}

pub(super) fn infer_principal_type(principal_id: &str) -> &'static str {
    if principal_id.starts_with("human") {
        "human"
    } else if principal_id.starts_with("agent") {
        "agent"
    } else if principal_id.starts_with("tool") {
        "tool"
    } else {
        "system"
    }
}

pub(super) fn infer_scope_type(scope_id: &str) -> &'static str {
    if scope_id.starts_with("system") {
        "system"
    } else if scope_id.starts_with("human") {
        "human"
    } else if scope_id.starts_with("workspace") {
        "workspace"
    } else if scope_id.starts_with("project") {
        "project"
    } else if scope_id.starts_with("task") {
        "task"
    } else if scope_id.starts_with("cron") {
        "cron"
    } else if scope_id.starts_with("conversation") {
        "conversation"
    } else if scope_id.starts_with("agent") {
        "agent"
    } else if scope_id.starts_with("relationship") {
        "relationship"
    } else if scope_id.starts_with("tool") {
        "tool"
    } else {
        "custom"
    }
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
