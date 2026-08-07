//! Durable-fact reconciliation bridge.
//!
//! Reconciliation decisions use only bounded durable facts, never process state,
//! transcripts, provider prose, or UI labels.

#[path = "work_reconciliation_queue.rs"]
mod queue;
#[path = "work_reconciliation_snapshot.rs"]
mod snapshot;

use std::str::FromStr;

use noema_tasks::{
    RunKind, TaskGateKind, TaskRecoveryReason, WorkCommandResult, WorkDomainError, WorkEventKind,
    WorkEventPayload, WorkReconciliationAction, WorkReconciliationSnapshot, WorkflowStageBehavior,
    WorkflowStageId, plan_reconciliation_action,
};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{
    StoreError,
    ids::allocate_id,
    work_commands::{WorkCommandService, helpers},
    work_events::{WorkEventScope, append_work_event_tx},
    work_notifications::enqueue_work_notification_tx,
    work_records::WorkReconciliationEnvelope,
};

/// Derive exactly one action from durable rows.
///
/// # Errors
///
/// Returns an error when the durable envelope violates reconciliation invariants.
pub fn plan_work_reconciliation(
    envelope: &WorkReconciliationEnvelope,
) -> Result<WorkReconciliationAction, StoreError> {
    plan_reconciliation_action(envelope.snapshot.clone()).map_err(StoreError::Work)
}

/// Validate a caller-supplied snapshot before handing it to the domain planner.
///
/// This is useful in focused tests and in recovery code that reconstructs a
/// snapshot from a bounded SQL projection.  It intentionally performs no SQL.
///
/// # Errors
///
/// Returns an error when the supplied facts do not describe a valid workflow state.
pub fn plan_snapshot(
    snapshot: WorkReconciliationSnapshot,
) -> Result<WorkReconciliationAction, StoreError> {
    plan_reconciliation_action(snapshot).map_err(StoreError::Work)
}

/// Store-owned request to apply one pure reconciliation action to a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReconciliation {
    /// Task identity whose durable envelope is planned inside the write transaction.
    pub task_id: noema_tasks::TaskId,
    /// Actor/component responsible for the derived action.
    pub actor_id: String,
    /// Correlation identity for all generated ledger events.
    pub correlation_id: String,
    /// Optional direct causation event/run identity.
    pub causation_id: Option<String>,
}

impl ApplyReconciliation {
    /// Validate the store-owned reconciliation request metadata.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] for blank actor/correlation identifiers.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.actor_id.trim().is_empty() || self.correlation_id.trim().is_empty() {
            return Err(WorkDomainError::InvalidInput {
                field: "reconciliation.metadata",
                message: "actor and correlation cannot be blank".to_string(),
            });
        }
        Ok(())
    }

    fn event_context(&self) -> helpers::CommandEventContext<'_> {
        helpers::CommandEventContext {
            actor_id: &self.actor_id,
            causation_id: self.causation_id.as_deref(),
            correlation_id: &self.correlation_id,
        }
    }

    fn scope(&self, task: &helpers::TaskState, run_id: Option<&str>) -> WorkEventScope {
        self.event_context().task_scope(task, run_id)
    }
}

/// Role carried by a queued reconciliation action.
#[must_use]
pub const fn action_run_kind(action: &WorkReconciliationAction) -> Option<RunKind> {
    match action {
        WorkReconciliationAction::QueueRun { run_kind }
        | WorkReconciliationAction::MoveToQueueAndQueueRun { run_kind }
        | WorkReconciliationAction::OpenRecoveryGate {
            retry_run_kind: Some(run_kind),
            ..
        } => Some(*run_kind),
        _ => None,
    }
}

