//! SQLite row adapters for durable task agent runs.

use noema_providers::{ProviderSelectionSnapshot, ReasoningEffort};
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
        .get::<_, String>(12)?
        .parse::<noema_providers::ProviderSelectionMode>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                12,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let reasoning_effort = row
        .get::<_, Option<String>>(14)?
        .as_deref()
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
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
        .get::<_, String>(22)?
        .parse::<RunStatus>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                22,
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
            provider_instance_key: None,
            provider_kind: row.get(10)?,
            provider_account_id: row.get(11)?,
            selection_mode,
            model_profile: row.get(13)?,
            reasoning_effort,
            selection_source: row.get(15)?,
        },
        actual_provider_kind: row.get(16)?,
        actual_model_profile: row.get(17)?,
        execution_policy: TaskExecutionPolicy {
            max_provider_continuations: row.get(18)?,
            max_tool_calls: row.get(19)?,
            max_active_minutes: row.get(20)?,
            progress_audit_interval: row.get(21)?,
        },
        status,
        priority: row.get(23)?,
        queued_at: row.get(24)?,
        lease_owner: row.get(25)?,
        lease_token: row.get(26)?,
        lease_expires_at: row.get(27)?,
        heartbeat_at: row.get(28)?,
        started_at: row.get(29)?,
        ended_at: row.get(30)?,
        cancellation_requested: row.get::<_, i64>(31)? != 0,
        retry_count: row.get(32)?,
        error_code: row.get(33)?,
        error_message: row.get(34)?,
        provider_call_count: row.get(35)?,
        tool_call_count: row.get(36)?,
        input_tokens: row.get(37)?,
        cached_input_tokens: row.get(38)?,
        output_tokens: row.get(39)?,
        active_milliseconds: row.get(40)?,
        created_at: row.get(41)?,
        updated_at: row.get(42)?,
    })
}
