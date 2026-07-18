//! Lease-fenced planner, executor, reviewer, and failure terminal writes.

use noema_tasks::{
    RunKind, RunStatus, RunTerminalKind, SafeErrorCode, TaskGateKind, TaskStageChangeReason,
    WorkDomainError, WorkEventKind, WorkEventPayload, WorkReconciliationAction,
    WorkReconciliationSnapshot, WorkflowStageBehavior, WorkflowStageId, plan_reconciliation_action,
    reported_failure_facts,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{ReportRunFailure, ReportTaskBlocked, SubmitTaskResult, WorkRunTerminal};
use crate::{
    StoreError,
    ids::allocate_id,
    work_commands::{WorkCommandService, helpers},
    work_events::append_work_event_tx,
    work_notifications::enqueue_work_notification_tx,
};

#[path = "work_run_terminal_helpers.rs"]
mod terminal_helpers;
#[path = "work_run_terminal_plan.rs"]
mod terminal_plan;
#[path = "work_run_terminal_replay.rs"]
mod terminal_replay;
#[path = "work_run_terminal_review.rs"]
mod terminal_review;

use terminal_helpers::{
    OpenGate, bump_task_to_waiting_tx, contract_criterion_ids_tx, insert_gate_tx,
    load_fenced_run_tx, load_running_fence_tx, mark_run_completed_tx, mark_run_waiting_tx, scope,
    validate_namespace,
};

impl WorkCommandService {
    /// Persist exactly one role-specific terminal from a leased worker.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal validation, idempotency comparison, lease
    /// fencing, or atomic terminal persistence fails.
    pub async fn record_work_run_terminal(
        &self,
        terminal: WorkRunTerminal,
        actor_id: &str,
        causation_id: Option<&str>,
        correlation_id: &str,
    ) -> Result<noema_tasks::WorkCommandResult, StoreError> {
        terminal.validate().map_err(StoreError::Work)?;
        if actor_id.trim().is_empty() || correlation_id.trim().is_empty() {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "run.event_scope",
                message: "actor and correlation identifiers are required".to_string(),
            }));
        }
        let write = self
            .store
            .with_immediate_transaction_retry(|transaction| {
                let mut write = if let Some(replay) =
                    terminal_replay::replay_terminal_tx(transaction, &terminal)?
                {
                    replay
                } else {
                    match &terminal {
                        WorkRunTerminal::Plan(value) => terminal_plan::submit_plan_tx(
                            transaction,
                            self,
                            value,
                            actor_id,
                            causation_id,
                            correlation_id,
                        ),
                        WorkRunTerminal::TaskResult(value) => submit_result_tx(
                            transaction,
                            self,
                            value,
                            actor_id,
                            causation_id,
                            correlation_id,
                        ),
                        WorkRunTerminal::Review(value) => terminal_review::submit_review_tx(
                            transaction,
                            self,
                            value,
                            actor_id,
                            causation_id,
                            correlation_id,
                        ),
                        WorkRunTerminal::Blocked(value) => report_blocked_tx(
                            transaction,
                            value,
                            actor_id,
                            causation_id,
                            correlation_id,
                        ),
                    }?
                };
                helpers::capture_write_snapshot_tx(transaction, &mut write)?;
                Ok(write)
            })
            .await?;
        helpers::materialize_result(&self.store, write).await
    }

    /// Persist a safe failure/interruption and either queue a bounded retry or
    /// open a human Recovery gate.
    ///
    /// # Errors
    ///
    /// Returns an error when failure validation, replay comparison, lease
    /// fencing, recovery planning, or atomic persistence fails.
    pub async fn report_work_run_failure(
        &self,
        report: ReportRunFailure,
        actor_id: &str,
        causation_id: Option<&str>,
        correlation_id: &str,
    ) -> Result<noema_tasks::WorkCommandResult, StoreError> {
        report.validate().map_err(StoreError::Work)?;
        let write = self
            .store
            .with_immediate_transaction_retry(|transaction| {
                let mut write = if let Some(replay) =
                    terminal_replay::replay_failure_tx(transaction, &report)?
                {
                    replay
                } else {
                    report_failure_tx(
                        transaction,
                        self,
                        &report,
                        actor_id,
                        causation_id,
                        correlation_id,
                    )?
                };
                helpers::capture_write_snapshot_tx(transaction, &mut write)?;
                Ok(write)
            })
            .await?;
        helpers::materialize_result(&self.store, write).await
    }
}

