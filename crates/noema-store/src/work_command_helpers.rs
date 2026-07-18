//! Shared SQL helpers for semantic Work command transactions.

use std::str::FromStr;

use noema_providers::{
    ProviderInstanceKey, ProviderSelectionMode, ProviderSelectionSnapshot, ReasoningEffort,
};
use noema_tasks::{
    CaptureTask, DelegateTask, TaskContractId, TaskExecutionPolicy, TaskId, WorkCommand,
    WorkDomainError, WorkEventRecord, WorkflowStageBehavior, WorkflowStageId,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{
    StoreError,
    work_command_result::ReceiptResponse,
    work_commands::{
        canonical_command_fingerprint, capture_source_fingerprint, command_actor_id,
        command_idempotency_key, delegate_source_fingerprint,
    },
};

#[path = "work_command_run_queue.rs"]
mod run_queue;

pub(crate) use run_queue::{
    QueuePinnedChildRun, QueueRun, provider_route_unavailable, queue_pinned_child_run_tx,
    queue_run_tx,
};

pub(crate) use crate::work_command_result::materialize_result;

#[derive(Debug, Clone)]
pub(crate) struct TaskState {
    pub task_id: TaskId,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub stage_id: WorkflowStageId,
    pub stage_behavior: WorkflowStageBehavior,
    pub generation: u64,
    pub revision: u64,
    pub current_contract_id: Option<TaskContractId>,
    pub active_gate_id: Option<noema_tasks::TaskGateId>,
    pub latest_run_id: Option<String>,
    pub latest_submission_id: Option<String>,
    pub latest_review_id: Option<String>,
    pub title: String,
    pub description_markdown: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandEventContext<'a> {
    pub actor_id: &'a str,
    pub causation_id: Option<&'a str>,
    pub correlation_id: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CommandWrite {
    pub task_id: Option<TaskId>,
    pub project_id: Option<ProjectId>,
    pub contract_id: Option<TaskContractId>,
    pub gate_id: Option<noema_tasks::TaskGateId>,
    pub run_id: Option<String>,
    pub event_id: noema_tasks::WorkEventId,
    pub event_sequence: u64,
    #[serde(default)]
    pub task_snapshot: Option<noema_tasks::TaskRecord>,
    #[serde(default)]
    pub task_detail_snapshot: Option<crate::WorkTaskDetail>,
    #[serde(default)]
    pub project_snapshot: Option<noema_workspaces::ProjectRecord>,
}

/// Query one task's scalar state and fail closed on unknown persisted enums.
pub(crate) fn load_task_state_tx(
    transaction: &Transaction<'_>,
    task_id: &TaskId,
) -> Result<TaskState, StoreError> {
    let row = transaction
        .query_row(
            r#"SELECT t.workspace_id, t.project_id, t.stage_id,
                      s.system_behavior, t.generation, t.revision,
                      t.current_contract_id, t.active_gate_id, t.latest_run_id,
                      t.latest_submission_id, t.latest_review_id,
                      t.title, t.description_markdown
               FROM tasks t
               JOIN workflow_stages s
                 ON s.workflow_id = t.workflow_id AND s.stage_id = t.stage_id
               WHERE t.task_id = ?1 LIMIT 1"#,
            [task_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                ))
            },
        )
        .optional()?;
    let row = row.ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    let generation = positive_u64(row.4, "task.generation")?;
    let revision = positive_u64(row.5, "task.revision")?;
    Ok(TaskState {
        task_id: task_id.clone(),
        workspace_id: WorkspaceId::new(row.0).map_err(StoreError::Workspace)?,
        project_id: row
            .1
            .map(ProjectId::new)
            .transpose()
            .map_err(StoreError::Workspace)?,
        stage_id: WorkflowStageId::new(row.2).map_err(StoreError::Work)?,
        stage_behavior: WorkflowStageBehavior::from_str(&row.3).map_err(StoreError::Work)?,
        generation,
        revision,
        current_contract_id: row
            .6
            .map(TaskContractId::new)
            .transpose()
            .map_err(StoreError::Work)?,
        active_gate_id: row
            .7
            .map(noema_tasks::TaskGateId::new)
            .transpose()
            .map_err(StoreError::Work)?,
        latest_run_id: row.8,
        latest_submission_id: row.9,
        latest_review_id: row.10,
        title: row.11,
        description_markdown: row.12,
    })
}

pub(crate) fn check_task_fence(
    task: &TaskState,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StoreError> {
    if task.revision != expected_revision {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }
    if task.generation != expected_generation {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    Ok(())
}

pub(crate) fn positive_u64(value: i64, field: &'static str) -> Result<u64, StoreError> {
    let value = u64::try_from(value).map_err(|_| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field,
            message: "persisted value must be positive".to_string(),
        })
    })?;
    if value == 0 {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field,
            message: "persisted value must be positive".to_string(),
        }));
    }
    Ok(value)
}

