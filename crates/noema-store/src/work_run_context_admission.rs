//! Atomic child-context admission and exact checkpoint replay.

use std::collections::HashSet;

use noema_tasks::{
    AgentRunItemKind, AgentRunItemRecord, AgentRunItemStatus, WorkDomainError, WorkEventPayload,
};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{
    StoreError,
    work_commands::WorkCommandService,
    work_events::{WorkEventScope, append_work_event_tx},
    work_run_context::{
        MAX_CONTEXT_PAYLOAD_BYTES, decode_run_item, load_work_run_execution_context_tx,
    },
    work_run_context_records::{WORK_RUN_CONTEXT_MAX_GATES, WORK_RUN_CONTEXT_MAX_MESSAGES},
    work_run_context_records::{WorkRunContextAdmission, WorkRunExecutionContext},
    work_runs::WorkRunFence,
};

impl WorkCommandService {
    /// Admit one exact run context at the safe provider boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when the fence or event scope is invalid, the run is
    /// stale, or its immutable execution context cannot be admitted.
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
                let checkpoint_id = checkpoint_item_id(&fence.run_id);
                if let Some(checkpoint) = load_checkpoint_tx(transaction, &checkpoint_id)? {
                    let context = checkpoint_context(&checkpoint, fence)?;
                    return Ok(WorkRunContextAdmission { context, checkpoint });
                }

                let mut context = load_work_run_execution_context_tx(transaction, &fence.run_id)?
                    .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
                if let Some(ancestor) = nearest_ancestor_checkpoint_context_tx(
                    transaction,
                    &context.run,
                    context.run.parent_run_id.as_deref(),
                )? {
                    context = inherit_checkpoint_context(ancestor, context)?;
                }

                let newly_admitted = context
                    .messages
                    .iter()
                    .filter(|message| message.consumed_by_run_id.is_none())
                    .cloned()
                    .collect::<Vec<_>>();
                let (checkpoint, context) = insert_checkpoint_tx(
                    transaction,
                    context,
                    &checkpoint_id,
                    correlation_id,
                )?;
                for message in &newly_admitted {
                    let changed = transaction.execute(
                        "UPDATE task_messages SET consumed_by_run_id = ?2, consumed_at = ?5 WHERE message_id = ?1 AND task_id = ?3 AND task_generation = ?4 AND consumed_by_run_id IS NULL AND consumed_at IS NULL",
                        params![message.message_id.as_str(), fence.run_id, context.task.task_id.as_str(), fence.task_generation, checkpoint.created_at],
                    )?;
                    if changed != 1 {
                        return Err(StoreError::Work(WorkDomainError::RunFenced));
                    }
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
              AND ((run.contract_id IS NULL AND ?4 IS NULL) OR run.contract_id = ?4)
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
            fence.contract_id.as_ref().map(ToString::to_string),
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
    mut context: WorkRunExecutionContext,
    item_id: &str,
    correlation_id: &str,
) -> Result<(AgentRunItemRecord, WorkRunExecutionContext), StoreError> {
    let created_at: String =
        transaction.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })?;
    for message in &mut context.messages {
        if message.consumed_by_run_id.is_none() {
            message.consumed_by_run_id = Some(context.run.run_id.clone());
            message.consumed_at = Some(created_at.clone());
        }
    }
    let payload = serde_json::to_string(&serde_json::json!({
        "v": 1,
        "task_id": &context.task.task_id,
        "generation": context.task.generation,
        "contract_id": &context.run.contract_id,
        "context": &context,
    }))?;
    if payload.len() > MAX_CONTEXT_PAYLOAD_BYTES {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "run.context_checkpoint",
            message: "checkpoint exceeds bounded context".to_string(),
        }));
    }
    transaction.execute(
        "INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, correlation_id, payload_json, created_at, updated_at) VALUES (?1, ?2, 0, 0, 'context_checkpoint', 'completed', ?3, ?4, ?5, ?5)",
        params![item_id, context.run.run_id, correlation_id, payload, created_at],
    )?;
    let checkpoint = load_checkpoint_tx(transaction, item_id)?.ok_or_else(|| {
        StoreError::InvariantViolation {
            message: format!("context checkpoint disappeared: {item_id}"),
        }
    })?;
    Ok((checkpoint, context))
}

fn checkpoint_context(
    checkpoint: &AgentRunItemRecord,
    fence: &WorkRunFence,
) -> Result<WorkRunExecutionContext, StoreError> {
    let context = decode_checkpoint_context(checkpoint)?;
    if checkpoint.item_id != checkpoint_item_id(&fence.run_id)
        || context.run.run_id != fence.run_id
        || context.run.task_generation != fence.task_generation
        || context.run.contract_id != fence.contract_id
        || context
            .messages
            .iter()
            .any(|message| message.consumed_by_run_id.is_none())
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    Ok(context)
}

fn decode_checkpoint_context(
    checkpoint: &AgentRunItemRecord,
) -> Result<WorkRunExecutionContext, StoreError> {
    if checkpoint.kind != AgentRunItemKind::ContextCheckpoint
        || checkpoint.status != AgentRunItemStatus::Completed
    {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "run item {} is not a completed context checkpoint",
                checkpoint.item_id
            ),
        });
    }
    let context = checkpoint
        .payload
        .get("context")
        .cloned()
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!(
                "context checkpoint {} has invalid payload",
                checkpoint.item_id
            ),
        })
        .and_then(|value| {
            serde_json::from_value::<WorkRunExecutionContext>(value).map_err(StoreError::Json)
        })?;
    if checkpoint.item_id != checkpoint_item_id(&checkpoint.run_id)
        || context.run.run_id != checkpoint.run_id
        || context.task.task_id != context.run.task_id
        || context.task.generation != context.run.task_generation
    {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "context checkpoint {} crosses its run/task identity",
                checkpoint.item_id
            ),
        });
    }
    Ok(context)
}