impl WorkCommandService {
    /// Atomically derive and apply the exact action from current durable facts.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid, durable facts are
    /// inconsistent, or the selected action cannot be committed atomically.
    pub async fn apply_work_reconciliation_action(
        &self,
        request: ApplyReconciliation,
    ) -> Result<WorkCommandResult, StoreError> {
        let write = self
            .store
            .with_immediate_transaction_retry(|transaction| {
                request.validate().map_err(StoreError::Work)?;
                let facts =
                    crate::work_reads::task::load_task_facts(transaction, &request.task_id)?
                        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
                let envelope = snapshot::derive_envelope(transaction, facts)?;
                let action = plan_work_reconciliation(&envelope)?;
                action
                    .validate_for_contract(envelope.current_contract.is_some())
                    .map_err(StoreError::Work)?;
                let mut task = helpers::load_task_state_tx(transaction, &request.task_id)?;
                let write = match &action {
                    WorkReconciliationAction::Idle => latest_task_marker(transaction, &task),
                    WorkReconciliationAction::QueueRun { run_kind }
                    | WorkReconciliationAction::MoveToQueueAndQueueRun { run_kind } => {
                        if let Some(marker) = already_queued_run_tx(transaction, &task, *run_kind)?
                        {
                            return capture_snapshot(transaction, marker);
                        }
                        if matches!(action, WorkReconciliationAction::QueueRun { .. }) {
                            queue::queue_reconciled_run_tx(
                                transaction,
                                self,
                                &task,
                                *run_kind,
                                &request,
                            )
                        } else {
                            queue::move_to_queue_and_queue_run_tx(
                                transaction,
                                self,
                                &mut task,
                                *run_kind,
                                &request,
                            )
                        }
                    }
                    WorkReconciliationAction::MoveToDone => {
                        let review_id = task.latest_review_id.clone().ok_or_else(|| {
                            StoreError::InvariantViolation {
                                message: format!("task {} has no approved review", task.task_id),
                            }
                        })?;
                        let submission_id = task.latest_submission_id.clone().ok_or_else(|| {
                            StoreError::InvariantViolation {
                                message: format!(
                                    "task {} has no reviewed submission",
                                    task.task_id
                                ),
                            }
                        })?;
                        helpers::complete_review_tx(
                            transaction,
                            &mut task,
                            &review_id,
                            &submission_id,
                            helpers::CommandEventContext {
                                actor_id: &request.actor_id,
                                causation_id: request.causation_id.as_deref(),
                                correlation_id: &request.correlation_id,
                            },
                            None,
                        )
                    }
                    WorkReconciliationAction::OpenRecoveryGate {
                        reason,
                        retry_run_kind,
                    } => open_recovery_gate_tx(
                        transaction,
                        &mut task,
                        *reason,
                        *retry_run_kind,
                        &request,
                    ),
                    WorkReconciliationAction::FenceStaleRuns => {
                        fence_stale_runs_tx(transaction, &task, &request)
                    }
                    WorkReconciliationAction::MaterializePlannedContractAndQueueExecutor => {
                        Err(StoreError::Work(WorkDomainError::ConfigurationUnavailable))
                    }
                }?;
                capture_snapshot(transaction, write)
            })
            .await?;
        helpers::materialize_result(&self.store, write).await
    }
}

fn capture_snapshot(
    transaction: &Transaction<'_>,
    mut write: helpers::CommandWrite,
) -> Result<helpers::CommandWrite, StoreError> {
    helpers::capture_write_snapshot_tx(transaction, &mut write)?;
    Ok(write)
}

fn latest_task_marker(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
) -> Result<helpers::CommandWrite, StoreError> {
    let event = transaction
        .query_row(
            "SELECT event_sequence, event_id, event_kind, workspace_id, project_id, task_id, run_id, actor_id, causation_id, correlation_id, payload_json, created_at FROM work_events WHERE task_id = ?1 ORDER BY event_sequence DESC LIMIT 1",
            [task.task_id.as_str()],
            crate::work_events::work_event_from_row,
        )
        .optional()?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    Ok(task_write(event, task).run(task.latest_run_id.clone()))
}

fn task_write(
    event: noema_tasks::WorkEventRecord,
    task: &helpers::TaskState,
) -> helpers::CommandWrite {
    helpers::task_write(event, task.task_id.clone())
        .project(task.project_id.clone())
        .contract(task.current_contract_id.clone())
        .gate(task.active_gate_id.clone())
}

