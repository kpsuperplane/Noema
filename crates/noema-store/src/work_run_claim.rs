//! FIFO Work-run claiming, lease fencing, and small lifecycle helpers.

use std::{collections::HashSet, str::FromStr};

use noema_tasks::{
    AgentRunHeartbeat, RunCancellationReason, RunStatus, TaskId, TaskStageChangeReason,
    WorkEventKind, WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{ReportRunFailure, WorkRunFence, report_expired_failure_tx, rows::load_run_tx};
use crate::work_commands::{WorkCommandService, helpers};
use crate::{
    StoreError,
    ids::allocate_id,
    work_events::{WorkEventScope, append_work_event_tx},
};

impl WorkCommandService {
    /// Recover expired leases and claim the oldest runnable queued run.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lease parameters, inconsistent queued work,
    /// or an atomic recovery/claim failure.
    pub async fn claim_next_work_run(
        &self,
        worker_id: &str,
        lease_seconds: i64,
        excluded_task_ids: &[TaskId],
    ) -> Result<Option<super::ClaimedWorkRun>, StoreError> {
        if worker_id.trim().is_empty() || !(1..=86_400).contains(&lease_seconds) {
            return Err(StoreError::Work(
                noema_tasks::WorkDomainError::InvalidInput {
                    field: "run.lease",
                    message: "worker and lease duration are invalid".to_string(),
                },
            ));
        }
        if excluded_task_ids.len() > 8 {
            return Err(StoreError::Work(
                noema_tasks::WorkDomainError::InvalidInput {
                    field: "run.excluded_task_ids",
                    message: "at most 8 task ids may be excluded".to_string(),
                },
            ));
        }
        let mut unique_task_ids = HashSet::with_capacity(excluded_task_ids.len());
        if excluded_task_ids
            .iter()
            .any(|task_id| !unique_task_ids.insert(task_id.as_str()))
        {
            return Err(StoreError::Work(
                noema_tasks::WorkDomainError::InvalidInput {
                    field: "run.excluded_task_ids",
                    message: "excluded task ids must be unique".to_string(),
                },
            ));
        }
        let excluded_task_ids_json = serde_json::to_string(
            &excluded_task_ids
                .iter()
                .map(TaskId::as_str)
                .collect::<Vec<_>>(),
        )?;
        let lease_token = allocate_id("lease");
        let claimed = self
            .store
            .with_immediate_transaction_retry(|transaction| {
                recover_expired_runs_tx(transaction, self)?;
                let run_id: Option<String> = transaction
                    .query_row(
                        "SELECT queued.run_id FROM agent_runs AS queued JOIN tasks AS task ON task.task_id = queued.task_id WHERE queued.status = 'queued' AND queued.cancellation_requested = 0 AND queued.task_id NOT IN (SELECT value FROM json_each(?1)) AND queued.task_generation = task.generation AND task.stage_id IN ('stage:personal:queue', 'stage:personal:doing') AND ((queued.run_kind = 'planner' AND queued.contract_id IS NULL AND task.current_contract_id IS NULL) OR (queued.run_kind IN ('executor', 'reviewer') AND queued.contract_id IS NOT NULL AND queued.contract_id = task.current_contract_id)) AND NOT EXISTS (SELECT 1 FROM agent_runs AS active WHERE active.task_id = queued.task_id AND active.status IN ('leased', 'running')) ORDER BY queued.queued_at ASC, queued.run_id ASC LIMIT 1",
                        [excluded_task_ids_json.as_str()],
                        |row| row.get(0),
                    )
                    .optional()?;
                let Some(run_id) = run_id else {
                    return Ok(None);
                };
                let changed = transaction.execute(
                    "UPDATE agent_runs SET status = 'leased', lease_owner = ?2, lease_token = ?3, lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?4 || ' seconds'), heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'queued' AND cancellation_requested = 0 AND task_id NOT IN (SELECT value FROM json_each(?5)) AND task_generation = (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) AND (SELECT stage_id FROM tasks WHERE task_id = agent_runs.task_id) IN ('stage:personal:queue', 'stage:personal:doing') AND ((run_kind = 'planner' AND contract_id IS NULL AND (SELECT current_contract_id FROM tasks WHERE task_id = agent_runs.task_id) IS NULL) OR (run_kind IN ('executor', 'reviewer') AND contract_id IS NOT NULL AND contract_id = (SELECT current_contract_id FROM tasks WHERE task_id = agent_runs.task_id)))",
                    params![
                        run_id,
                        worker_id,
                        lease_token,
                        lease_seconds,
                        excluded_task_ids_json
                    ],
                )?;
                if changed != 1 {
                    return Ok(None);
                }
                let run = load_run_tx(transaction, &run_id)?
                    .ok_or(StoreError::Work(noema_tasks::WorkDomainError::WorkUnavailable))?;
                let mut task = helpers::load_task_state_tx(transaction, &run.task_id)?;
                if task.stage_behavior == WorkflowStageBehavior::Dispatch {
                    let revision = task.revision.checked_add(1).ok_or_else(|| StoreError::Work(noema_tasks::WorkDomainError::InvalidInput { field: "task.revision", message: "revision overflow".to_string() }))?;
                    let changed = transaction.execute(
                        "UPDATE tasks SET stage_id = 'stage:personal:doing', revision = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?3 AND revision = ?4 AND stage_id = 'stage:personal:queue'",
                        params![run.task_id.as_str(), revision, run.task_generation, task.revision],
                    )?;
                    if changed != 1 {
                        return Err(StoreError::Work(noema_tasks::WorkDomainError::StaleRevision));
                    }
                    let from_stage = task.stage_id.clone();
                    task.stage_id = WorkflowStageId::new("stage:personal:doing").map_err(StoreError::Work)?;
                    task.stage_behavior = WorkflowStageBehavior::Active;
                    task.revision = revision;
                    append_work_event_tx(
                        transaction,
                        scope(&task, &run.run_id, "actor:store:run-claim"),
                        WorkEventPayload::task_stage_changed(
                            revision,
                            task.generation,
                            from_stage,
                            task.stage_id.clone(),
                            TaskStageChangeReason::RunStarted,
                        )
                        .map_err(StoreError::Work)?,
                    )?;
                }
                append_work_event_tx(
                    transaction,
                    scope_with_run(&task, &run.run_id, "actor:store:run-claim"),
                    WorkEventPayload::run_lifecycle(
                        WorkEventKind::RunClaimed,
                        run.run_kind,
                        run.task_generation,
                        run.attempt_index,
                        run.review_round,
                    )
                    .map_err(StoreError::Work)?,
                )?;
                Ok(Some(super::ClaimedWorkRun {
                    run,
                    lease_token: lease_token.clone(),
                }))
            })
            .await?;
        Ok(claimed)
    }

    /// Start a leased run and fence the transition by its opaque token.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or stale fence, invalid event scope, or
    /// a failed atomic state transition.
    pub async fn start_work_run(
        &self,
        fence: &super::WorkRunFence,
        actor_id: &str,
        causation_id: Option<&str>,
        correlation_id: &str,
    ) -> Result<noema_tasks::AgentRunRecord, StoreError> {
        fence.validate().map_err(StoreError::Work)?;
        let run = self
            .store
            .with_immediate_transaction_retry(|transaction| {
                let run = load_run_tx(transaction, &fence.run_id)?
                    .ok_or(StoreError::Work(noema_tasks::WorkDomainError::WorkUnavailable))?;
                validate_fence(&run, fence)?;
                if run.status != RunStatus::Leased {
                    return Err(StoreError::Work(noema_tasks::WorkDomainError::InvalidTransition));
                }
                let changed = transaction.execute(
                    "UPDATE agent_runs SET status = 'running', started_at = COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'leased' AND lease_token = ?2 AND cancellation_requested = 0 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AND task_generation = (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) AND (SELECT stage_id FROM tasks WHERE task_id = agent_runs.task_id) = 'stage:personal:doing'",
                    params![fence.run_id, fence.lease_token],
                )?;
                if changed != 1 {
                    return Err(StoreError::Work(noema_tasks::WorkDomainError::RunFenced));
                }
                let task = helpers::load_task_state_tx(transaction, &run.task_id)?;
                append_work_event_tx(
                    transaction,
                    WorkEventScope {
                        actor_id: actor_id.to_string(),
                        causation_id: causation_id.map(ToOwned::to_owned),
                        correlation_id: correlation_id.to_string(),
                        ..scope(&task, &run.run_id, actor_id)
                    },
                    WorkEventPayload::run_lifecycle(
                        WorkEventKind::RunStarted,
                        run.run_kind,
                        run.task_generation,
                        run.attempt_index,
                        run.review_round,
                    )
                    .map_err(StoreError::Work)?,
                )?;
                load_run_tx(transaction, &fence.run_id)?.ok_or(StoreError::Work(noema_tasks::WorkDomainError::WorkUnavailable))
            })
            .await?;
        Ok(run)
    }

    /// Renew a leased/running run and expose the cancellation bit to workers.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lease parameters, an invalid/stale fence,
    /// or a failed atomic renewal.
    pub async fn heartbeat_work_run(
        &self,
        fence: &super::WorkRunFence,
        lease_seconds: i64,
    ) -> Result<AgentRunHeartbeat, StoreError> {
        fence.validate().map_err(StoreError::Work)?;
        if !(1..=86_400).contains(&lease_seconds) {
            return Err(StoreError::Work(
                noema_tasks::WorkDomainError::InvalidInput {
                    field: "run.lease_seconds",
                    message: "lease duration is out of bounds".to_string(),
                },
            ));
        }
        self.store
            .with_immediate_transaction_retry(|transaction| {
                let run = load_run_tx(transaction, &fence.run_id)?.ok_or(StoreError::Work(
                    noema_tasks::WorkDomainError::WorkUnavailable,
                ))?;
                validate_fence(&run, fence)?;
                if run.status != RunStatus::Running {
                    return Err(StoreError::Work(
                        noema_tasks::WorkDomainError::InvalidTransition,
                    ));
                }
                let task = helpers::load_task_state_tx(transaction, &run.task_id)?;
                if task.generation != fence.task_generation {
                    return Err(StoreError::Work(
                        noema_tasks::WorkDomainError::StaleGeneration,
                    ));
                }
                let changed = transaction.execute(
                    "UPDATE agent_runs SET
                        lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?3 || ' seconds'),
                        heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE run_id = ?1 AND lease_token = ?2 AND status = 'running'
                       AND cancellation_requested = 0
                       AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                       AND task_generation = ?4
                       AND ((contract_id IS NULL AND ?5 IS NULL) OR contract_id = ?5)
                       AND EXISTS (
                         SELECT 1 FROM tasks task
                         WHERE task.task_id = agent_runs.task_id AND task.generation = ?4
                       )",
                    params![
                        fence.run_id,
                        fence.lease_token,
                        lease_seconds,
                        i64::try_from(fence.task_generation).map_err(|_| StoreError::Work(
                            noema_tasks::WorkDomainError::InvalidInput {
                                field: "run_fence.task_generation",
                                message: "generation exceeds SQLite range".to_string(),
                            }
                        ))?,
                        fence.contract_id.as_ref().map(ToString::to_string),
                    ],
                )?;
                if changed != 1 {
                    return Err(StoreError::Work(noema_tasks::WorkDomainError::RunFenced));
                }
                let run = load_run_tx(transaction, &fence.run_id)?.ok_or(StoreError::Work(
                    noema_tasks::WorkDomainError::WorkUnavailable,
                ))?;
                append_work_event_tx(
                    transaction,
                    scope_with_run(&task, &run.run_id, "actor:store:run-heartbeat"),
                    WorkEventPayload::run_heartbeat(
                        run.run_kind,
                        run.task_generation,
                        run.provider_call_count,
                        run.tool_call_count,
                        run.active_milliseconds,
                    )
                    .map_err(StoreError::Work)?,
                )?;
                transaction
                    .query_row(
                        "SELECT lease_expires_at, cancellation_requested FROM agent_runs WHERE run_id = ?1",
                        [fence.run_id.as_str()],
                        |row| Ok(AgentRunHeartbeat {
                            lease_expires_at: row.get(0)?,
                            cancellation_requested: row.get::<_, i64>(1)? != 0,
                        }),
                    )
                    .map_err(StoreError::Sqlite)
            })
            .await
    }
}