pub(crate) fn positive_u32(value: i64, field: &'static str) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field,
            message: "persisted value is outside the u32 range".to_string(),
        })
    })
}

pub(crate) fn nonnegative_u32(value: i64, field: &'static str) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field,
            message: "persisted value is outside the u32 range".to_string(),
        })
    })
}

pub(crate) fn nonnegative_u64(value: i64, field: &'static str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field,
            message: "persisted value cannot be negative".to_string(),
        })
    })
}

pub(crate) fn load_policy_tx(
    transaction: &Transaction<'_>,
) -> Result<TaskExecutionPolicy, StoreError> {
    let values = transaction
        .query_row(
            "SELECT max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, max_automatic_retries, max_review_rounds FROM task_execution_policy WHERE policy_id = 'default'",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .map_err(StoreError::Sqlite)?;
    let policy = TaskExecutionPolicy {
        max_provider_continuations: positive_u32(values.0, "policy.max_provider_continuations")?,
        max_tool_calls: positive_u32(values.1, "policy.max_tool_calls")?,
        max_active_minutes: positive_u32(values.2, "policy.max_active_minutes")?,
        progress_audit_interval: positive_u32(values.3, "policy.progress_audit_interval")?,
        max_automatic_retries: nonnegative_u32(values.4, "policy.max_automatic_retries")?,
        max_review_rounds: positive_u32(values.5, "policy.max_review_rounds")?,
    };
    policy.validated().map_err(StoreError::Work)
}

/// Load one provider snapshot from an immutable contract row.
pub(crate) fn load_contract_model_tx(
    transaction: &Transaction<'_>,
    contract_id: &TaskContractId,
    reviewer: bool,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let columns = if reviewer {
        "reviewer_provider_kind, reviewer_provider_account_id, reviewer_provider_instance_key, reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort, reviewer_selection_source"
    } else {
        "executor_provider_kind, executor_provider_account_id, executor_provider_instance_key, executor_selection_mode, executor_model_profile, executor_reasoning_effort, executor_selection_source"
    };
    transaction
        .query_row(
            &format!("SELECT {columns} FROM task_execution_contracts WHERE contract_id = ?1"),
            [contract_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::Work(WorkDomainError::ContractRequired))
        .and_then(provider_snapshot)
}

pub(crate) fn provider_snapshot(
    row: (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ),
) -> Result<ProviderSelectionSnapshot, StoreError> {
    Ok(ProviderSelectionSnapshot {
        provider_kind: row.0,
        provider_account_id: row.1,
        provider_instance_key: Some(ProviderInstanceKey::new(row.2).map_err(|error| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "provider_instance_key",
                message: error.to_string(),
            })
        })?),
        selection_mode: ProviderSelectionMode::from_str(&row.3).map_err(|error| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "provider.selection_mode",
                message: error.to_string(),
            })
        })?,
        model_profile: row.4,
        reasoning_effort: row
            .5
            .map(|value| {
                ReasoningEffort::from_persistence_str(&value).ok_or_else(|| {
                    StoreError::Work(WorkDomainError::InvalidInput {
                        field: "provider.reasoning_effort",
                        message: format!("unknown value {value}"),
                    })
                })
            })
            .transpose()?,
        selection_source: row.6,
    })
}

