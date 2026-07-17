//! SQLite row adapters for durable task agent runs.

use noema_providers::{ProviderInstanceKey, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{AgentRunRecord, RunKind, RunStatus, TaskExecutionPolicy};

pub(super) fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRunRecord> {
    let run_kind = row
        .get::<_, String>(2)?
        .parse::<RunKind>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let selection_mode = row
        .get::<_, String>(13)?
        .parse::<noema_providers::ProviderSelectionMode>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                13,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let reasoning_effort = row
        .get::<_, Option<String>>(15)?
        .as_deref()
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    15,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid reasoning effort",
                    )),
                )
            })
        })
        .transpose()?;
    let status = row
        .get::<_, String>(23)?
        .parse::<RunStatus>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                23,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(AgentRunRecord {
        run_id: row.get(0)?,
        task_id: row.get(1)?,
        run_kind,
        agent_id: row.get(3)?,
        attempt_index: row.get(4)?,
        revision_index: row.get(5)?,
        parent_run_id: row.get(6)?,
        triggering_submission_id: row.get(7)?,
        triggering_review_id: row.get(8)?,
        resume_message: row.get(9)?,
        model: ProviderSelectionSnapshot {
            provider_instance_key: Some(
                ProviderInstanceKey::new(row.get::<_, String>(12)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        12,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
            ),
            provider_kind: row.get(10)?,
            provider_account_id: row.get(11)?,
            selection_mode,
            model_profile: row.get(14)?,
            reasoning_effort,
            selection_source: row.get(16)?,
        },
        actual_provider_kind: row.get(17)?,
        actual_model_profile: row.get(18)?,
        execution_policy: TaskExecutionPolicy {
            max_provider_continuations: row.get(19)?,
            max_tool_calls: row.get(20)?,
            max_active_minutes: row.get(21)?,
            progress_audit_interval: row.get(22)?,
        },
        status,
        priority: row.get(24)?,
        queued_at: row.get(25)?,
        lease_owner: row.get(26)?,
        lease_token: row.get(27)?,
        lease_expires_at: row.get(28)?,
        heartbeat_at: row.get(29)?,
        started_at: row.get(30)?,
        ended_at: row.get(31)?,
        cancellation_requested: row.get::<_, i64>(32)? != 0,
        retry_count: row.get(33)?,
        error_code: row.get(34)?,
        error_message: row.get(35)?,
        provider_call_count: row.get(36)?,
        tool_call_count: row.get(37)?,
        input_tokens: row.get(38)?,
        cached_input_tokens: row.get(39)?,
        output_tokens: row.get(40)?,
        active_milliseconds: row.get(41)?,
        created_at: row.get(42)?,
        updated_at: row.get(43)?,
    })
}