fn already_queued_run_tx(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
    run_kind: RunKind,
) -> Result<Option<helpers::CommandWrite>, StoreError> {
    let Some((run_id, kind, generation, status)) = transaction
        .query_row(
            "SELECT run_id, run_kind, task_generation, status FROM agent_runs WHERE run_id = ?1",
            [task.latest_run_id.as_deref().unwrap_or("")],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    if RunKind::from_str(&kind).map_err(StoreError::Work)? != run_kind
        || helpers::positive_u64(generation, "run.task_generation")? != task.generation
        || !matches!(status.as_str(), "queued" | "leased" | "running")
    {
        return Ok(None);
    }
    let event = transaction
        .query_row(
            "SELECT event_sequence, event_id, event_kind, workspace_id, project_id, task_id, run_id, actor_id, causation_id, correlation_id, payload_json, created_at FROM work_events WHERE run_id = ?1 ORDER BY event_sequence DESC LIMIT 1",
            [run_id.as_str()],
            crate::work_events::work_event_from_row,
        )
        .optional()?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    Ok(Some(task_write(event, task).run(Some(run_id))))
}

fn open_recovery_gate_tx(
    transaction: &Transaction<'_>,
    task: &mut helpers::TaskState,
    reason: TaskRecoveryReason,
    retry_run_kind: Option<RunKind>,
    request: &ApplyReconciliation,
) -> Result<helpers::CommandWrite, StoreError> {
    if task.stage_behavior == WorkflowStageBehavior::HumanGate {
        let Some(gate_id) = task.active_gate_id.as_ref() else {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        };
        let existing = transaction
            .query_row(
                "SELECT recovery_reason, retry_run_kind, gate_state FROM task_gates WHERE gate_id = ?1 AND task_id = ?2",
                params![gate_id.as_str(), task.task_id.as_str()],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?)),
            )
            .optional()?
            .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
        if existing.2 == "open"
            && existing.0.as_deref() == Some(reason.as_str())
            && existing.1.as_deref() == retry_run_kind.map(RunKind::as_str)
        {
            return capture_snapshot(transaction, latest_task_marker(transaction, task)?);
        }
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    let gate_id = noema_tasks::TaskGateId::new(allocate_id("gate")).map_err(StoreError::Work)?;
    transaction.execute(
        "INSERT INTO task_gates (gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, context_markdown, opened_by_actor_id, originating_run_id) VALUES (?1, ?2, ?3, ?4, 'recovery', 'open', ?5, ?6, 'Reconciliation requires a recovery decision.', 'The durable work facts are inconsistent or exhausted.', ?7, ?8)",
        params![gate_id.as_str(), task.task_id.as_str(), task.generation, task.current_contract_id.as_ref().map(ToString::to_string), reason.as_str(), retry_run_kind.map(|kind| kind.as_str()), request.actor_id, task.latest_run_id],
    )?;
    let revision = helpers::increment(task.revision, "task.revision")?;
    transaction.execute(
        "UPDATE tasks SET stage_id = 'stage:personal:waiting', active_gate_id = ?2, queued_at = NULL, revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?4 AND revision = ?5",
        params![task.task_id.as_str(), gate_id.as_str(), revision, task.generation, task.revision],
    )?;
    let from_stage = task.stage_id.clone();
    task.stage_id = WorkflowStageId::new("stage:personal:waiting").map_err(StoreError::Work)?;
    task.stage_behavior = WorkflowStageBehavior::HumanGate;
    task.active_gate_id = Some(gate_id.clone());
    task.revision = revision;
    let _gate_event = append_work_event_tx(
        transaction,
        request.scope(task, None),
        WorkEventPayload::gate_opened(
            gate_id.clone(),
            task.generation,
            TaskGateKind::Recovery,
            task.latest_run_id.clone(),
            Some(reason),
            retry_run_kind,
        )
        .map_err(StoreError::Work)?,
    )?;
    let mut event = append_work_event_tx(
        transaction,
        request.scope(task, None),
        WorkEventPayload::task_stage_changed(
            revision,
            task.generation,
            from_stage,
            task.stage_id.clone(),
            noema_tasks::TaskStageChangeReason::Recovery,
        )
        .map_err(StoreError::Work)?,
    )?;
    if let Some(notification_event) = enqueue_work_notification_tx(
        transaction,
        &event,
        noema_tasks::NotificationKind::TaskRecovery,
        &serde_json::json!({
            "task_id": task.task_id.as_str(),
            "gate_id": gate_id.as_str(),
            "action_needed": true,
        }),
    )? {
        event = notification_event;
    }
    Ok(task_write(event, task)
        .gate(Some(gate_id))
        .run(task.latest_run_id.clone()))
}

fn fence_stale_runs_tx(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
    request: &ApplyReconciliation,
) -> Result<helpers::CommandWrite, StoreError> {
    let mut statement = transaction.prepare("SELECT run_id, run_kind, task_generation FROM agent_runs WHERE task_id = ?1 AND status IN ('queued', 'leased', 'running')")?;
    let rows = statement
        .query_map([task.task_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut marker = latest_task_marker(transaction, task)?;
    for (run_id, kind, generation) in rows {
        let run_kind = kind.parse::<RunKind>().map_err(StoreError::Work)?;
        let generation = helpers::positive_u64(generation, "run.task_generation")?;
        transaction.execute("UPDATE agent_runs SET status = 'cancelled', cancellation_requested = 1, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status IN ('queued', 'leased', 'running')", [run_id.as_str()])?;
        let record = append_work_event_tx(
            transaction,
            request.scope(task, Some(&run_id)),
            WorkEventPayload::run_cancelled(
                WorkEventKind::RunCancelled,
                run_kind,
                generation,
                if generation == task.generation {
                    noema_tasks::RunCancellationReason::Superseded
                } else {
                    noema_tasks::RunCancellationReason::StaleGeneration
                },
            )
            .map_err(StoreError::Work)?,
        )?;
        marker = task_write(record, task);
    }
    Ok(marker)
}
