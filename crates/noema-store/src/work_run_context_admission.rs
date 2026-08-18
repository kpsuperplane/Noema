//! Atomic admission of current Task run context.

use noema_tasks::{
    AgentRunItemKind, AgentRunItemRecord, AgentRunItemStatus, WorkDomainError, WorkEventPayload,
};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{
    StoreError,
    work_commands::WorkCommandService,
    work_events::{WorkEventScope, append_work_event_tx},
    work_run_context::{decode_run_item, load_work_run_execution_context_tx},
    work_run_context_records::WorkRunContextAdmission,
    work_runs::WorkRunFence,
};

impl WorkCommandService {
    /// Admit the current run context at the safe provider boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when the run fence is stale or the current context
    /// cannot be read.
    pub async fn admit_work_run_execution_context(
        &self,
        fence: &WorkRunFence,
        actor_id: &str,
        causation_id: Option<&str>,
        correlation_id: &str,
    ) -> Result<WorkRunContextAdmission, StoreError> {
        fence.validate().map_err(StoreError::Work)?;
        if actor_id.trim().is_empty() || correlation_id.trim().is_empty() {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "run.context_admission",
                message: "actor and correlation identifiers are required".to_string(),
            }));
        }
        self.store
            .with_immediate_transaction_retry(|transaction| {
                require_admission_fence_tx(transaction, fence)?;
                let mut context = load_work_run_execution_context_tx(transaction, &fence.run_id)?
                    .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
                let checkpoint_id = checkpoint_item_id(&fence.run_id);
                let checkpoint = match load_checkpoint_tx(transaction, &checkpoint_id)? {
                    Some(checkpoint) => {
                        validate_checkpoint(&checkpoint, fence)?;
                        checkpoint
                    }
                    None => insert_checkpoint_tx(
                        transaction,
                        &fence.run_id,
                        &checkpoint_id,
                        correlation_id,
                    )?,
                };
                for message in &mut context.messages {
                    if message.consumed_by_run_id.is_some() {
                        continue;
                    }
                    let changed = transaction.execute(
                        "UPDATE task_messages SET consumed_by_run_id = ?2, consumed_at = ?5 WHERE message_id = ?1 AND task_id = ?3 AND task_generation = ?4 AND consumed_by_run_id IS NULL AND consumed_at IS NULL",
                        params![message.message_id.as_str(), fence.run_id, context.task.task_id.as_str(), fence.task_generation, checkpoint.created_at],
                    )?;
                    if changed != 1 {
                        return Err(StoreError::Work(WorkDomainError::RunFenced));
                    }
                    message.consumed_by_run_id = Some(fence.run_id.clone());
                    message.consumed_at = Some(checkpoint.created_at.clone());
                    append_work_event_tx(
                        transaction,
                        WorkEventScope {
                            workspace_id: context.task.workspace_id.clone(),
                            project_id: context.task.project_id.clone(),
                            task_id: Some(context.task.task_id.clone()),
                            run_id: Some(fence.run_id.clone()),
                            actor_id: actor_id.to_string(),
                            causation_id: causation_id.map(ToOwned::to_owned),
                            correlation_id: correlation_id.to_string(),
                        },
                        WorkEventPayload::task_message_consumed(
                            message.message_id.clone(),
                            fence.task_generation,
                            fence.run_id.clone(),
                        )
                        .map_err(StoreError::Work)?,
                    )?;
                }
                Ok(WorkRunContextAdmission { context, checkpoint })
            })
            .await
    }
}

fn require_admission_fence_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
) -> Result<(), StoreError> {
    let matches: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM agent_runs run
            JOIN tasks task ON task.task_id = run.task_id
            WHERE run.run_id = ?1 AND run.lease_token = ?2
              AND run.status = 'running' AND run.cancellation_requested = 0
              AND run.lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
              AND run.task_generation = ?3 AND task.generation = ?3
              AND task.stage_id = 'stage:personal:doing'
              AND task.latest_run_id = run.run_id
        )",
        params![
            fence.run_id,
            fence.lease_token,
            i64::try_from(fence.task_generation).map_err(|_| StoreError::Work(
                WorkDomainError::InvalidInput {
                    field: "run_fence.task_generation",
                    message: "generation exceeds SQLite range".to_string(),
                }
            ))?,
        ],
        |row| row.get(0),
    )?;
    matches
        .then_some(())
        .ok_or(StoreError::Work(WorkDomainError::RunFenced))
}

fn checkpoint_item_id(run_id: &str) -> String {
    format!(
        "run_item:context_checkpoint:{}",
        run_id.strip_prefix("run:").unwrap_or(run_id)
    )
}

fn load_checkpoint_tx(
    transaction: &Transaction<'_>,
    item_id: &str,
) -> Result<Option<AgentRunItemRecord>, StoreError> {
    transaction
        .query_row(
            "SELECT item_id, run_id, sequence_index, round_index, kind, status, correlation_id, parent_item_id, content_text, payload_json, created_at, updated_at FROM agent_run_items WHERE item_id = ?1",
            [item_id],
            decode_run_item,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

fn insert_checkpoint_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
    item_id: &str,
    correlation_id: &str,
) -> Result<AgentRunItemRecord, StoreError> {
    transaction.execute(
        "INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, correlation_id, payload_json) VALUES (?1, ?2, 0, 0, 'context_checkpoint', 'completed', ?3, '{}')",
        params![item_id, run_id, correlation_id],
    )?;
    load_checkpoint_tx(transaction, item_id)?.ok_or_else(|| StoreError::InvariantViolation {
        message: format!("context admission marker disappeared: {item_id}"),
    })
}

fn validate_checkpoint(
    checkpoint: &AgentRunItemRecord,
    fence: &WorkRunFence,
) -> Result<(), StoreError> {
    if checkpoint.item_id != checkpoint_item_id(&fence.run_id)
        || checkpoint.run_id != fence.run_id
        || checkpoint.kind != AgentRunItemKind::ContextCheckpoint
        || checkpoint.status != AgentRunItemStatus::Completed
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    Ok(())
}
