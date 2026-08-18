//! Shared SQL helpers for semantic Work command transactions.

use std::str::FromStr;

use noema_providers::{
    ProviderInstanceKey, ProviderSelectionMode, ProviderSelectionSnapshot, ReasoningEffort,
};
use noema_tasks::{
    CaptureTask, DelegateTask, PERSONAL_DONE_STAGE_ID, TaskContractId, TaskExecutionPolicy, TaskId,
    TaskStageChangeReason, WorkCommand, WorkDomainError, WorkEventPayload, WorkEventRecord,
    WorkflowStageBehavior, WorkflowStageId,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{
    NoemaStore, StoreError,
    work_command_result::ReceiptResponse,
    work_commands::{
        canonical_command_fingerprint, capture_source_fingerprint, command_actor_id,
        command_idempotency_key, delegate_source_fingerprint,
    },
    work_events::append_work_event_tx,
    work_notifications::enqueue_work_notification_tx,
};

pub(crate) async fn command_transaction(
    store: &NoemaStore,
    envelope: &WorkCommand,
    mut write: impl FnMut(&Transaction<'_>) -> Result<CommandTransactionOutcome, StoreError>,
) -> Result<CommandWrite, StoreError> {
    store
        .with_immediate_transaction_retry(|transaction| {
            if let Some(replay) = lookup_receipt_tx(transaction, envelope)? {
                return Ok(replay);
            }
            match write(transaction)? {
                CommandTransactionOutcome::Replay(write) => Ok(write),
                CommandTransactionOutcome::Write(write) => {
                    finish_write_tx(transaction, envelope, write)
                }
            }
        })
        .await
}

pub(crate) enum CommandTransactionOutcome {
    Replay(CommandWrite),
    Write(CommandWrite),
}

impl From<CommandWrite> for CommandTransactionOutcome {
    fn from(write: CommandWrite) -> Self {
        Self::Write(write)
    }
}

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
    pub executor_agent_id: String,
    pub cwd_override: Option<String>,
    pub task_directory: String,
    pub execution_complexity: Option<noema_tasks::TaskComplexity>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandEventContext<'a> {
    pub actor_id: &'a str,
    pub causation_id: Option<&'a str>,
    pub correlation_id: &'a str,
}

impl CommandEventContext<'_> {
    pub(crate) fn task_scope(
        self,
        task: &TaskState,
        run_id: Option<&str>,
    ) -> crate::work_events::WorkEventScope {
        self.scope(
            &task.workspace_id,
            task.project_id.as_ref(),
            Some(&task.task_id),
            run_id,
        )
    }

    pub(crate) fn scope(
        self,
        workspace_id: &WorkspaceId,
        project_id: Option<&ProjectId>,
        task_id: Option<&TaskId>,
        run_id: Option<&str>,
    ) -> crate::work_events::WorkEventScope {
        crate::work_events::WorkEventScope {
            workspace_id: workspace_id.clone(),
            project_id: project_id.cloned(),
            task_id: task_id.cloned(),
            run_id: run_id.map(str::to_owned),
            actor_id: self.actor_id.to_string(),
            causation_id: self.causation_id.map(str::to_owned),
            correlation_id: self.correlation_id.to_string(),
        }
    }
}

pub(crate) fn event_context(meta: &noema_tasks::CommandMeta) -> CommandEventContext<'_> {
    CommandEventContext {
        actor_id: &meta.actor_id,
        causation_id: meta.causation_id.as_deref(),
        correlation_id: &meta.correlation_id,
    }
}

/// Complete a reviewer-approved submission and enqueue the owner notification.
///
/// Callers must have already verified that the review is current and approved.
/// The approved review is the completion authority.
pub(crate) fn complete_review_tx(
    transaction: &Transaction<'_>,
    task: &mut TaskState,
    review_id: &str,
    submission_id: &str,
    event: CommandEventContext<'_>,
    run_id: Option<&str>,
) -> Result<CommandWrite, StoreError> {
    if task.active_gate_id.is_some() || task.stage_behavior != WorkflowStageBehavior::Active {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let revision = increment(task.revision, "task.revision")?;
    let from_stage = task.stage_id.clone();
    let changed = transaction.execute(
        "UPDATE tasks SET stage_id = ?2, completed_submission_id = ?3, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), queued_at = NULL, revision = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?5 AND generation = ?6",
        params![
            task.task_id.as_str(),
            PERSONAL_DONE_STAGE_ID,
            submission_id,
            revision,
            task.revision,
            task.generation,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }
    task.stage_id = WorkflowStageId::new(PERSONAL_DONE_STAGE_ID).map_err(StoreError::Work)?;
    task.stage_behavior = WorkflowStageBehavior::TerminalSuccess;
    task.revision = revision;
    let scope = event.task_scope(task, run_id);
    let _completed_event = append_work_event_tx(
        transaction,
        scope.clone(),
        WorkEventPayload::task_completed(revision, task.generation).map_err(StoreError::Work)?,
    )?;
    let mut event = append_work_event_tx(
        transaction,
        scope,
        WorkEventPayload::task_stage_changed(
            revision,
            task.generation,
            from_stage,
            task.stage_id.clone(),
            TaskStageChangeReason::Completed,
        )
        .map_err(StoreError::Work)?,
    )?;
    if let Some(notification_event) = enqueue_work_notification_tx(
        transaction,
        &event,
        noema_tasks::NotificationKind::TaskCompleted,
        &serde_json::json!({
            "task_id": task.task_id.as_str(),
            "submission_id": submission_id,
            "review_id": review_id,
            "action_needed": false,
        }),
    )? {
        event = notification_event;
    }
    Ok(task_write(event, task.task_id.clone()).contract(task.current_contract_id.clone()))
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
                      t.title, t.description_markdown, t.executor_agent_id, t.cwd_override,
                      t.task_directory, t.execution_complexity
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
                    row.get::<_, String>(13)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, String>(15)?,
                    row.get::<_, Option<String>>(16)?,
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
        executor_agent_id: row.13,
        cwd_override: row.14,
        task_directory: row.15,
        execution_complexity: row
            .16
            .map(|value| value.parse())
            .transpose()
            .map_err(StoreError::Work)?,
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

pub(crate) fn load_fenced_task_tx(
    transaction: &Transaction<'_>,
    precondition: &noema_tasks::TaskPrecondition,
) -> Result<TaskState, StoreError> {
    let task = load_task_state_tx(transaction, &precondition.task_id)?;
    check_task_fence(
        &task,
        precondition.expected_revision,
        precondition.expected_generation,
    )?;
    Ok(task)
}

pub(crate) fn increment<T: Counter>(value: T, field: &'static str) -> Result<T, StoreError> {
    value.checked_increment().ok_or_else(|| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field,
            message: format!("{field} overflow"),
        })
    })
}