fn nearest_ancestor_checkpoint_context_tx(
    transaction: &Transaction<'_>,
    current_run: &noema_tasks::AgentRunRecord,
    parent_run_id: Option<&str>,
) -> Result<Option<WorkRunExecutionContext>, StoreError> {
    let mut next = parent_run_id.map(ToOwned::to_owned);
    let mut seen = HashSet::new();
    for _ in 0..256 {
        let Some(run_id) = next else {
            return Ok(None);
        };
        if !seen.insert(run_id.clone()) {
            return Err(StoreError::InvariantViolation {
                message: "run checkpoint lineage is cyclic".to_string(),
            });
        }
        if let Some(checkpoint) = load_checkpoint_tx(transaction, &checkpoint_item_id(&run_id))? {
            let context = decode_checkpoint_context(&checkpoint)?;
            if context.run.run_kind == current_run.run_kind
                && context.run.task_generation == current_run.task_generation
                && context.run.contract_id == current_run.contract_id
                && context.run.review_round == current_run.review_round
                && context.run.triggering_submission_id == current_run.triggering_submission_id
                && context.run.triggering_review_id == current_run.triggering_review_id
            {
                return Ok(Some(context));
            }
        }
        next = transaction
            .query_row(
                "SELECT parent_run_id FROM agent_runs WHERE run_id = ?1",
                [run_id.as_str()],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
    }
    Err(StoreError::InvariantViolation {
        message: "run checkpoint lineage exceeds the recovery bound".to_string(),
    })
}

fn inherit_checkpoint_context(
    ancestor: WorkRunExecutionContext,
    mut current: WorkRunExecutionContext,
) -> Result<WorkRunExecutionContext, StoreError> {
    if ancestor.task.task_id != current.task.task_id
        || ancestor.task.generation != current.task.generation
        || ancestor.run.contract_id != current.run.contract_id
    {
        return Err(StoreError::InvariantViolation {
            message: "recovery child crosses its ancestor context fence".to_string(),
        });
    }
    let latest_submission = select_inherited_submission(&ancestor, &current);
    let latest_review = select_inherited_review(&ancestor, &current);

    let ancestor_gate_ids = ancestor
        .relevant_gates
        .iter()
        .map(|gate| gate.gate_id.as_str().to_string())
        .collect::<HashSet<_>>();
    let mut gates = ancestor.relevant_gates;
    for gate in current.relevant_gates {
        if !ancestor_gate_ids.contains(gate.gate_id.as_str()) {
            gates.push(gate);
        }
    }
    gates.sort_by(|left, right| {
        left.opened_at
            .cmp(&right.opened_at)
            .then_with(|| left.gate_id.as_str().cmp(right.gate_id.as_str()))
    });
    if gates.len() > WORK_RUN_CONTEXT_MAX_GATES {
        return Err(StoreError::InvariantViolation {
            message: "recovery child exceeds bounded inherited gates".to_string(),
        });
    }

    let mut message_ids = ancestor
        .messages
        .iter()
        .map(|message| message.message_id.as_str().to_string())
        .collect::<HashSet<_>>();
    let mut messages = ancestor.messages;
    for message in current.messages {
        let belongs_to_new_gate = message
            .gate_id
            .as_ref()
            .is_some_and(|gate_id| !ancestor_gate_ids.contains(gate_id.as_str()));
        let triggers_current_review = current
            .run
            .triggering_review_id
            .as_deref()
            .is_some_and(|review_id| message.review_id.as_deref() == Some(review_id));
        if !message_ids.contains(message.message_id.as_str())
            && (belongs_to_new_gate || triggers_current_review)
        {
            message_ids.insert(message.message_id.as_str().to_string());
            messages.push(message);
        }
    }
    messages.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.message_id.as_str().cmp(right.message_id.as_str()))
    });
    if messages.len() > WORK_RUN_CONTEXT_MAX_MESSAGES {
        return Err(StoreError::InvariantViolation {
            message: "recovery child exceeds bounded inherited messages".to_string(),
        });
    }

    current.workspace = ancestor.workspace;
    current.project = ancestor.project;
    current.contract = ancestor.contract;
    current.relevant_gates = gates;
    current.messages = messages;
    current.latest_submission = latest_submission;
    current.latest_review = latest_review;
    Ok(current)
}

fn select_inherited_submission(
    ancestor: &WorkRunExecutionContext,
    current: &WorkRunExecutionContext,
) -> Option<noema_tasks::TaskSubmissionRecord> {
    current
        .run
        .triggering_submission_id
        .as_deref()
        .and_then(|id| {
            current
                .latest_submission
                .as_ref()
                .filter(|submission| submission.submission_id == id)
                .cloned()
        })
        .or_else(|| ancestor.latest_submission.clone())
}

fn select_inherited_review(
    ancestor: &WorkRunExecutionContext,
    current: &WorkRunExecutionContext,
) -> Option<noema_tasks::TaskReviewRecord> {
    current
        .run
        .triggering_review_id
        .as_deref()
        .and_then(|id| {
            current
                .latest_review
                .as_ref()
                .filter(|review| review.review_id == id)
                .cloned()
        })
        .or_else(|| ancestor.latest_review.clone())
}
