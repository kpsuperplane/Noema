//! Transactional deadline processing for one-time and recurring Work.

use std::str::FromStr;

use noema_tasks::{
    CommandMeta, MissedRunPolicy, OverlapPolicy, PERSONAL_CANCELLED_STAGE_ID,
    PERSONAL_INBOX_STAGE_ID, RecurrenceOccurrenceResolution, RecurrenceOccurrenceTrigger, TaskId,
    TaskSourceKind, WorkEventPayload, WorkflowStageId,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{
    NoemaStore, StoreError, ids::allocate_id, task_files::allocate_task_directory_tx,
    work_events::append_work_event_tx,
};

use super::{WorkCommandService, helpers, tasks};

const RUNTIME_ACTOR: &str = "actor:runtime:scheduler";

impl NoemaStore {
    /// Return the earliest one-time, recurrence, or releasable coalesced deadline.
    ///
    /// # Errors
    /// Returns a store error when the deadline projection cannot be read.
    pub async fn next_work_schedule_deadline(&self) -> Result<Option<i64>, StoreError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT MIN(value) FROM (
                   SELECT task.scheduled_for AS value FROM tasks task
                   JOIN workflow_stages stage
                     ON stage.workflow_id = task.workflow_id AND stage.stage_id = task.stage_id
                   WHERE task.scheduled_for IS NOT NULL AND task.queued_at IS NULL
                     AND task.completed_at IS NULL AND task.cancelled_at IS NULL
                     AND stage.system_behavior = 'intake'
                   UNION ALL
                   SELECT COALESCE(recurrence.pending_coalesced_at, recurrence.next_run_at) AS value
                   FROM task_recurrences recurrence WHERE recurrence.lifecycle = 'active'
                     AND (recurrence.pending_coalesced_at IS NULL OR NOT EXISTS (
                       SELECT 1 FROM tasks task JOIN workflow_stages stage
                       ON stage.workflow_id = task.workflow_id AND stage.stage_id = task.stage_id
                       WHERE task.recurrence_id = recurrence.recurrence_id
                         AND stage.system_behavior NOT IN ('terminal_success', 'terminal_cancelled')
                     ))
                 )",
                    [],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Load one recurring template by exact identity.
    ///
    /// # Errors
    /// Returns a store error when the recurrence cannot be decoded or read.
    pub async fn get_task_recurrence(
        &self,
        recurrence_id: &noema_tasks::TaskRecurrenceId,
    ) -> Result<Option<noema_tasks::TaskRecurrenceRecord>, StoreError> {
        let recurrence_id = recurrence_id.clone();
        self.with_connection(move |connection| {
            connection.query_row(
                "SELECT recurrence_id, workspace_id, project_id, title, description_markdown, authorization_context_json, starts_at, cron_expression, time_zone, missed_run_policy, overlap_policy, lifecycle, revision, next_run_at, pending_coalesced_at, created_at, updated_at FROM task_recurrences WHERE recurrence_id = ?1",
                [recurrence_id.as_str()],
                |row| Ok(noema_tasks::TaskRecurrenceRecord {
                    recurrence_id: noema_tasks::TaskRecurrenceId::new(row.get::<_, String>(0)?).map_err(sql_conversion)?,
                    workspace_id: noema_workspaces::WorkspaceId::new(row.get::<_, String>(1)?).map_err(sql_conversion)?,
                    project_id: row.get::<_, Option<String>>(2)?.map(noema_workspaces::ProjectId::new).transpose().map_err(sql_conversion)?,
                    title: row.get(3)?, description_markdown: row.get(4)?,
                    authorization_context: serde_json::from_str(&row.get::<_, String>(5)?).map_err(sql_conversion)?,
                    starts_at: row.get(6)?, cron_expression: row.get(7)?, time_zone: row.get(8)?,
                    missed_run_policy: noema_tasks::MissedRunPolicy::from_str(&row.get::<_, String>(9)?).map_err(sql_conversion)?,
                    overlap_policy: noema_tasks::OverlapPolicy::from_str(&row.get::<_, String>(10)?).map_err(sql_conversion)?,
                    lifecycle: noema_tasks::RecurrenceLifecycle::from_str(&row.get::<_, String>(11)?).map_err(sql_conversion)?,
                    revision: u64::try_from(row.get::<_, i64>(12)?).map_err(sql_conversion)?,
                    next_run_at: row.get(13)?, pending_coalesced_at: row.get(14)?,
                    created_at: row.get(15)?, updated_at: row.get(16)?,
                }),
            ).optional().map_err(StoreError::Sqlite)
        }).await
    }

    /// List newest recurrence slot records with a bounded caller limit.
    ///
    /// # Errors
    /// Returns a store error when occurrence history cannot be decoded or read.
    pub async fn list_task_recurrence_occurrences(
        &self,
        recurrence_id: &noema_tasks::TaskRecurrenceId,
        first: usize,
    ) -> Result<Vec<noema_tasks::RecurrenceOccurrenceRecord>, StoreError> {
        let recurrence_id = recurrence_id.clone();
        self.with_connection(move |connection| {
            let mut statement = connection.prepare(
                "SELECT recurrence_id, recurrence_revision, scheduled_for, local_slot, trigger_kind, resolution, task_id, created_at FROM task_recurrence_occurrences WHERE recurrence_id = ?1 ORDER BY created_at DESC, occurrence_id DESC LIMIT ?2",
            )?;
            statement.query_map(params![recurrence_id.as_str(), i64::try_from(first).unwrap_or(i64::MAX)], |row| {
                Ok(noema_tasks::RecurrenceOccurrenceRecord {
                    recurrence_id: noema_tasks::TaskRecurrenceId::new(row.get::<_, String>(0)?).map_err(sql_conversion)?,
                    recurrence_revision: u64::try_from(row.get::<_, i64>(1)?).map_err(sql_conversion)?,
                    scheduled_for: row.get(2)?, local_slot: row.get(3)?,
                    trigger: noema_tasks::RecurrenceOccurrenceTrigger::from_str(&row.get::<_, String>(4)?).map_err(sql_conversion)?,
                    resolution: noema_tasks::RecurrenceOccurrenceResolution::from_str(&row.get::<_, String>(5)?).map_err(sql_conversion)?,
                    task_id: row.get::<_, Option<String>>(6)?.map(TaskId::new).transpose().map_err(sql_conversion)?,
                    created_at: row.get(7)?,
                })
            })?.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        }).await
    }
}

fn sql_conversion(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

impl WorkCommandService {
    /// Process every schedule due at `now` in one immediate transaction.
    /// `recovering` is true only for startup recovery and activates missed-run policy.
    ///
    /// # Errors
    /// Returns a store error when any due transition cannot commit atomically.
    pub async fn process_due_work_schedules(
        &self,
        now: i64,
        recovering: bool,
    ) -> Result<Vec<TaskId>, StoreError> {
        self.store
            .with_immediate_transaction_retry(|transaction| {
                let mut changed = Vec::new();
                loop {
                    if let Some(task_id) = next_due_task_tx(transaction, now)? {
                        process_due_task_tx(self, transaction, &task_id, now, recovering)?;
                        changed.push(task_id);
                        continue;
                    }
                    if let Some(recurrence_id) = next_due_recurrence_tx(transaction, now)? {
                        if let Some(task_id) = process_due_recurrence_tx(
                            self,
                            transaction,
                            &recurrence_id,
                            now,
                            recovering,
                        )? {
                            changed.push(task_id);
                        }
                        continue;
                    }
                    break;
                }
                Ok(changed)
            })
            .await
    }
}

fn runtime_meta(id: &str) -> CommandMeta {
    CommandMeta {
        actor_id: RUNTIME_ACTOR.to_string(),
        causation_id: None,
        correlation_id: format!("correlation:schedule:{id}"),
        idempotency_key: None,
    }
}

fn next_due_task_tx(transaction: &Transaction<'_>, now: i64) -> Result<Option<TaskId>, StoreError> {
    let id = transaction.query_row(
        "SELECT task.task_id FROM tasks task
         JOIN workflow_stages stage ON stage.workflow_id = task.workflow_id AND stage.stage_id = task.stage_id
         WHERE task.scheduled_for <= ?1 AND task.queued_at IS NULL
           AND task.completed_at IS NULL AND task.cancelled_at IS NULL
           AND stage.system_behavior = 'intake'
         ORDER BY task.scheduled_for, task.task_id LIMIT 1",
        [now],
        |row| row.get::<_, String>(0),
    ).optional()?;
    id.map(TaskId::new).transpose().map_err(StoreError::Work)
}

fn process_due_task_tx(
    service: &WorkCommandService,
    transaction: &Transaction<'_>,
    task_id: &TaskId,
    now: i64,
    recovering: bool,
) -> Result<(), StoreError> {
    let (scheduled_for, missed): (i64, String) = transaction.query_row(
        "SELECT scheduled_for, missed_run_policy FROM tasks WHERE task_id = ?1",
        [task_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let task = helpers::load_task_state_tx(transaction, task_id)?;
    let missed: MissedRunPolicy = missed.parse().map_err(StoreError::Work)?;
    if recovering && scheduled_for < now && missed == MissedRunPolicy::Skip {
        cancel_missed_task_tx(transaction, &task)?;
    } else {
        tasks::queue_task_tx(
            transaction,
            service.provider_registry.as_ref(),
            task,
            &runtime_meta(task_id.as_str()),
            true,
        )?;
    }
    Ok(())
}

fn cancel_missed_task_tx(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
) -> Result<(), StoreError> {
    let revision = helpers::increment(task.revision, "task.revision")?;
    transaction.execute(
        "UPDATE tasks SET stage_id = ?2, cancelled_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?4 AND generation = ?5",
        params![task.task_id.as_str(), PERSONAL_CANCELLED_STAGE_ID, revision, task.revision, task.generation],
    )?;
    let meta = runtime_meta(task.task_id.as_str());
    let cancelled = WorkEventPayload::task_cancelled(revision, task.generation, true)
        .map_err(StoreError::Work)?;
    append_work_event_tx(
        transaction,
        helpers::event_context(&meta).task_scope(task, None),
        cancelled,
    )?;
    let changed = WorkEventPayload::task_stage_changed(
        revision,
        task.generation,
        task.stage_id.clone(),
        WorkflowStageId::new(PERSONAL_CANCELLED_STAGE_ID).map_err(StoreError::Work)?,
        noema_tasks::TaskStageChangeReason::Cancelled,
    )
    .map_err(StoreError::Work)?;
    append_work_event_tx(
        transaction,
        helpers::event_context(&meta).task_scope(task, None),
        changed,
    )?;
    Ok(())
}

struct DueRecurrence {
    recurrence_id: String,
    workspace_id: String,
    project_id: Option<String>,
    title: String,
    description: String,
    authorization: String,
    executor_agent_id: String,
    cwd_override: Option<String>,
    cron: String,
    time_zone: String,
    missed: MissedRunPolicy,
    overlap: OverlapPolicy,
    revision: u64,
    next_run_at: i64,
    pending: Option<i64>,
}

fn next_due_recurrence_tx(
    transaction: &Transaction<'_>,
    now: i64,
) -> Result<Option<String>, StoreError> {
    Ok(transaction
        .query_row(
            "SELECT recurrence.recurrence_id FROM task_recurrences recurrence
         WHERE recurrence.lifecycle = 'active' AND (
           (recurrence.pending_coalesced_at IS NULL AND recurrence.next_run_at <= ?1) OR
           (recurrence.pending_coalesced_at <= ?1 AND NOT EXISTS (
             SELECT 1 FROM tasks task JOIN workflow_stages stage
             ON stage.workflow_id = task.workflow_id AND stage.stage_id = task.stage_id
             WHERE task.recurrence_id = recurrence.recurrence_id
               AND stage.system_behavior NOT IN ('terminal_success', 'terminal_cancelled')
           ))
         )
         ORDER BY COALESCE(pending_coalesced_at, next_run_at), recurrence_id LIMIT 1",
            [now],
            |row| row.get(0),
        )
        .optional()?)
}

fn load_due_recurrence_tx(
    transaction: &Transaction<'_>,
    recurrence_id: &str,
) -> Result<DueRecurrence, StoreError> {
    let row = transaction.query_row(
        "SELECT workspace_id, project_id, title, description_markdown, authorization_context_json, executor_agent_id, cwd_override, cron_expression, time_zone, missed_run_policy, overlap_policy, revision, next_run_at, pending_coalesced_at FROM task_recurrences WHERE recurrence_id = ?1",
        [recurrence_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, Option<String>>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?, row.get::<_, String>(9)?, row.get::<_, String>(10)?, row.get::<_, i64>(11)?, row.get::<_, i64>(12)?, row.get::<_, Option<i64>>(13)?)),
    )?;
    Ok(DueRecurrence {
        recurrence_id: recurrence_id.to_string(),
        workspace_id: row.0,
        project_id: row.1,
        title: row.2,
        description: row.3,
        authorization: row.4,
        executor_agent_id: row.5,
        cwd_override: row.6,
        cron: row.7,
        time_zone: row.8,
        missed: row.9.parse().map_err(StoreError::Work)?,
        overlap: row.10.parse().map_err(StoreError::Work)?,
        revision: u64::try_from(row.11).map_err(|_| StoreError::InvariantViolation {
            message: "recurrence revision overflow".to_string(),
        })?,
        next_run_at: row.12,
        pending: row.13,
    })
}

fn process_due_recurrence_tx(
    service: &WorkCommandService,
    transaction: &Transaction<'_>,
    recurrence_id: &str,
    now: i64,
    recovering: bool,
) -> Result<Option<TaskId>, StoreError> {
    let recurrence = load_due_recurrence_tx(transaction, recurrence_id)?;
    let active = recurrence_has_nonterminal_task_tx(transaction, recurrence_id)?;
    if let Some(pending) = recurrence.pending {
        if active {
            return Ok(None);
        }
        let (task_id, _) = materialize_occurrence_tx(
            service,
            transaction,
            &recurrence,
            pending,
            true,
            RecurrenceOccurrenceTrigger::Scheduled,
            &runtime_meta(recurrence_id),
        )?;
        transaction.execute(
            "UPDATE task_recurrences SET pending_coalesced_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE recurrence_id = ?1 AND revision = ?2",
            params![recurrence_id, recurrence.revision],
        )?;
        return Ok(Some(task_id));
    }
    let due = recurrence.next_run_at;
    let missed = recovering && due < now;
    let resolution = if missed && recurrence.missed == MissedRunPolicy::Skip {
        RecurrenceOccurrenceResolution::Skipped
    } else if active {
        match recurrence.overlap {
            OverlapPolicy::Skip => RecurrenceOccurrenceResolution::Skipped,
            OverlapPolicy::QueueOne => RecurrenceOccurrenceResolution::Coalesced,
            OverlapPolicy::Allow => RecurrenceOccurrenceResolution::Materialized,
        }
    } else {
        RecurrenceOccurrenceResolution::Materialized
    };
    let next_lower_bound = if missed {
        now.saturating_add(1)
    } else {
        due.saturating_add(1)
    };
    let local_slot =
        noema_tasks::recurrence_local_slot(due, &recurrence.time_zone).map_err(StoreError::Work)?;
    let duplicate_local_slot = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_recurrence_occurrences WHERE recurrence_id = ?1 AND local_slot = ?2)",
        params![recurrence_id, local_slot],
        |row| row.get::<_, bool>(0),
    )?;
    let next = noema_tasks::next_recurrence_at_or_after(
        &recurrence.cron,
        &recurrence.time_zone,
        next_lower_bound,
    )
    .map_err(StoreError::Work)?;
    let resolution = if duplicate_local_slot {
        RecurrenceOccurrenceResolution::Skipped
    } else {
        resolution
    };
    let task_id = match resolution {
        RecurrenceOccurrenceResolution::Materialized => Some(
            materialize_occurrence_tx(
                service,
                transaction,
                &recurrence,
                due,
                false,
                RecurrenceOccurrenceTrigger::Scheduled,
                &runtime_meta(recurrence_id),
            )?
            .0,
        ),
        RecurrenceOccurrenceResolution::Skipped | RecurrenceOccurrenceResolution::Coalesced => {
            record_unmaterialized_occurrence_tx(
                transaction,
                recurrence_id,
                &recurrence,
                due,
                resolution,
            )?;
            None
        }
    };
    transaction.execute(
        "UPDATE task_recurrences SET next_run_at = ?2, pending_coalesced_at = CASE WHEN ?3 = 'coalesced' THEN COALESCE(pending_coalesced_at, ?4) ELSE pending_coalesced_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE recurrence_id = ?1 AND revision = ?5",
        params![recurrence_id, next, resolution.as_str(), due, recurrence.revision],
    )?;
    Ok(task_id)
}

pub(crate) fn recurrence_has_nonterminal_task_tx(
    transaction: &Transaction<'_>,
    recurrence_id: &str,
) -> Result<bool, StoreError> {
    Ok(transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks task JOIN workflow_stages stage ON stage.workflow_id = task.workflow_id AND stage.stage_id = task.stage_id WHERE task.recurrence_id = ?1 AND stage.system_behavior NOT IN ('terminal_success', 'terminal_cancelled'))",
        [recurrence_id], |row| row.get(0),
    )?)
}

