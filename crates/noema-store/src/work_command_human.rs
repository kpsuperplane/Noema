//! Human gate, cancellation, and completion-reopen command transactions.

use std::str::FromStr;

use noema_tasks::{
    AnswerTask, GateResolutionKind, PERSONAL_QUEUE_STAGE_ID, RetryTask, RunKind, RunStatus,
    RunTerminalKind, TaskGateId, TaskGateState, TaskMessageKind, TaskRecoveryReason, WorkCommand,
    WorkDomainError, WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{WorkCommandService, helpers};
use crate::{
    StoreError, ids::allocate_id, run_items::finish_agent_run_records_tx,
    work_events::append_work_event_tx,
};

#[path = "work_command_cancel_reopen.rs"]
mod cancel_reopen;
#[path = "work_command_reopen.rs"]
mod reopen;
#[path = "work_command_human_validation.rs"]
mod validation;

struct OriginatingRunRow {
    run_kind: String,
    attempt_index: i64,
    review_round: i64,
}

struct OriginatingRun {
    run_kind: RunKind,
    attempt_index: u32,
    review_round: u32,
}

pub(super) async fn execute(
    service: &WorkCommandService,
    command: &WorkCommand,
) -> Result<helpers::CommandWrite, StoreError> {
    match command {
        WorkCommand::AnswerTask(value) => answer(service, value).await,
        WorkCommand::RetryTask(value) => retry(service, value).await,
        WorkCommand::CancelTask(value) => cancel_reopen::cancel(service, value).await,
        WorkCommand::ReopenTask(value) => reopen::execute(service, value).await,
        _ => Err(StoreError::InvariantViolation {
            message: "human writer received an unsupported command".to_string(),
        }),
    }
}

async fn answer(
    service: &WorkCommandService,
    command: &AnswerTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::AnswerTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    helpers::command_transaction(&service.store, &envelope, |transaction| {
            let mut task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
            if task.stage_behavior != WorkflowStageBehavior::HumanGate {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            let gate = load_gate(transaction, &command.gate_id)?;
            validate_open_gate(&task, &gate, &command.gate_id)?;
            validation::validate_gate_resolution(&gate, GateResolutionKind::Answer)?;
            let from_stage = task.stage_id.clone();
            let answer = command
                .answer
                .normalized_for(gate.kind)
                .map_err(StoreError::Work)?;
            let message_id = noema_tasks::TaskMessageId::new(allocate_id("task_message"))
                .map_err(StoreError::Work)?;
            transaction.execute(
                "INSERT INTO task_messages (message_id, task_id, task_generation, gate_id, message_kind, body_markdown, approval_decision, author_actor_id) VALUES (?1, ?2, ?3, ?4, 'human_answer', ?5, ?6, ?7)",
                params![
                    message_id.as_str(),
                    task_id.as_str(),
                    task.generation,
                    gate.gate_id.as_str(),
                    answer.message_markdown,
                    answer.approval_decision.map(|value| value.as_str()),
                    command.meta.actor_id,
                ],
            )?;
            transaction.execute(
                "UPDATE task_gates SET gate_state = 'resolved', resolved_by_actor_id = ?2, resolution_message_id = ?3, resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE gate_id = ?1 AND gate_state = 'open'",
                params![gate.gate_id.as_str(), command.meta.actor_id, message_id.as_str()],
            )?;
            let revision = helpers::increment(task.revision, "task.revision")?;
            transaction.execute(
                "UPDATE tasks SET stage_id = ?2, active_gate_id = NULL, queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?4 AND generation = ?5",
                params![task_id.as_str(), PERSONAL_QUEUE_STAGE_ID, revision, task.revision, task.generation],
            )?;
            task.stage_id = WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?;
            task.stage_behavior = WorkflowStageBehavior::Dispatch;
            task.revision = revision;
            let _message_event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).task_scope(&task, None),
                WorkEventPayload::task_message_appended(
                    message_id.clone(),
                    task.generation,
                    TaskMessageKind::HumanAnswer,
                    Some(gate.gate_id.clone()),
                )
                .map_err(StoreError::Work)?,
            )?;
            let _gate_event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).task_scope(&task, None),
                WorkEventPayload::gate_resolved(
                    gate.gate_id.clone(),
                    task.generation,
                    gate.kind,
                    message_id,
                    GateResolutionKind::Answer,
                )
                .map_err(StoreError::Work)?,
            )?;
            let _stage_event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).task_scope(&task, None),
                WorkEventPayload::task_stage_changed(
                    revision,
                    task.generation,
                    from_stage,
                    task.stage_id.clone(),
                    noema_tasks::TaskStageChangeReason::GateResolved,
                )
                .map_err(StoreError::Work)?,
            )?;
            let originating = gate
                .originating_run_id
                .as_deref()
                .map(|originating_run_id| {
                    transaction
                        .query_row(
                            "SELECT run_kind, attempt_index, review_round FROM agent_runs WHERE run_id = ?1",
                            [originating_run_id],
                            |row| {
                                Ok(OriginatingRunRow {
                                    run_kind: row.get(0)?,
                                    attempt_index: row.get(1)?,
                                    review_round: row.get(2)?,
                                })
                            },
                        )
                        .optional()
                })
                .transpose()?
                .flatten()
                .map(|row| {
                    Ok::<_, StoreError>(OriginatingRun {
                        run_kind: RunKind::from_str(&row.run_kind).map_err(StoreError::Work)?,
                        attempt_index: helpers::nonnegative_u32(
                            row.attempt_index,
                            "run.attempt_index",
                        )?,
                        review_round: helpers::nonnegative_u32(
                            row.review_round,
                            "run.review_round",
                        )?,
                    })
                })
                .transpose()?;
            if let (Some(originating_run_id), Some(origin)) =
                (gate.originating_run_id.as_deref(), originating.as_ref())
            {
                transaction.execute(
                    "UPDATE agent_runs SET status = 'completed', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'waiting_for_approval'",
                    [originating_run_id],
                )?;
                finish_agent_run_records_tx(
                    transaction,
                    originating_run_id,
                    RunStatus::Completed,
                )?;
                let _origin_completed_event = append_work_event_tx(
                    transaction,
                    helpers::event_context(&command.meta)
                        .task_scope(&task, Some(originating_run_id)),
                    WorkEventPayload::run_completed(
                        origin.run_kind,
                        task.generation,
                        RunTerminalKind::GateResolved,
                    )
                    .map_err(StoreError::Work)?,
                )?;
            }
            let origin = gate
                .originating_run_id
                .as_deref();
            let run_kind = gate
                .retry_run_kind
                .or_else(|| originating.as_ref().map(|origin| origin.run_kind))
                .ok_or(StoreError::Work(WorkDomainError::ConfigurationUnavailable))?;
            let attempt_index = originating
                .as_ref()
                .map_or(0, |origin| origin.attempt_index)
                .checked_add(1)
                .ok_or_else(|| StoreError::Work(WorkDomainError::InvalidInput {
                    field: "run.attempt_index",
                    message: "attempt index overflow".to_string(),
                }))?;
            let review_round = originating
                .as_ref()
                .map_or(u32::from(run_kind != RunKind::Planner), |origin| {
                    origin.review_round
                });
            let queued = if let Some(origin_run_id) = origin {
                let parent = crate::work_runs::rows::load_run_tx(transaction, origin_run_id)?
                    .ok_or_else(|| StoreError::InvariantViolation {
                        message: format!("gate {} references a missing run", gate.gate_id),
                    })?;
                if parent.run_kind != run_kind {
                    return Err(StoreError::InvariantViolation {
                        message: format!("gate {} changes its retry run kind", gate.gate_id),
                    });
                }
                helpers::queue_pinned_child_run_tx(
                    transaction,
                    service.provider_registry.as_ref(),
                    &task,
                    &parent,
                    helpers::QueuePinnedChildRun {
                        attempt_index,
                        event: helpers::event_context(&command.meta),
                    },
                )
            } else {
                helpers::queue_run_tx(
                    transaction,
                    service.provider_registry.as_ref(),
                    &task,
                    helpers::QueueRun {
                        run_kind,
                        planner_complexity: None,
                        review_round,
                        attempt_index,
                        parent_run_id: origin,
                        event: helpers::event_context(&command.meta),
                    },
                )
            };
            let (run_id, run_event) = match queued {
                Ok(queued) => queued,
                Err(error) if helpers::provider_route_unavailable(&error) => {
                    let write = super::recovery::open_configuration_recovery_tx(
                        transaction,
                        &mut task,
                        super::recovery::ConfigurationRecovery {
                            retry_run_kind: run_kind,
                            originating_run_id: origin,
                            event: helpers::event_context(&command.meta),
                        },
                    )?;
                    return Ok(write.into());
                }
                Err(error) => return Err(error),
            };
            Ok(helpers::task_write(run_event, task_id.clone())
                .gate(Some(gate.gate_id))
                .run(Some(run_id))
                .into())
        })
        .await
}

