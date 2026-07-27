//! Resumption of a background run after one governed action is resolved.

use noema_providers::ProviderRegistry;
use noema_tasks::{RunStatus, TaskId, WorkDomainError};
use rusqlite::{OptionalExtension, params};

use super::{WorkCommandService, helpers};
use crate::{GovernedActionState, StoreError, governed_actions::action_from_tx, work_runs::rows};

impl WorkCommandService {
    /// Complete a waiting parent run with the exact governed-action outcome and
    /// queue one pinned child continuation when the outcome is actionable.
    /// Foreground actions return `None`; uncertain outcomes stop at the
    /// completed parent run so a model cannot repeat an ambiguous write.
    ///
    /// The operation is idempotent so resolution recovery can safely call it
    /// after the external action outcome has already been persisted.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the action is not terminal, its task lineage
    /// is stale, or the pinned provider route can no longer be resolved.
    pub async fn resume_after_governed_action(
        &self,
        action_id: &str,
        revision: u64,
        actor_id: &str,
    ) -> Result<Option<String>, StoreError> {
        self.store
            .with_immediate_transaction_retry(|transaction| {
                let action = action_from_tx(transaction, action_id, revision)?
                    .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
                if !matches!(
                    action.state,
                    GovernedActionState::Succeeded
                        | GovernedActionState::Failed
                        | GovernedActionState::OutcomeUncertain
                        | GovernedActionState::Declined
                        | GovernedActionState::Superseded
                        | GovernedActionState::Cancelled
                ) {
                    return Err(StoreError::Work(WorkDomainError::InvalidTransition));
                }
                let (Some(task_id), Some(parent_run_id)) =
                    (action.task_id.as_deref(), action.run_id.as_deref())
                else {
                    return Ok(None);
                };
                let run_item_status = match action.state {
                    GovernedActionState::Succeeded => "completed",
                    GovernedActionState::Declined
                    | GovernedActionState::Superseded
                    | GovernedActionState::Cancelled => "skipped",
                    GovernedActionState::Failed | GovernedActionState::OutcomeUncertain => "failed",
                    _ => unreachable!("terminal state checked above"),
                };
                let payload = serde_json::to_string(&serde_json::json!({
                    "governed_action": {
                        "action_id": action.action_id,
                        "revision": action.revision,
                        "capability_name": action.capability_name,
                        "state": action.state.as_str(),
                        "result": action.output,
                        "failure_code": action.failure_code,
                    }
                }))?;
                resume_waiting_run_tx(
                    transaction,
                    self.provider_registry.as_ref(),
                    task_id,
                    parent_run_id,
                    actor_id,
                    InterventionRunResult {
                        item_id: format!("run_item:governed_action:{}", action.action_id),
                        status: run_item_status,
                        correlation_id: action.action_id.clone(),
                        content: format!(
                            "Governed action {} {}",
                            action.capability_name,
                            action.state.as_str()
                        ),
                        payload,
                        queue_child: action.state != GovernedActionState::OutcomeUncertain,
                    },
                )
            })
            .await
    }
}

pub(super) struct InterventionRunResult {
    pub(super) item_id: String,
    pub(super) status: &'static str,
    pub(super) correlation_id: String,
    pub(super) content: String,
    pub(super) payload: String,
    pub(super) queue_child: bool,
}

pub(super) fn resume_waiting_run_tx(
    transaction: &rusqlite::Transaction<'_>,
    provider_registry: &ProviderRegistry,
    task_id: &str,
    parent_run_id: &str,
    actor_id: &str,
    result: InterventionRunResult,
) -> Result<Option<String>, StoreError> {
    if let Some(child_id) = transaction
        .query_row(
            "SELECT run_id FROM agent_runs WHERE parent_run_id = ?1 ORDER BY created_at, run_id LIMIT 1",
            [parent_run_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(Some(child_id));
    }
    if !result.queue_child
        && transaction
            .query_row(
                "SELECT 1 FROM agent_run_items WHERE run_id = ?1 AND item_id = ?2",
                params![parent_run_id, result.item_id],
                |_row| Ok(()),
            )
            .optional()?
            .is_some()
    {
        return Ok(None);
    }
    let task_id = TaskId::new(task_id).map_err(StoreError::Work)?;
    let task = helpers::load_task_state_tx(transaction, &task_id)?;
    let parent = rows::load_run_tx(transaction, parent_run_id)?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    if parent.status != RunStatus::WaitingForApproval
        || parent.task_generation != task.generation
        || parent.contract_id != task.current_contract_id
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let sequence_index: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM agent_run_items WHERE run_id = ?1 AND kind <> 'context_checkpoint'",
        [parent_run_id],
        |row| row.get(0),
    )?;
    transaction.execute(
        "INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, correlation_id, content_text, payload_json) VALUES (?1, ?2, ?3, ?4, 'tool_result', ?5, ?6, ?7, ?8)",
        params![result.item_id, parent_run_id, sequence_index, i64::from(parent.review_round), result.status, result.correlation_id, result.content, result.payload],
    )?;
    if transaction.execute(
        "UPDATE agent_runs SET status = 'completed', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'waiting_for_approval'",
        [parent_run_id],
    )? != 1
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    if !result.queue_child {
        return Ok(None);
    }
    let attempt_index = parent.attempt_index.checked_add(1).ok_or_else(|| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "run.attempt_index",
            message: "attempt index overflow".to_string(),
        })
    })?;
    let correlation_id = format!("correlation:{}", result.correlation_id);
    let (child_id, _event) = helpers::queue_pinned_child_run_tx(
        transaction,
        provider_registry,
        &task,
        &parent,
        helpers::QueuePinnedChildRun {
            attempt_index,
            triggering_submission_id: parent.triggering_submission_id.as_deref(),
            triggering_review_id: parent.triggering_review_id.as_deref(),
            event: helpers::CommandEventContext {
                actor_id,
                causation_id: None,
                correlation_id: &correlation_id,
            },
        },
    )?;
    Ok(Some(child_id))
}
