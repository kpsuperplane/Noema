//! Reconciliation lineage derivation and durable run queueing.

use noema_tasks::{
    RunKind, WorkDomainError, WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{ApplyReconciliation, reconciliation_scope};
use crate::{
    StoreError,
    work_commands::{WorkCommandService, helpers},
    work_events::append_work_event_tx,
    work_runs::rows::load_run_tx,
};

struct ReconciliationLineage {
    review_round: u32,
    attempt_index: u32,
    parent_run_id: Option<String>,
    triggering_submission_id: Option<String>,
    triggering_review_id: Option<String>,
}

impl ReconciliationLineage {
    fn root(review_round: u32) -> Self {
        Self {
            review_round,
            attempt_index: 0,
            parent_run_id: None,
            triggering_submission_id: None,
            triggering_review_id: None,
        }
    }
}

fn reconciliation_lineage_tx(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
    run_kind: RunKind,
) -> Result<ReconciliationLineage, StoreError> {
    let default_round = u32::from(run_kind != RunKind::Planner);
    let Some(latest_run_id) = task.latest_run_id.as_deref() else {
        return Ok(ReconciliationLineage::root(default_round));
    };
    let Some(parent) = load_run_tx(transaction, latest_run_id)? else {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} points at a missing latest run", task.task_id),
        });
    };
    if parent.task_generation != task.generation {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} latest run crosses its generation", task.task_id),
        });
    }
    if parent.run_kind == run_kind {
        let attempt_index = parent.attempt_index.checked_add(1).ok_or_else(|| {
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "run.attempt_index",
                message: "attempt index overflow".to_string(),
            })
        })?;
        return Ok(ReconciliationLineage {
            review_round: parent.review_round,
            attempt_index,
            parent_run_id: Some(parent.run_id),
            triggering_submission_id: parent.triggering_submission_id,
            triggering_review_id: parent.triggering_review_id,
        });
    }
    match (parent.run_kind, run_kind) {
        (RunKind::Planner, RunKind::Executor) => Ok(ReconciliationLineage {
            review_round: 1,
            attempt_index: 0,
            parent_run_id: Some(parent.run_id),
            triggering_submission_id: None,
            triggering_review_id: None,
        }),
        (RunKind::Executor, RunKind::Reviewer) => {
            let submission_id = task
                .latest_submission_id
                .clone()
                .or(parent.triggering_submission_id)
                .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
            let review_round = transaction
                .query_row(
                    "SELECT review_round FROM task_submissions WHERE submission_id = ?1",
                    [submission_id.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
            Ok(ReconciliationLineage {
                review_round: helpers::nonnegative_u32(review_round, "submission.review_round")?,
                attempt_index: 0,
                parent_run_id: Some(parent.run_id),
                triggering_submission_id: Some(submission_id),
                triggering_review_id: None,
            })
        }
        (RunKind::Reviewer, RunKind::Executor) => {
            let review_round = parent.review_round.checked_add(1).ok_or_else(|| {
                StoreError::Work(WorkDomainError::InvalidInput {
                    field: "run.review_round",
                    message: "review round overflow".to_string(),
                })
            })?;
            let persisted_review = transaction
                .query_row(
                    "SELECT review_id FROM task_reviews WHERE reviewer_run_id = ?1 ORDER BY review_id DESC LIMIT 1",
                    [parent.run_id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            let review_id = task
                .latest_review_id
                .clone()
                .or(persisted_review)
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!(
                        "reviewer run {} has no persisted review lineage",
                        parent.run_id
                    ),
                })?;
            Ok(ReconciliationLineage {
                review_round,
                attempt_index: 0,
                parent_run_id: Some(parent.run_id),
                triggering_submission_id: None,
                triggering_review_id: Some(review_id),
            })
        }
        _ => Err(StoreError::InvariantViolation {
            message: format!(
                "unsupported reconciliation lineage {} -> {}",
                parent.run_kind, run_kind
            ),
        }),
    }
}

fn queue_lineage_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    task: &helpers::TaskState,
    run_kind: RunKind,
    request: &ApplyReconciliation,
    lineage: &ReconciliationLineage,
) -> Result<(String, noema_tasks::WorkEventRecord), StoreError> {
    if run_kind == RunKind::Planner && lineage.parent_run_id.is_some() {
        let parent_id = lineage.parent_run_id.as_deref().expect("checked above");
        let parent =
            load_run_tx(transaction, parent_id)?.ok_or_else(|| StoreError::InvariantViolation {
                message: format!("Planner reconciliation parent is missing: {parent_id}"),
            })?;
        helpers::queue_pinned_child_run_tx(
            transaction,
            service.provider_registry.as_ref(),
            task,
            &parent,
            helpers::QueuePinnedChildRun {
                attempt_index: lineage.attempt_index,
                triggering_submission_id: lineage.triggering_submission_id.as_deref(),
                triggering_review_id: lineage.triggering_review_id.as_deref(),
                event: helpers::CommandEventContext {
                    actor_id: &request.actor_id,
                    causation_id: request.causation_id.as_deref(),
                    correlation_id: &request.correlation_id,
                },
            },
        )
    } else {
        helpers::queue_run_tx(
            transaction,
            service.provider_registry.as_ref(),
            task,
            helpers::QueueRun {
                run_kind,
                contract_id: task.current_contract_id.as_ref(),
                planner_complexity: None,
                review_round: lineage.review_round,
                attempt_index: lineage.attempt_index,
                parent_run_id: lineage.parent_run_id.as_deref(),
                triggering_submission_id: lineage.triggering_submission_id.as_deref(),
                triggering_review_id: lineage.triggering_review_id.as_deref(),
                event: helpers::CommandEventContext {
                    actor_id: &request.actor_id,
                    causation_id: request.causation_id.as_deref(),
                    correlation_id: &request.correlation_id,
                },
            },
        )
    }
}