async fn retry(
    service: &WorkCommandService,
    command: &RetryTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::RetryTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        let mut task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
        if task.stage_behavior != WorkflowStageBehavior::HumanGate {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let gate = load_gate(transaction, &command.gate_id)?;
        validate_open_gate(&task, &gate, &command.gate_id)?;
        validation::validate_gate_resolution(&gate, GateResolutionKind::Retry)?;

        // SQLite requires a resolution message for every resolved gate. Keep a
        // bounded synthetic note when the caller omitted one, so Retry remains
        // atomic and the gate cannot be left in an impossible half-resolved
        // state.
        let message_id = noema_tasks::TaskMessageId::new(allocate_id("task_message"))
            .map_err(StoreError::Work)?;
        let note = command.note.as_deref().unwrap_or("Retry requested.");
        transaction.execute(
            "INSERT INTO task_messages (message_id, task_id, task_generation, gate_id, message_kind, body_markdown, author_actor_id) VALUES (?1, ?2, ?3, ?4, 'retry_note', ?5, ?6)",
            params![
                message_id.as_str(),
                task_id.as_str(),
                task.generation,
                gate.gate_id.as_str(),
                note,
                command.meta.actor_id,
            ],
        )?;
        transaction.execute(
            "UPDATE task_gates SET gate_state = 'resolved', resolved_by_actor_id = ?2, resolution_message_id = ?3, resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE gate_id = ?1 AND gate_state = 'open'",
            params![gate.gate_id.as_str(), command.meta.actor_id, message_id.as_str()],
        )?;
        let from_stage = task.stage_id.clone();
        let revision = helpers::increment(task.revision, "task.revision")?;
        transaction.execute(
            "UPDATE tasks SET stage_id = ?2, active_gate_id = NULL, queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?4 AND generation = ?5",
            params![task_id.as_str(), PERSONAL_QUEUE_STAGE_ID, revision, task.revision, task.generation],
        )?;
        task.stage_id = WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?;
        task.stage_behavior = WorkflowStageBehavior::Dispatch;
        task.revision = revision;
            let _gate_event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).task_scope(&task, None),
            WorkEventPayload::gate_resolved(
                gate.gate_id.clone(),
                task.generation,
                gate.kind,
                message_id.clone(),
                GateResolutionKind::Retry,
            )
            .map_err(StoreError::Work)?,
        )?;
        if command.note.is_some() {
            let _note_event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).task_scope(&task, None),
                WorkEventPayload::task_message_appended(
                    message_id.clone(),
                    task.generation,
                    TaskMessageKind::RetryNote,
                    Some(gate.gate_id.clone()),
                )
                .map_err(StoreError::Work)?,
            )?;
        }
        let _stage_event = append_work_event_tx(
            transaction,
            helpers::event_context(&command.meta).task_scope(&task, None),
            WorkEventPayload::task_stage_changed(
                revision,
                task.generation,
                from_stage,
                task.stage_id.clone(),
                noema_tasks::TaskStageChangeReason::GateResolved,
            )
            .map_err(StoreError::Work)?,
        )?;
        let run_kind = gate
            .retry_run_kind
            .ok_or(StoreError::Work(WorkDomainError::ConfigurationUnavailable))?;
        let parent = gate
            .originating_run_id
            .as_deref()
            .map(|run_id| {
                crate::work_runs::rows::load_run_tx(transaction, run_id)?.ok_or_else(|| {
                    StoreError::InvariantViolation {
                        message: format!("gate {} references a missing run", gate.gate_id),
                    }
                })
            })
            .transpose()?;
        let queued = if gate.recovery_reason == Some(TaskRecoveryReason::ReviewRoundsExhausted) {
            let parent = parent.as_ref().ok_or_else(|| StoreError::InvariantViolation {
                message: format!("review recovery gate {} has no originating run", gate.gate_id),
            })?;
            if parent.run_kind != RunKind::Reviewer || run_kind != RunKind::Executor {
                return Err(StoreError::InvariantViolation {
                    message: format!("review recovery gate {} has invalid role lineage", gate.gate_id),
                });
            }
            let review_round = helpers::increment(parent.review_round, "run.review_round")?;
            helpers::queue_run_tx(
                transaction,
                service.provider_registry.as_ref(),
                &task,
                helpers::QueueRun {
                    run_kind: RunKind::Executor,
                    planner_complexity: None,
                    review_round,
                    attempt_index: 0,
                    parent_run_id: Some(&parent.run_id),
                    event: helpers::event_context(&command.meta),
                },
            )
        } else if let Some(parent) = parent.as_ref() {
            if parent.run_kind != run_kind {
                return Err(StoreError::InvariantViolation {
                    message: format!("gate {} changes its retry run kind", gate.gate_id),
                });
            }
            let attempt_index = helpers::increment(parent.attempt_index, "run.attempt_index")?;
            helpers::queue_pinned_child_run_tx(
                transaction,
                service.provider_registry.as_ref(),
                &task,
                parent,
                helpers::QueuePinnedChildRun {
                    attempt_index,
                    event: helpers::event_context(&command.meta),
                },
            )
        } else {
            helpers::queue_run_tx(
                transaction,
                service.provider_registry.as_ref(),
                &task,
                helpers::QueueRun {
                    run_kind,
                    planner_complexity: None,
                    review_round: u32::from(run_kind != RunKind::Planner),
                    attempt_index: 0,
                    parent_run_id: None,
                    event: helpers::event_context(&command.meta),
                },
            )
        };
        let (run_id, run_event) = match queued {
            Ok(queued) => queued,
            Err(error) if helpers::provider_route_unavailable(&error) => {
                let write = super::recovery::open_configuration_recovery_tx(
                    transaction,
                    &mut task,
                    super::recovery::ConfigurationRecovery {
                        retry_run_kind: run_kind,
                        originating_run_id: gate.originating_run_id.as_deref(),
                        event: helpers::event_context(&command.meta),
                    },
                )?;
                return Ok(write.into());
            }
            Err(error) => return Err(error),
        };
        Ok(helpers::task_write(run_event, task_id.clone())
            .gate(Some(gate.gate_id))
            .run(Some(run_id))
            .into())
    })
    .await
}

fn load_gate(
    transaction: &Transaction<'_>,
    gate_id: &TaskGateId,
) -> Result<noema_tasks::TaskGateRecord, StoreError> {
    crate::work_reads::rows::load_gate_optional(transaction, gate_id)?
        .ok_or(StoreError::Work(WorkDomainError::GateRequired))
}

fn validate_open_gate(
    task: &helpers::TaskState,
    gate: &noema_tasks::TaskGateRecord,
    expected: &TaskGateId,
) -> Result<(), StoreError> {
    if &gate.gate_id != expected
        || gate.state != TaskGateState::Open
        || gate.task_generation != task.generation
        || task.active_gate_id.as_ref() != Some(expected)
    {
        return Err(StoreError::Work(WorkDomainError::GateRequired));
    }
    Ok(())
}
