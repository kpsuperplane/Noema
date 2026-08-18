//! Live Work-run authority checks for governed external effects.

use rusqlite::{Transaction, params};

use crate::{StoreError, WorkRunFence};

pub(crate) fn require_origin_execution_live_tx(
    transaction: &Transaction<'_>,
    action_id: &str,
    revision: u64,
    run_fence: Option<&WorkRunFence>,
) -> Result<(), StoreError> {
    let origin = transaction.query_row(
        "SELECT task_id, run_id FROM governed_actions WHERE action_id = ?1 AND revision = ?2",
        params![action_id, revision],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        },
    )?;
    let (Some(task_id), Some(run_id)) = origin else {
        if run_fence.is_some() {
            return Err(conflict("foreground action received a task run fence"));
        }
        return Ok(());
    };
    let count = if let Some(fence) = run_fence {
        fence.validate().map_err(StoreError::Work)?;
        if fence.run_id != run_id {
            return Err(conflict("task action run fence does not match its origin"));
        }
        transaction.query_row(
            r#"
            SELECT COUNT(*)
            FROM agent_runs runs JOIN tasks ON tasks.task_id = runs.task_id
            WHERE runs.run_id = ?1 AND runs.task_id = ?2 AND runs.status = 'running'
              AND runs.lease_token = ?3
              AND runs.lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
              AND runs.task_generation = ?4 AND tasks.generation = ?4
              AND runs.cancellation_requested = 0
              AND tasks.cancelled_at IS NULL AND tasks.completed_at IS NULL
            "#,
            params![run_id, task_id, fence.lease_token, fence.task_generation,],
            |row| row.get::<_, i64>(0),
        )?
    } else {
        transaction.query_row(
            r#"
            SELECT COUNT(*)
            FROM agent_runs runs JOIN tasks ON tasks.task_id = runs.task_id
            WHERE runs.run_id = ?1 AND runs.task_id = ?2
              AND runs.status = 'waiting_for_approval'
              AND runs.task_generation = tasks.generation
              AND runs.cancellation_requested = 0
              AND tasks.cancelled_at IS NULL AND tasks.completed_at IS NULL
            "#,
            params![run_id, task_id],
            |row| row.get::<_, i64>(0),
        )?
    };
    if count != 1 {
        return Err(conflict("task action lost its live run authority"));
    }
    Ok(())
}

fn conflict(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}