fn submit_result_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    command: &SubmitTaskResult,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> Result<helpers::CommandWrite, StoreError> {
    let (run, task) = load_running_fence_tx(transaction, &command.fence, RunKind::Executor)?;
    let contract_id = command
        .fence
        .contract_id
        .as_ref()
        .ok_or(StoreError::Work(WorkDomainError::ContractRequired))?;
    let criterion_ids = contract_criterion_ids_tx(transaction, contract_id)?;
    let submission = command
        .submission
        .normalized(&criterion_ids)
        .map_err(StoreError::Work)?;
    if submission.task_id != task.task_id
        || submission.contract_id != *contract_id
        || submission.executor_run_id != run.run_id
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let submission_id = submission
        .submission_id
        .clone()
        .unwrap_or_else(|| allocate_id("submission"));
    validate_namespace(&submission_id, "submission:", "submission.submission_id")?;
    if task.stage_behavior != WorkflowStageBehavior::Active {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let artifact_versions = submission
        .artifact_ids
        .iter()
        .map(|artifact_id| {
            submitted_artifact_version_tx(transaction, artifact_id, &run, &task)
                .map(|version_id| (artifact_id, version_id))
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    transaction.execute(
        "INSERT INTO task_submissions (submission_id, task_id, contract_id, executor_run_id, review_round, summary, result_markdown) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            &submission_id,
            task.task_id.as_str(),
            contract_id.as_str(),
            run.run_id,
            submission.review_round,
            submission.summary,
            submission.result_markdown,
        ],
    )?;
    for criterion in &submission.criteria {
        transaction.execute(
            "INSERT INTO task_submission_criteria (submission_id, criterion_id, evidence_markdown) VALUES (?1, ?2, ?3)",
            params![&submission_id, criterion.criterion_id, criterion.evidence_markdown],
        )?;
    }
    // The submission row's generated id is used above after insertion.  Keep
    // the input normalized with the allocated id for the event/result marker.
    let persisted_submission_id = transaction.query_row(
        "SELECT submission_id FROM task_submissions WHERE executor_run_id = ?1",
        [run.run_id.as_str()],
        |row| row.get::<_, String>(0),
    )?;
    for (ordinal, (artifact_id, version_id)) in artifact_versions.iter().enumerate() {
        let ordinal = i64::try_from(ordinal + 1).map_err(|_| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "submission.artifact_ids",
                message: "artifact ordinal exceeds SQLite range".to_string(),
            })
        })?;
        transaction.execute(
            "INSERT INTO task_submission_artifacts (submission_id, ordinal, artifact_id, artifact_version_id) VALUES (?1, ?2, ?3, ?4)",
            params![persisted_submission_id, ordinal, artifact_id, version_id],
        )?;
    }
    transaction.execute(
        "UPDATE tasks SET latest_submission_id = ?2, latest_run_id = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?4",
        params![task.task_id.as_str(), &persisted_submission_id, run.run_id, task.generation],
    )?;
    mark_run_completed_tx(transaction, &run, &command.fence)?;
    let _submission_event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::submission_created(
            persisted_submission_id.clone(),
            contract_id.clone(),
            submission.review_round,
            u32::try_from(submission.criteria.len()).unwrap_or(u32::MAX),
            u32::try_from(submission.artifact_ids.len()).unwrap_or(u32::MAX),
        )
        .map_err(StoreError::Work)?,
    )?;
    let _completed_event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::run_completed(
            run.run_kind,
            run.task_generation,
            RunTerminalKind::Submission,
        )
        .map_err(StoreError::Work)?,
    )?;
    let (reviewer_run_id, review_event) = helpers::queue_run_tx(
        transaction,
        service.provider_registry.as_ref(),
        &task,
        helpers::QueueRun {
            run_kind: RunKind::Reviewer,
            contract_id: Some(contract_id),
            planner_complexity: None,
            review_round: submission.review_round,
            attempt_index: 0,
            parent_run_id: Some(&run.run_id),
            triggering_submission_id: Some(&persisted_submission_id),
            triggering_review_id: None,
            event: helpers::CommandEventContext {
                actor_id,
                causation_id,
                correlation_id,
            },
        },
    )?;
    let event = review_event;
    Ok(helpers::write_marker(
        event,
        Some(task.task_id),
        None,
        Some(contract_id.clone()),
        None,
        Some(reviewer_run_id),
    ))
}

