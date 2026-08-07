//! Human decisions and attention reads for governed actions.

use rusqlite::{OptionalExtension, params};

use crate::{
    GovernedActionRecord, GovernedActionState, NoemaStore, StoreError, WorkRunFence,
    governed_actions::{action_from_tx, insert_event},
};

/// One explicit human decision for an immutable action revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedActionDecision {
    /// Permit the saved action to execute once after live revalidation.
    Approve,
    /// Decline the saved action without execution.
    Decline,
}

impl NoemaStore {
    /// List pending actions owned by one human, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn list_pending_governed_actions(
        &self,
        owner_human_id: &str,
        conversation_id: Option<&str>,
        task_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GovernedActionRecord>, StoreError> {
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(100);
        self.with_connection(|connection| {
            let ids = connection
                .prepare(
                    r#"
                    SELECT action_id, revision
                    FROM governed_actions
                    WHERE owner_human_id = ?1 AND state = 'awaiting_approval'
                      AND (?2 IS NULL OR conversation_id = ?2)
                      AND (?3 IS NULL OR task_id = ?3)
                    ORDER BY created_at DESC, action_id DESC
                    LIMIT ?4
                    "#,
                )?
                .query_map(
                    params![owner_human_id, conversation_id, task_id, limit],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            ids.into_iter()
                .map(|(action_id, revision)| {
                    let revision =
                        u64::try_from(revision).map_err(|_| action_conflict("invalid revision"))?;
                    action_from_tx(connection, &action_id, revision)?.ok_or_else(|| {
                        action_conflict("pending action disappeared during its read")
                    })
                })
                .collect()
        })
        .await
    }

    /// Apply one expected-revision human decision.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the action is stale, belongs to another human,
    /// was already resolved, or the transaction fails.
    pub async fn decide_governed_action(
        &self,
        action_id: &str,
        revision: u64,
        human_id: &str,
        decision: GovernedActionDecision,
    ) -> Result<GovernedActionRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let (approval_state, owner, action_state) = transaction
                .query_row(
                    r#"
                    SELECT approvals.state, actions.owner_human_id, actions.state
                    FROM governed_actions actions
                    JOIN governed_action_approvals approvals
                      ON approvals.action_id = actions.action_id
                     AND approvals.action_revision = actions.revision
                    WHERE actions.action_id = ?1 AND actions.revision = ?2
                    "#,
                    params![action_id, revision],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| action_conflict("approval was not found"))?;
            if owner != human_id || approval_state != "pending" || action_state != "awaiting_approval" {
                return Err(action_conflict("approval is stale or belongs to another human"));
            }
            let (approval_state, action_state, event_kind, terminal) = match decision {
                GovernedActionDecision::Approve => {
                    ("approved", GovernedActionState::Executable, "approved", false)
                }
                GovernedActionDecision::Decline => {
                    ("declined", GovernedActionState::Declined, "declined", true)
                }
            };
            transaction.execute(
                r#"
                UPDATE governed_action_approvals
                SET state = ?3, decided_by_human_id = ?4,
                    decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE action_id = ?1 AND action_revision = ?2 AND state = 'pending'
                "#,
                params![action_id, revision, approval_state, human_id],
            )?;
            transaction.execute(
                r#"
                UPDATE governed_actions
                SET state = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    completed_at = CASE WHEN ?4 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE NULL END
                WHERE action_id = ?1 AND revision = ?2 AND state = 'awaiting_approval'
                "#,
                params![action_id, revision, action_state.as_str(), terminal],
            )?;
            insert_event(
                transaction,
                action_id,
                revision,
                event_kind,
                human_id,
                &serde_json::json!({}),
            )?;
            action_from_tx(transaction, action_id, revision)?.ok_or_else(|| {
                action_conflict("decided action could not be reloaded")
            })
        })
        .await
    }

    /// Supersede an approved or pending action that failed live revalidation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the action is stale or cannot be updated.
    pub async fn supersede_governed_action(
        &self,
        action_id: &str,
        revision: u64,
        reason: &str,
    ) -> Result<GovernedActionRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                r#"
                UPDATE governed_actions
                SET state = 'superseded', failure_code = ?3,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE action_id = ?1 AND revision = ?2
                  AND state IN ('awaiting_approval', 'executable')
                "#,
                params![action_id, revision, reason],
            )?;
            if changed != 1 {
                return Err(action_conflict("action is no longer executable"));
            }
            transaction.execute(
                "UPDATE governed_action_approvals SET state = 'superseded' WHERE action_id = ?1 AND action_revision = ?2 AND state IN ('pending', 'approved')",
                params![action_id, revision],
            )?;
            insert_event(
                transaction,
                action_id,
                revision,
                "superseded",
                "system:action_gateway",
                &serde_json::json!({"reason": reason}),
            )?;
            action_from_tx(transaction, action_id, revision)?.ok_or_else(|| {
                action_conflict("superseded action could not be reloaded")
            })
        })
        .await
    }
}

