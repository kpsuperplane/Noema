use noema_providers::ReasoningEffort;
use noema_tasks::{
    AutomaticRecoveryInput, AutomaticRecoveryPlan, RunKind, TaskStatus, plan_automatic_recovery,
};
use rusqlite::{OptionalExtension, params};

use super::{
    RUN_COLUMNS,
    events::{append_run_event, append_task_event},
};
use crate::{StoreError, agent_run_rows::run_from_row, ids::allocate_id};

pub(super) fn recover_expired_runs(
    conn: &rusqlite::Connection,
    now: &str,
) -> Result<(), StoreError> {
    let expired_ids = {
        let mut statement = conn.prepare(
            "SELECT run_id FROM agent_runs WHERE status IN ('leased', 'running') AND lease_expires_at IS NOT NULL AND CAST(lease_expires_at AS INTEGER) <= CAST(?1 AS INTEGER) ORDER BY lease_expires_at, run_id",
        )?;
        statement
            .query_map([now], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for run_id in expired_ids {
        let run = conn.query_row(
            &format!("SELECT {RUN_COLUMNS} FROM agent_runs WHERE run_id = ?1"),
            [&run_id],
            run_from_row,
        )?;
        let task_state = conn
            .query_row(
                "SELECT status, latest_run_id FROM tasks WHERE task_id = ?1",
                [&run.task_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let is_latest = task_state
            .as_ref()
            .is_some_and(|(_, latest)| latest.as_deref() == Some(run_id.as_str()));
        conn.execute(
            "UPDATE agent_runs SET status = 'interrupted', lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), error_code = 'lease_expired', error_message = 'worker lease expired before run completion', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status IN ('leased', 'running')",
            [&run_id],
        )?;
        append_run_event(
            conn,
            &run_id,
            "run.interrupted",
            serde_json::json!({"reason": "lease_expired"}),
        )?;
        let Some((task_status, _)) = task_state else {
            continue;
        };
        let task_status =
            task_status
                .parse::<TaskStatus>()
                .map_err(|error| StoreError::InvalidEnum {
                    kind: "task_status",
                    value: error.to_string(),
                })?;
        let recovery_plan = plan_automatic_recovery(AutomaticRecoveryInput {
            is_latest,
            cancellation_requested: run.cancellation_requested,
            task_status,
            run_kind: run.run_kind,
            attempt_index: run.attempt_index,
            retry_count: run.retry_count,
        })
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
        let AutomaticRecoveryPlan::QueueChild {
            attempt_index: child_attempt_index,
            retry_count: child_retry_count,
            task_status: next_task_status,
        } = recovery_plan
        else {
            if recovery_plan == AutomaticRecoveryPlan::NoAction {
                continue;
            }
            let changed = conn.execute(
                "UPDATE tasks SET status = 'failed', terminal_reason = 'automatic infrastructure resume limit reached', error_code = 'automatic_resume_exhausted', error_message = 'run lease expired after three automatic resumptions', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?2 AND status NOT IN ('completed', 'failed', 'cancelled')",
                params![run.task_id, run_id],
            )?;
            if changed == 1 {
                append_task_event(
                    conn,
                    &run.task_id,
                    "task.failed",
                    serde_json::json!({
                        "run_id": run_id,
                        "error_code": "automatic_resume_exhausted",
                    }),
                )?;
            }
            continue;
        };
        let child_run_id = allocate_id("run");
        conn.execute(
            r#"INSERT INTO agent_runs (
                run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                parent_run_id, triggering_submission_id, triggering_review_id,
                provider_kind, provider_account_id, selection_mode, model_profile,
                reasoning_effort, selection_source, max_provider_continuations,
                max_tool_calls, max_active_minutes, progress_audit_interval,
                status, priority, retry_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, 'queued', ?20, ?21)"#,
            params![
                child_run_id,
                run.task_id,
                run.run_kind.as_str(),
                run.agent_id,
                child_attempt_index,
                run.revision_index,
                run.run_id,
                run.triggering_submission_id,
                run.triggering_review_id,
                run.model.provider_kind,
                run.model.provider_account_id,
                run.model.selection_mode.as_str(),
                run.model.model_profile,
                run.model
                    .reasoning_effort
                    .map(ReasoningEffort::as_persistence_str),
                run.model.selection_source,
                run.execution_policy.max_provider_continuations,
                run.execution_policy.max_tool_calls,
                run.execution_policy.max_active_minutes,
                run.execution_policy.progress_audit_interval,
                run.priority,
                child_retry_count,
            ],
        )?;
        append_run_event(
            conn,
            &child_run_id,
            "run.queued",
            serde_json::json!({
                "automatic_resume_of_run_id": run_id,
                "retry_count": child_retry_count,
            }),
        )?;
        let changed = conn.execute(
            "UPDATE tasks SET status = ?2, latest_run_id = ?3, terminal_reason = NULL, error_code = NULL, error_message = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?4 AND status NOT IN ('completed', 'failed', 'cancelled')",
            params![run.task_id, next_task_status.as_str(), child_run_id, run_id],
        )?;
        if changed != 1 {
            return Err(StoreError::InvariantViolation {
                message: format!("task changed while recovering expired run: {}", run.task_id),
            });
        }
        append_task_event(
            conn,
            &run.task_id,
            "task.automatically_resumed",
            serde_json::json!({
                "interrupted_run_id": run_id,
                "new_run_id": child_run_id,
                "retry_count": child_retry_count,
            }),
        )?;
    }
    Ok(())
}

pub(super) fn recover_interrupted_runs(conn: &rusqlite::Connection) -> Result<(), StoreError> {
    let interrupted = {
        let mut statement = conn.prepare(
            "SELECT r.run_id, r.task_id, r.run_kind, r.attempt_index, r.retry_count, r.cancellation_requested, t.status FROM agent_runs r JOIN tasks t ON t.task_id = r.task_id AND t.latest_run_id = r.run_id WHERE r.status = 'interrupted' AND r.cancellation_requested = 0 AND t.status NOT IN ('completed', 'failed', 'cancelled') ORDER BY r.updated_at, r.run_id",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (
        run_id,
        task_id,
        run_kind,
        attempt_index,
        retry_count,
        cancellation_requested,
        task_status,
    ) in interrupted
    {
        let run_kind = run_kind
            .parse::<RunKind>()
            .map_err(|error| StoreError::InvalidEnum {
                kind: "run_kind",
                value: error.to_string(),
            })?;
        let task_status =
            task_status
                .parse::<TaskStatus>()
                .map_err(|error| StoreError::InvalidEnum {
                    kind: "task_status",
                    value: error.to_string(),
                })?;
        let recovery_plan = plan_automatic_recovery(AutomaticRecoveryInput {
            is_latest: true,
            cancellation_requested,
            task_status,
            run_kind,
            attempt_index,
            retry_count,
        })
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
        let AutomaticRecoveryPlan::QueueChild {
            attempt_index: child_attempt_index,
            retry_count: child_retry_count,
            task_status: next_task_status,
        } = recovery_plan
        else {
            if recovery_plan == AutomaticRecoveryPlan::NoAction {
                continue;
            }
            let changed = conn.execute(
                "UPDATE tasks SET status = 'failed', terminal_reason = 'automatic infrastructure resume limit reached', error_code = 'automatic_resume_exhausted', error_message = 'run was interrupted after three automatic resumptions', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?2 AND status NOT IN ('completed', 'failed', 'cancelled')",
                params![task_id, run_id],
            )?;
            if changed == 1 {
                append_task_event(
                    conn,
                    &task_id,
                    "task.failed",
                    serde_json::json!({
                        "run_id": run_id,
                        "error_code": "automatic_resume_exhausted",
                    }),
                )?;
            }
            continue;
        };
        let child_run_id = allocate_id("run");
        let inserted = conn.execute(
            r#"INSERT INTO agent_runs (
                run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                parent_run_id, triggering_submission_id, triggering_review_id,
                provider_kind, provider_account_id, selection_mode, model_profile,
                reasoning_effort, selection_source, max_provider_continuations,
                max_tool_calls, max_active_minutes, progress_audit_interval,
                status, priority, retry_count
            ) SELECT ?2, task_id, run_kind, agent_id, ?3, revision_index,
                run_id, triggering_submission_id, triggering_review_id,
                provider_kind, provider_account_id, selection_mode, model_profile,
                reasoning_effort, selection_source, max_provider_continuations,
                max_tool_calls, max_active_minutes, progress_audit_interval,
                'queued', priority, ?4
              FROM agent_runs WHERE run_id = ?1 AND status = 'interrupted'
                AND cancellation_requested = 0"#,
            params![run_id, child_run_id, child_attempt_index, child_retry_count],
        )?;
        if inserted != 1 {
            continue;
        }
        append_run_event(
            conn,
            &child_run_id,
            "run.queued",
            serde_json::json!({
                "automatic_resume_of_run_id": run_id,
                "retry_count": child_retry_count,
            }),
        )?;
        let changed = conn.execute(
            "UPDATE tasks SET status = ?3, latest_run_id = ?2, terminal_reason = NULL, error_code = NULL, error_message = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?4 AND status NOT IN ('completed', 'failed', 'cancelled')",
            params![task_id, child_run_id, next_task_status.as_str(), run_id],
        )?;
        if changed != 1 {
            return Err(StoreError::InvariantViolation {
                message: format!("task changed while recovering interrupted run: {task_id}"),
            });
        }
        append_task_event(
            conn,
            &task_id,
            "task.automatically_resumed",
            serde_json::json!({
                "interrupted_run_id": run_id,
                "new_run_id": child_run_id,
                "retry_count": child_retry_count,
            }),
        )?;
    }
    Ok(())
}