fn submitted_artifact_version_tx(
    transaction: &Transaction<'_>,
    artifact_id: &str,
    run: &noema_tasks::AgentRunRecord,
    task: &helpers::TaskState,
) -> Result<String, StoreError> {
    let artifact = transaction
        .query_row(
            "SELECT artifact.owner_object_type, artifact.owner_object_id,
                    artifact.current_version_id, artifact.created_by_actor_id,
                    version.created_by_actor_id, artifact.metadata_json
             FROM artifacts AS artifact
             JOIN artifact_versions AS version
               ON version.artifact_version_id = artifact.current_version_id
              AND version.artifact_id = artifact.artifact_id
             WHERE artifact.artifact_id = ?1 AND artifact.deleted_at IS NULL",
            [artifact_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((owner_kind, owner_id, version_id, creator_id, version_creator_id, metadata_json)) =
        artifact
    else {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    };
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json)?;
    let metadata_task_id = metadata.get("task_id").and_then(serde_json::Value::as_str);
    let metadata_run_id = metadata
        .get("task_run_id")
        .and_then(serde_json::Value::as_str);
    if owner_kind != "task"
        || owner_id != task.task_id.as_str()
        || metadata_task_id != Some(task.task_id.as_str())
        || metadata_run_id != Some(run.run_id.as_str())
        || creator_id != run.agent_id
        || version_creator_id != run.agent_id
        || run.run_kind != RunKind::Executor
        || run.task_id != task.task_id
        || run.task_generation != task.generation
        || run.contract_id != task.current_contract_id
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    Ok(version_id)
}

fn report_blocked_tx(
    transaction: &Transaction<'_>,
    command: &ReportTaskBlocked,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> Result<helpers::CommandWrite, StoreError> {
    let (run, mut task) = load_running_fence_tx(transaction, &command.fence, RunKind::Executor)?;
    if task.stage_behavior != WorkflowStageBehavior::Active {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let gate_id = insert_gate_tx(
        transaction,
        &task,
        OpenGate {
            gate_kind: command.gate_kind,
            prompt: &terminal_replay::normalize_required(&command.prompt_markdown),
            context: &terminal_replay::normalize_optional(&command.context_markdown),
            actor_id,
            originating_run_id: Some(&run.run_id),
            recovery_reason: None,
            retry_run_kind: None,
        },
    )?;
    let _revision = bump_task_to_waiting_tx(transaction, &mut task)?;
    mark_run_waiting_tx(transaction, &run, &command.fence)?;
    let _gate_event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::gate_opened(
            gate_id.clone(),
            task.generation,
            command.gate_kind,
            Some(run.run_id.clone()),
            None,
            None,
        )
        .map_err(StoreError::Work)?,
    )?;
    let _stage_event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::task_stage_changed(
            task.revision,
            task.generation,
            WorkflowStageId::new("stage:personal:doing").map_err(StoreError::Work)?,
            task.stage_id.clone(),
            TaskStageChangeReason::GateOpened,
        )
        .map_err(StoreError::Work)?,
    )?;
    let mut event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::run_waiting_for_approval(
            run.run_kind,
            run.task_generation,
            gate_id.clone(),
            command.gate_kind,
        )
        .map_err(StoreError::Work)?,
    )?;
    if let Some(notification_event) = enqueue_work_notification_tx(
        transaction,
        &event,
        noema_tasks::NotificationKind::TaskWaiting,
        &serde_json::json!({"task_id": task.task_id.as_str(), "gate_id": gate_id.as_str()}),
    )? {
        event = notification_event;
    }
    Ok(helpers::write_marker(
        event,
        Some(task.task_id),
        None,
        run.contract_id,
        Some(gate_id),
        Some(run.run_id),
    ))
}

pub(crate) fn report_failure_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    report: &ReportRunFailure,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> Result<helpers::CommandWrite, StoreError> {
    report_failure_tx_inner(
        transaction,
        service,
        report,
        actor_id,
        causation_id,
        correlation_id,
        false,
    )
}

pub(crate) fn report_expired_failure_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    report: &ReportRunFailure,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> Result<helpers::CommandWrite, StoreError> {
    report_failure_tx_inner(
        transaction,
        service,
        report,
        actor_id,
        causation_id,
        correlation_id,
        true,
    )
}