fn action_conflict(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}

pub(crate) fn mark_origin_run_waiting_tx(
    transaction: &rusqlite::Transaction<'_>,
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
            return Err(action_conflict(
                "foreground action received a task run fence",
            ));
        }
        return Ok(());
    };
    mark_run_waiting_for_intervention_tx(transaction, &task_id, &run_id, run_fence)
}

pub(crate) fn mark_run_waiting_for_intervention_tx(
    transaction: &rusqlite::Transaction<'_>,
    task_id: &str,
    run_id: &str,
    run_fence: Option<&WorkRunFence>,
) -> Result<(), StoreError> {
    let fence = run_fence.ok_or_else(|| action_conflict("task action has no live run fence"))?;
    fence.validate().map_err(StoreError::Work)?;
    if fence.run_id != run_id {
        return Err(action_conflict(
            "task action run fence does not match its origin",
        ));
    }
    let contract_id = fence.contract_id.as_ref().map(ToString::to_string);
    let changed = transaction.execute(
        r#"
        UPDATE agent_runs
        SET status = 'waiting_for_approval', lease_owner = NULL, lease_token = NULL,
            lease_expires_at = NULL, ended_at = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE run_id = ?1 AND task_id = ?2 AND status = 'running' AND lease_token = ?3
          AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          AND task_generation = ?4
          AND (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) = ?4
          AND ((contract_id IS NULL AND ?5 IS NULL) OR contract_id = ?5)
          AND cancellation_requested = 0
        "#,
        params![
            run_id,
            task_id,
            fence.lease_token,
            fence.task_generation,
            contract_id
        ],
    )?;
    if changed != 1 {
        return Err(action_conflict("task action lost its live run fence"));
    }
    Ok(())
}

pub(crate) fn cancel_task_governed_actions_tx(
    transaction: &rusqlite::Transaction<'_>,
    task_id: &str,
    actor_id: &str,
) -> Result<(), StoreError> {
    let actions = transaction
        .prepare(
            "SELECT action_id, revision, state FROM governed_actions WHERE task_id = ?1 AND state IN ('proposed', 'awaiting_approval', 'executable', 'executing') ORDER BY created_at, action_id",
        )?
        .query_map([task_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (action_id, revision, state) in actions {
        invalidate_action_tx(
            transaction,
            &action_id,
            revision,
            &state,
            actor_id,
            "task_cancelled",
        )?;
    }
    Ok(())
}

pub(crate) fn invalidate_run_governed_actions_tx(
    transaction: &rusqlite::Transaction<'_>,
    run_id: &str,
    actor_id: &str,
) -> Result<(), StoreError> {
    let actions = transaction
        .prepare(
            "SELECT action_id, revision, state FROM governed_actions WHERE run_id = ?1 AND state IN ('proposed', 'awaiting_approval', 'executable', 'executing') ORDER BY created_at, action_id",
        )?
        .query_map([run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (action_id, revision, state) in actions {
        invalidate_action_tx(
            transaction,
            &action_id,
            revision,
            &state,
            actor_id,
            "origin_run_expired",
        )?;
    }
    Ok(())
}

fn invalidate_action_tx(
    transaction: &rusqlite::Transaction<'_>,
    action_id: &str,
    revision: i64,
    current_state: &str,
    actor_id: &str,
    reason: &str,
) -> Result<(), StoreError> {
    let (state, event_kind) = if current_state == "executing" {
        ("outcome_uncertain", "outcome_uncertain")
    } else {
        ("cancelled", "cancelled")
    };
    transaction.execute(
        "UPDATE governed_actions SET state = ?3, failure_code = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE action_id = ?1 AND revision = ?2 AND state = ?5",
        params![action_id, revision, state, reason, current_state],
    )?;
    transaction.execute(
        "UPDATE governed_action_approvals SET state = 'superseded' WHERE action_id = ?1 AND action_revision = ?2 AND state IN ('pending', 'approved')",
        params![action_id, revision],
    )?;
    insert_event(
        transaction,
        action_id,
        u64::try_from(revision).map_err(|_| action_conflict("invalid revision"))?,
        event_kind,
        actor_id,
        &serde_json::json!({"reason": reason}),
    )
}