pub(crate) fn lookup_receipt_tx(
    transaction: &Transaction<'_>,
    command: &WorkCommand,
) -> Result<Option<CommandWrite>, StoreError> {
    let Some(idempotency_key) = command_idempotency_key(command) else {
        return Ok(None);
    };
    let fingerprint = canonical_command_fingerprint(command)?;
    let row = transaction
        .query_row(
            "SELECT request_fingerprint, response_json FROM work_command_receipts WHERE actor_id = ?1 AND command_name = ?2 AND idempotency_key = ?3",
            params![command_actor_id(command), command.name(), idempotency_key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((stored_fingerprint, response_json)) = row else {
        return Ok(None);
    };
    if stored_fingerprint != fingerprint {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    let response: ReceiptResponse = serde_json::from_str(&response_json)?;
    Ok(Some(response.into()))
}

pub(crate) fn lookup_delegate_source_receipt_tx(
    transaction: &Transaction<'_>,
    command: &DelegateTask,
) -> Result<Option<CommandWrite>, StoreError> {
    let (Some(conversation_id), Some(tool_call_id)) = (
        command.provenance.conversation_id.as_deref(),
        command.provenance.source_tool_call_id.as_deref(),
    ) else {
        return Ok(None);
    };
    let task_id: Option<String> = transaction
        .query_row(
            "SELECT task_id FROM tasks WHERE source_conversation_id = ?1 AND source_tool_call_id = ?2 LIMIT 1",
            params![conversation_id, tool_call_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(task_id) = task_id else {
        return Ok(None);
    };
    let response_json: Option<String> = transaction
        .query_row(
            "SELECT response_json FROM work_command_receipts WHERE command_name = 'task.delegate' AND result_task_id = ?1 ORDER BY created_at, actor_id, idempotency_key LIMIT 1",
            [task_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(response_json) = response_json else {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    };
    let response: ReceiptResponse = serde_json::from_str(&response_json)?;
    if response.delegate_source_fingerprint.as_deref()
        != Some(delegate_source_fingerprint(command)?.as_str())
        || response.task_id.as_ref().map(TaskId::as_str) != Some(task_id.as_str())
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    Ok(Some(response.into()))
}

pub(crate) fn lookup_capture_source_receipt_tx(
    transaction: &Transaction<'_>,
    command: &CaptureTask,
) -> Result<Option<CommandWrite>, StoreError> {
    let (Some(conversation_id), Some(tool_call_id)) = (
        command.provenance.conversation_id.as_deref(),
        command.provenance.source_tool_call_id.as_deref(),
    ) else {
        return Ok(None);
    };
    let task_id: Option<String> = transaction
        .query_row(
            "SELECT task_id FROM tasks WHERE source_conversation_id = ?1 AND source_tool_call_id = ?2 LIMIT 1",
            params![conversation_id, tool_call_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(task_id) = task_id else {
        return Ok(None);
    };
    let response_json: Option<String> = transaction
        .query_row(
            "SELECT response_json FROM work_command_receipts WHERE command_name = 'task.capture' AND result_task_id = ?1 ORDER BY created_at, actor_id, idempotency_key LIMIT 1",
            [task_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(response_json) = response_json else {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    };
    let response: ReceiptResponse = serde_json::from_str(&response_json)?;
    if response.capture_source_fingerprint.as_deref()
        != Some(capture_source_fingerprint(command)?.as_str())
        || response.task_id.as_ref().map(TaskId::as_str) != Some(task_id.as_str())
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    Ok(Some(response.into()))
}

pub(crate) fn save_receipt_tx(
    transaction: &Transaction<'_>,
    command: &WorkCommand,
    write: &mut CommandWrite,
) -> Result<(), StoreError> {
    capture_write_snapshot_tx(transaction, write)?;
    let source_receipt_key;
    let idempotency_key = if let Some(key) = command_idempotency_key(command) {
        key
    } else if matches!(command, WorkCommand::CaptureTask(command) if command.provenance.conversation_id.is_some() && command.provenance.source_tool_call_id.is_some())
    {
        let task_id = write
            .task_id
            .as_ref()
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "source-owned Capture receipt has no task identity".to_string(),
            })?;
        source_receipt_key = format!("source-replay:{}", task_id.as_str());
        &source_receipt_key
    } else {
        return Ok(());
    };
    let fingerprint = canonical_command_fingerprint(command)?;
    let mut response = ReceiptResponse::from(&*write);
    if let WorkCommand::DelegateTask(command) = command {
        response.delegate_source_fingerprint = Some(delegate_source_fingerprint(command)?);
    } else if let WorkCommand::CaptureTask(command) = command {
        response.capture_source_fingerprint = Some(capture_source_fingerprint(command)?);
    }
    let response = serde_json::to_string(&response)?;
    transaction.execute(
        "INSERT INTO work_command_receipts (actor_id, command_name, idempotency_key, request_fingerprint, result_task_id, result_project_id, result_event_sequence, response_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            command_actor_id(command),
            command.name(),
            idempotency_key,
            fingerprint,
            write.task_id.as_ref().map(ToString::to_string),
            write.project_id.as_ref().map(ToString::to_string),
            i64::try_from(write.event_sequence).map_err(|_| StoreError::InvariantViolation {
                message: "event sequence exceeds SQLite integer range".to_string(),
            })?,
            response,
        ],
    )?;
    Ok(())
}

pub(crate) fn capture_write_snapshot_tx(
    transaction: &Transaction<'_>,
    write: &mut CommandWrite,
) -> Result<(), StoreError> {
    if let Some(task_id) = write.task_id.as_ref() {
        let detail = crate::work_reads::task::load_task_facts(transaction, task_id)?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("command result task {task_id} disappeared before commit"),
            })?
            .into_detail();
        write.task_snapshot = Some(detail.task.clone());
        write.task_detail_snapshot = Some(detail);
    }
    if let Some(project_id) = write.project_id.as_ref() {
        write.project_snapshot = Some(crate::work_reads::rows::load_project(
            transaction,
            project_id,
        )?);
    }
    Ok(())
}

pub(crate) fn write_marker(
    event: WorkEventRecord,
    task_id: Option<TaskId>,
    project_id: Option<ProjectId>,
    contract_id: Option<TaskContractId>,
    gate_id: Option<noema_tasks::TaskGateId>,
    run_id: Option<String>,
) -> CommandWrite {
    CommandWrite {
        task_id,
        project_id,
        contract_id,
        gate_id,
        run_id,
        event_id: event.event_id,
        event_sequence: event.event_sequence,
        task_snapshot: None,
        task_detail_snapshot: None,
        project_snapshot: None,
    }
}