fn report_failure_tx_inner(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    report: &ReportRunFailure,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
    allow_expired_lease: bool,
) -> Result<helpers::CommandWrite, StoreError> {
    let (run, mut task) =
        load_fenced_run_tx(transaction, &report.fence, None, allow_expired_lease)?;
    if !matches!(run.status, RunStatus::Running | RunStatus::Leased) {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let error_message = terminal_replay::normalize_failure_message(report.error_message.as_deref());
    let changed = transaction.execute(
        if allow_expired_lease {
            "UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?5 AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AND task_generation = ?6 AND (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) = ?6 AND status IN ('running', 'leased') AND cancellation_requested = 0"
        } else {
            "UPDATE agent_runs SET status = ?2, error_code = ?3, error_message = ?4, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?5 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AND task_generation = ?6 AND (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) = ?6 AND status IN ('running', 'leased') AND cancellation_requested = 0"
        },
        params![report.fence.run_id, report.status.as_str(), report.error_code.as_str(), error_message, report.fence.lease_token, report.fence.task_generation],
    )?;
    if changed != 1 {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let _failure_event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::run_failure(
            if report.status == RunStatus::Interrupted {
                WorkEventKind::RunInterrupted
            } else {
                WorkEventKind::RunFailed
            },
            run.run_kind,
            run.task_generation,
            run.attempt_index,
            report.error_code.clone(),
            report.retryable,
        )
        .map_err(StoreError::Work)?,
    )?;
    let mut action = plan_reported_failure(&task, &run, report)?;
    let queued_run_kind = match &action {
        WorkReconciliationAction::QueueRun { run_kind } => Some(*run_kind),
        _ => None,
    };
    if let Some(run_kind) = queued_run_kind {
        if run_kind != run.run_kind {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "failure recovery changed run {} from {} to {}",
                    run.run_id, run.run_kind, run_kind
                ),
            });
        }
        let next_attempt = run.attempt_index.checked_add(1).ok_or_else(|| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "run.attempt_index",
                message: "attempt index overflow".to_string(),
            })
        })?;
        match helpers::queue_pinned_child_run_tx(
            transaction,
            service.provider_registry.as_ref(),
            &task,
            &run,
            helpers::QueuePinnedChildRun {
                attempt_index: next_attempt,
                triggering_submission_id: run.triggering_submission_id.as_deref(),
                triggering_review_id: run.triggering_review_id.as_deref(),
                event: helpers::CommandEventContext {
                    actor_id,
                    causation_id,
                    correlation_id,
                },
            },
        ) {
            Ok((child_run_id, event)) => {
                return Ok(helpers::write_marker(
                    event,
                    Some(task.task_id),
                    None,
                    run.contract_id,
                    None,
                    Some(child_run_id),
                ));
            }
            Err(error) if helpers::provider_route_unavailable(&error) => {
                let route_error =
                    SafeErrorCode::new("configuration_unavailable").map_err(StoreError::Work)?;
                action = plan_recovery_action(&task, &run, report.status, &route_error, false)?;
            }
            Err(error) => return Err(error),
        }
    }
    let WorkReconciliationAction::OpenRecoveryGate {
        reason,
        retry_run_kind,
    } = action
    else {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "failure recovery planner returned an unsupported action for run {}",
                run.run_id
            ),
        });
    };
    let gate_id = insert_gate_tx(
        transaction,
        &task,
        OpenGate {
            gate_kind: TaskGateKind::Recovery,
            prompt: "The run failed and needs a recovery decision.",
            context: &error_message,
            actor_id,
            originating_run_id: Some(&run.run_id),
            recovery_reason: Some(reason),
            retry_run_kind,
        },
    )?;
    let _revision = bump_task_to_waiting_tx(transaction, &mut task)?;
    let _gate_event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::gate_opened(
            gate_id.clone(),
            task.generation,
            TaskGateKind::Recovery,
            Some(run.run_id.clone()),
            Some(reason),
            retry_run_kind,
        )
        .map_err(StoreError::Work)?,
    )?;
    let mut event = append_work_event_tx(
        transaction,
        scope(
            &task,
            actor_id,
            causation_id,
            correlation_id,
            Some(&run.run_id),
        ),
        WorkEventPayload::task_stage_changed(
            task.revision,
            task.generation,
            WorkflowStageId::new("stage:personal:doing").map_err(StoreError::Work)?,
            task.stage_id.clone(),
            TaskStageChangeReason::Recovery,
        )
        .map_err(StoreError::Work)?,
    )?;
    if let Some(notification_event) = enqueue_work_notification_tx(
        transaction,
        &event,
        noema_tasks::NotificationKind::TaskRecovery,
        &serde_json::json!({"task_id": task.task_id.as_str(), "gate_id": gate_id.as_str()}),
    )? {
        event = notification_event;
    }
    Ok(helpers::write_marker(
        event,
        Some(task.task_id),
        None,
        run.contract_id,
        Some(gate_id),
        Some(run.run_id),
    ))
}

fn plan_reported_failure(
    task: &helpers::TaskState,
    run: &noema_tasks::AgentRunRecord,
    report: &ReportRunFailure,
) -> Result<WorkReconciliationAction, StoreError> {
    plan_recovery_action(
        task,
        run,
        report.status,
        &report.error_code,
        report.retryable,
    )
}

fn plan_recovery_action(
    task: &helpers::TaskState,
    run: &noema_tasks::AgentRunRecord,
    status: RunStatus,
    error_code: &SafeErrorCode,
    retryable: bool,
) -> Result<WorkReconciliationAction, StoreError> {
    let facts = reported_failure_facts(
        run.run_kind,
        status,
        error_code,
        retryable,
        retryable && run.attempt_index >= run.execution_policy.max_automatic_retries,
    );
    plan_reconciliation_action(WorkReconciliationSnapshot {
        stage_behavior: task.stage_behavior,
        has_current_contract: task.current_contract_id.is_some(),
        failed_run: Some(facts),
        ..WorkReconciliationSnapshot::default()
    })
    .map_err(StoreError::Work)
}