pub(crate) trait Counter: Sized {
    fn checked_increment(self) -> Option<Self>;
}

impl Counter for u32 {
    fn checked_increment(self) -> Option<Self> {
        self.checked_add(1)
    }
}

impl Counter for u64 {
    fn checked_increment(self) -> Option<Self> {
        self.checked_add(1)
    }
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
        "reviewer_provider_kind, reviewer_provider_account_id, reviewer_provider_instance_key, reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort, reviewer_fast_mode, reviewer_selection_source"
    } else {
        "executor_provider_kind, executor_provider_account_id, executor_provider_instance_key, executor_selection_mode, executor_model_profile, executor_reasoning_effort, executor_fast_mode, executor_selection_source"
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
                    row.get::<_, bool>(6)?,
                    row.get::<_, Option<String>>(7)?,
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
        bool,
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
        fast_mode: row.6,
        selection_source: row.7,
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
    lookup_source_receipt_tx(
        transaction,
        &command.provenance,
        "task.delegate",
        &delegate_source_fingerprint(command)?,
        |response| response.delegate_source_fingerprint.as_deref(),
    )
}

pub(crate) fn lookup_capture_source_receipt_tx(
    transaction: &Transaction<'_>,
    command: &CaptureTask,
) -> Result<Option<CommandWrite>, StoreError> {
    lookup_source_receipt_tx(
        transaction,
        &command.provenance,
        "task.capture",
        &capture_source_fingerprint(command)?,
        |response| response.capture_source_fingerprint.as_deref(),
    )
}

fn lookup_source_receipt_tx(
    transaction: &Transaction<'_>,
    provenance: &noema_tasks::TaskProvenance,
    command_name: &str,
    fingerprint: &str,
    stored_fingerprint: impl FnOnce(&ReceiptResponse) -> Option<&str>,
) -> Result<Option<CommandWrite>, StoreError> {
    let (Some(conversation_id), Some(tool_call_id)) = (
        provenance.conversation_id.as_deref(),
        provenance.source_tool_call_id.as_deref(),
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
            "SELECT response_json FROM work_command_receipts WHERE command_name = ?1 AND result_task_id = ?2 ORDER BY created_at, actor_id, idempotency_key LIMIT 1",
            params![command_name, task_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(response_json) = response_json else {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    };
    let response: ReceiptResponse = serde_json::from_str(&response_json)?;
    if stored_fingerprint(&response) != Some(fingerprint)
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

pub(crate) fn finish_write_tx(
    transaction: &Transaction<'_>,
    command: &WorkCommand,
    mut write: CommandWrite,
) -> Result<CommandWrite, StoreError> {
    save_receipt_tx(transaction, command, &mut write)?;
    Ok(write)
}

pub(crate) fn capture_write_snapshot_tx(
    transaction: &Transaction<'_>,
    write: &mut CommandWrite,
) -> Result<(), StoreError> {
    if let Some(task_id) = write.task_id.as_ref() {
        let facts =
            crate::work_reads::task::load_task_facts(transaction, task_id)?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: format!("command result task {task_id} disappeared before commit"),
                }
            })?;
        let history = crate::work_reads::history::load_task_history(transaction, task_id)?;
        let artifacts =
            crate::work_reads::artifacts::load_recent_task_artifacts(transaction, task_id)?;
        let detail = facts.into_detail(history, artifacts);
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
        event_id: event.event_id().clone(),
        event_sequence: event.event_sequence(),
        task_snapshot: None,
        task_detail_snapshot: None,
        project_snapshot: None,
    }
}

pub(crate) fn task_write(event: WorkEventRecord, task_id: TaskId) -> CommandWrite {
    write_marker(event, Some(task_id), None, None, None, None)
}

pub(crate) fn project_write(event: WorkEventRecord, project_id: ProjectId) -> CommandWrite {
    write_marker(event, None, Some(project_id), None, None, None)
}

impl CommandWrite {
    pub(crate) fn project(mut self, project_id: Option<ProjectId>) -> Self {
        self.project_id = project_id;
        self
    }

    pub(crate) fn contract(mut self, contract_id: Option<TaskContractId>) -> Self {
        self.contract_id = contract_id;
        self
    }

    pub(crate) fn gate(mut self, gate_id: Option<noema_tasks::TaskGateId>) -> Self {
        self.gate_id = gate_id;
        self
    }

    pub(crate) fn run(mut self, run_id: Option<String>) -> Self {
        self.run_id = run_id;
        self
    }
}