fn validate_fence(
    run: &noema_tasks::AgentRunRecord,
    fence: &super::WorkRunFence,
) -> Result<(), StoreError> {
    if run.task_generation != fence.task_generation {
        return Err(StoreError::Work(
            noema_tasks::WorkDomainError::StaleGeneration,
        ));
    }
    if run.contract_id != fence.contract_id
        || run.lease_token.as_deref() != Some(fence.lease_token.as_str())
    {
        return Err(StoreError::Work(noema_tasks::WorkDomainError::RunFenced));
    }
    Ok(())
}

fn scope(task: &helpers::TaskState, run_id: &str, actor_id: &str) -> WorkEventScope {
    WorkEventScope {
        workspace_id: task.workspace_id.clone(),
        project_id: task.project_id.clone(),
        task_id: Some(task.task_id.clone()),
        run_id: Some(run_id.to_string()),
        actor_id: actor_id.to_string(),
        causation_id: None,
        correlation_id: format!("correlation:run:{run_id}"),
    }
}

fn scope_with_run(task: &helpers::TaskState, run_id: &str, actor_id: &str) -> WorkEventScope {
    scope(task, run_id, actor_id)
}

fn recover_expired_runs_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
) -> Result<(), StoreError> {
    let ids = {
        let mut statement = transaction.prepare("SELECT run_id FROM agent_runs WHERE status IN ('leased', 'running') AND lease_expires_at IS NOT NULL AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ORDER BY lease_expires_at, run_id")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for run_id in ids {
        transaction.execute_batch("SAVEPOINT recover_expired_run")?;
        let recovered = recover_one_expired_run_tx(transaction, service, &run_id);
        match recovered {
            Ok(()) => transaction.execute_batch("RELEASE SAVEPOINT recover_expired_run")?,
            Err(_) => {
                transaction.execute_batch(
                    "ROLLBACK TO SAVEPOINT recover_expired_run; RELEASE SAVEPOINT recover_expired_run",
                )?;
                park_unrecoverable_expired_run_tx(transaction, &run_id)?;
            }
        }
    }
    Ok(())
}

fn recover_one_expired_run_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    run_id: &str,
) -> Result<(), StoreError> {
    let run = load_run_tx(transaction, run_id)?.ok_or(StoreError::Work(
        noema_tasks::WorkDomainError::WorkUnavailable,
    ))?;
    let task = helpers::load_task_state_tx(transaction, &run.task_id)?;
    if run.cancellation_requested
        || run.task_generation != task.generation
        || task.stage_behavior.is_terminal()
    {
        transaction.execute(
                "UPDATE agent_runs SET status = 'cancelled', cancellation_requested = 1, ended_at = COALESCE(ended_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status IN ('leased', 'running')",
                [run.run_id.as_str()],
            )?;
        let reason = if run.task_generation != task.generation {
            RunCancellationReason::StaleGeneration
        } else {
            RunCancellationReason::LeaseExpired
        };
        append_work_event_tx(
            transaction,
            scope_with_run(&task, &run.run_id, "actor:store:lease-recovery"),
            WorkEventPayload::run_cancelled(
                WorkEventKind::RunCancelled,
                run.run_kind,
                run.task_generation,
                reason,
            )
            .map_err(StoreError::Work)?,
        )?;
        return Ok(());
    }
    let lease_token = run
        .lease_token
        .clone()
        .ok_or(StoreError::Work(noema_tasks::WorkDomainError::RunFenced))?;
    let report = ReportRunFailure {
        fence: WorkRunFence {
            run_id: run.run_id.clone(),
            lease_token,
            task_generation: run.task_generation,
            contract_id: run.contract_id.clone(),
        },
        status: RunStatus::Interrupted,
        error_code: noema_tasks::SafeErrorCode::new("lease_expired").map_err(StoreError::Work)?,
        error_message: Some("lease expired".to_string()),
        retryable: true,
    };
    report_expired_failure_tx(
        transaction,
        service,
        &report,
        "actor:store:lease-recovery",
        Some(run.run_id.as_str()),
        &format!("correlation:run:{}", run.run_id),
    )?;
    Ok(())
}