fn record_unmaterialized_occurrence_tx(
    transaction: &Transaction<'_>,
    recurrence_id: &str,
    recurrence: &DueRecurrence,
    due: i64,
    resolution: RecurrenceOccurrenceResolution,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT OR IGNORE INTO task_recurrence_occurrences (occurrence_id, recurrence_id, recurrence_revision, scheduled_for, local_slot, resolution) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![allocate_id("occurrence"), recurrence_id, recurrence.revision, due,
            noema_tasks::recurrence_local_slot(due, &recurrence.time_zone).map_err(StoreError::Work)?, resolution.as_str()],
    )?;
    Ok(())
}

fn materialize_occurrence_tx(
    service: &WorkCommandService,
    transaction: &Transaction<'_>,
    recurrence: &DueRecurrence,
    scheduled_for: i64,
    release_coalesced: bool,
    trigger: RecurrenceOccurrenceTrigger,
    meta: &CommandMeta,
) -> Result<(TaskId, helpers::CommandTransactionOutcome), StoreError> {
    let recurrence_id = recurrence.recurrence_id.as_str();
    let occurrence_id = allocate_id("occurrence");
    let local_slot = match trigger {
        RecurrenceOccurrenceTrigger::Scheduled => {
            noema_tasks::recurrence_local_slot(scheduled_for, &recurrence.time_zone)
                .map_err(StoreError::Work)?
        }
        RecurrenceOccurrenceTrigger::Manual => format!("manual:{occurrence_id}"),
    };
    if !release_coalesced && trigger == RecurrenceOccurrenceTrigger::Scheduled {
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_recurrence_occurrences WHERE recurrence_id = ?1 AND local_slot = ?2)",
            params![recurrence_id, local_slot], |row| row.get::<_, bool>(0),
        )?;
        if exists {
            return Err(StoreError::InvariantViolation {
                message: "recurrence slot already resolved".to_string(),
            });
        }
    }
    let task_id = TaskId::new(allocate_id("task")).map_err(StoreError::Work)?;
    let workspace_id =
        WorkspaceId::new(recurrence.workspace_id.clone()).map_err(StoreError::Workspace)?;
    let project_id = recurrence
        .project_id
        .as_ref()
        .map(|value| ProjectId::new(value.clone()))
        .transpose()
        .map_err(StoreError::Workspace)?;
    let task_directory = allocate_task_directory_tx(
        transaction,
        &workspace_id,
        project_id.as_ref(),
        &recurrence.title,
    )?;
    transaction.execute(
        "INSERT INTO tasks (task_id, workspace_id, project_id, workflow_id, stage_id, title, description_markdown, executor_agent_id, cwd_override, task_directory, authorization_context_json, source_kind, created_by_actor_id, scheduled_for, schedule_time_zone, missed_run_policy, recurrence_id, recurrence_revision, recurrence_scheduled_for) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![task_id.as_str(), recurrence.workspace_id, recurrence.project_id,
            noema_tasks::PERSONAL_WORKFLOW_ID, PERSONAL_INBOX_STAGE_ID, recurrence.title,
            recurrence.description, recurrence.executor_agent_id, recurrence.cwd_override,
            task_directory, recurrence.authorization, TaskSourceKind::System.as_str(), meta.actor_id,
            scheduled_for, recurrence.time_zone, recurrence.missed.as_str(), recurrence_id,
            recurrence.revision, scheduled_for],
    )?;
    if release_coalesced {
        transaction.execute(
            "UPDATE task_recurrence_occurrences SET resolution = 'materialized', task_id = ?3 WHERE recurrence_id = ?1 AND local_slot = ?2 AND resolution = 'coalesced'",
            params![recurrence_id, local_slot, task_id.as_str()],
        )?;
    } else {
        transaction.execute(
            "INSERT INTO task_recurrence_occurrences (occurrence_id, recurrence_id, recurrence_revision, scheduled_for, local_slot, trigger_kind, resolution, task_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'materialized', ?7)",
            params![occurrence_id, recurrence_id, recurrence.revision, scheduled_for, local_slot, trigger.as_str(), task_id.as_str()],
        )?;
    }
    let task = helpers::load_task_state_tx(transaction, &task_id)?;
    let captured = WorkEventPayload::task_captured(
        1,
        1,
        WorkflowStageId::new(PERSONAL_INBOX_STAGE_ID).map_err(StoreError::Work)?,
        TaskSourceKind::System,
    )
    .map_err(StoreError::Work)?;
    append_work_event_tx(
        transaction,
        helpers::event_context(meta).task_scope(&task, None),
        captured,
    )?;
    let queued = tasks::queue_task_tx(
        transaction,
        service.provider_registry.as_ref(),
        task,
        meta,
        true,
    )?;
    Ok((task_id, queued))
}

pub(crate) fn materialize_manual_occurrence_tx(
    service: &WorkCommandService,
    transaction: &Transaction<'_>,
    recurrence_id: &str,
    now: i64,
    meta: &CommandMeta,
) -> Result<helpers::CommandTransactionOutcome, StoreError> {
    let recurrence = load_due_recurrence_tx(transaction, recurrence_id)?;
    materialize_occurrence_tx(
        service,
        transaction,
        &recurrence,
        now,
        false,
        RecurrenceOccurrenceTrigger::Manual,
        meta,
    )
    .map(|(_, outcome)| outcome)
}