pub(super) fn queue_reconciled_run_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    task: &helpers::TaskState,
    run_kind: RunKind,
    request: &ApplyReconciliation,
) -> Result<helpers::CommandWrite, StoreError> {
    let lineage = reconciliation_lineage_tx(transaction, task, run_kind)?;
    let (run_id, event) =
        match queue_lineage_tx(transaction, service, task, run_kind, request, &lineage) {
            Ok(queued) => queued,
            Err(error) if helpers::provider_route_unavailable(&error) => {
                let mut task = task.clone();
                return crate::work_commands::recovery::open_configuration_recovery_tx(
                    transaction,
                    &mut task,
                    crate::work_commands::recovery::ConfigurationRecovery {
                        retry_run_kind: run_kind,
                        originating_run_id: lineage.parent_run_id.as_deref(),
                        event: helpers::CommandEventContext {
                            actor_id: &request.actor_id,
                            causation_id: request.causation_id.as_deref(),
                            correlation_id: &request.correlation_id,
                        },
                    },
                );
            }
            Err(error) => return Err(error),
        };
    Ok(helpers::write_marker(
        event,
        Some(task.task_id.clone()),
        task.project_id.clone(),
        task.current_contract_id.clone(),
        task.active_gate_id.clone(),
        Some(run_id),
    ))
}

pub(super) fn move_to_queue_and_queue_run_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    task: &mut helpers::TaskState,
    run_kind: RunKind,
    request: &ApplyReconciliation,
) -> Result<helpers::CommandWrite, StoreError> {
    if task.stage_behavior != WorkflowStageBehavior::HumanGate {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let revision = task.revision.checked_add(1).ok_or_else(|| {
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "task.revision",
            message: "revision overflow".to_string(),
        })
    })?;
    let changed = transaction.execute(
        "UPDATE tasks SET stage_id = 'stage:personal:queue', active_gate_id = NULL, queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), revision = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?3 AND revision = ?4",
        params![task.task_id.as_str(), revision, task.generation, task.revision],
    )?;
    if changed != 1 {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }
    let from_stage = task.stage_id.clone();
    task.stage_id = WorkflowStageId::new("stage:personal:queue").map_err(StoreError::Work)?;
    task.stage_behavior = WorkflowStageBehavior::Dispatch;
    task.active_gate_id = None;
    task.revision = revision;
    append_work_event_tx(
        transaction,
        reconciliation_scope(task, request),
        WorkEventPayload::task_stage_changed(
            revision,
            task.generation,
            from_stage,
            task.stage_id.clone(),
            noema_tasks::TaskStageChangeReason::GateResolved,
        )
        .map_err(StoreError::Work)?,
    )?;
    let lineage = reconciliation_lineage_tx(transaction, task, run_kind)?;
    let (run_id, run_event) =
        match queue_lineage_tx(transaction, service, task, run_kind, request, &lineage) {
            Ok(queued) => queued,
            Err(error) if helpers::provider_route_unavailable(&error) => {
                return crate::work_commands::recovery::open_configuration_recovery_tx(
                    transaction,
                    task,
                    crate::work_commands::recovery::ConfigurationRecovery {
                        retry_run_kind: run_kind,
                        originating_run_id: lineage.parent_run_id.as_deref(),
                        event: helpers::CommandEventContext {
                            actor_id: &request.actor_id,
                            causation_id: request.causation_id.as_deref(),
                            correlation_id: &request.correlation_id,
                        },
                    },
                );
            }
            Err(error) => return Err(error),
        };
    Ok(helpers::write_marker(
        run_event,
        Some(task.task_id.clone()),
        task.project_id.clone(),
        task.current_contract_id.clone(),
        None,
        Some(run_id),
    ))
}