fn park_unrecoverable_expired_run_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<(), StoreError> {
    let metadata = transaction
        .query_row(
            "SELECT run.run_kind, run.task_generation, run.attempt_index,
                    task.workspace_id, task.project_id, task.task_id
             FROM agent_runs run JOIN tasks task ON task.task_id = run.task_id
             WHERE run.run_id = ?1",
            [run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;
    let changed = transaction.execute(
        "UPDATE agent_runs SET status = 'failed', error_code = 'recovery_invariant',
                error_message = 'expired run recovery failed safely',
                ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE run_id = ?1 AND status IN ('leased', 'running')
           AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        [run_id],
    )?;
    if changed != 1 {
        return Ok(());
    }
    let Some((kind, generation, attempt, workspace_id, project_id, task_id)) = metadata else {
        return Ok(());
    };
    let (Ok(kind), Ok(generation), Ok(attempt), Ok(workspace_id), Ok(task_id), Ok(project_id)) = (
        noema_tasks::RunKind::from_str(&kind),
        u64::try_from(generation),
        u32::try_from(attempt),
        noema_workspaces::WorkspaceId::new(workspace_id),
        noema_tasks::TaskId::new(task_id),
        project_id.map(noema_workspaces::ProjectId::new).transpose(),
    ) else {
        return Ok(());
    };
    append_work_event_tx(
        transaction,
        WorkEventScope {
            workspace_id,
            project_id,
            task_id: Some(task_id),
            run_id: Some(run_id.to_string()),
            actor_id: "actor:store:lease-recovery".to_string(),
            causation_id: Some(run_id.to_string()),
            correlation_id: format!("correlation:run:{run_id}"),
        },
        WorkEventPayload::run_failure(
            WorkEventKind::RunFailed,
            kind,
            generation,
            attempt,
            noema_tasks::SafeErrorCode::new("recovery_invariant").map_err(StoreError::Work)?,
            false,
        )
        .map_err(StoreError::Work)?,
    )?;
    Ok(())
}
