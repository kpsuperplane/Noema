//! Reconciliation lineage derivation and durable run queueing.

use noema_tasks::{
    RunKind, WorkDomainError, WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{Transaction, params};

use super::{ApplyReconciliation, task_write};
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
}

impl ReconciliationLineage {
    fn root(review_round: u32) -> Self {
        Self {
            review_round,
            attempt_index: 0,
            parent_run_id: None,
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
        let attempt_index = helpers::increment(parent.attempt_index, "run.attempt_index")?;
        return Ok(ReconciliationLineage {
            review_round: parent.review_round,
            attempt_index,
            parent_run_id: Some(parent.run_id),
        });
    }
    match (parent.run_kind, run_kind) {
        (RunKind::Planner, RunKind::Executor) => Ok(ReconciliationLineage {
            parent_run_id: Some(parent.run_id),
            ..ReconciliationLineage::root(1)
        }),
        (RunKind::Executor, RunKind::Reviewer) => Ok(ReconciliationLineage {
            review_round: parent.review_round,
            parent_run_id: Some(parent.run_id),
            ..ReconciliationLineage::root(0)
        }),
        (RunKind::Reviewer, RunKind::Executor) => {
            let review_round = helpers::increment(parent.review_round, "run.review_round")?;
            Ok(ReconciliationLineage {
                review_round,
                parent_run_id: Some(parent.run_id),
                ..ReconciliationLineage::root(0)
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
                event: request.event_context(),
            },
        )
    } else {
        helpers::queue_run_tx(
            transaction,
            service.provider_registry.as_ref(),
            task,
            helpers::QueueRun {
                run_kind,
                planner_complexity: None,
                review_round: lineage.review_round,
                attempt_index: lineage.attempt_index,
                parent_run_id: lineage.parent_run_id.as_deref(),
                event: request.event_context(),
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
    let mut task = task.clone();
    let lineage = reconciliation_lineage_tx(transaction, &task, run_kind)?;
    queue_or_recover_tx(transaction, service, &mut task, run_kind, request, &lineage)
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
    let revision = helpers::increment(task.revision, "task.revision")?;
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
        request.scope(task, None),
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
    queue_or_recover_tx(transaction, service, task, run_kind, request, &lineage)
}

fn queue_or_recover_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    task: &mut helpers::TaskState,
    run_kind: RunKind,
    request: &ApplyReconciliation,
    lineage: &ReconciliationLineage,
) -> Result<helpers::CommandWrite, StoreError> {
    let (run_id, event) =
        match queue_lineage_tx(transaction, service, task, run_kind, request, lineage) {
            Ok(queued) => queued,
            Err(error) if helpers::provider_route_unavailable(&error) => {
                return crate::work_commands::recovery::open_configuration_recovery_tx(
                    transaction,
                    task,
                    crate::work_commands::recovery::ConfigurationRecovery {
                        retry_run_kind: run_kind,
                        originating_run_id: lineage.parent_run_id.as_deref(),
                        event: request.event_context(),
                    },
                );
            }
            Err(error) => return Err(error),
        };
    Ok(task_write(event, task).run(Some(run_id)))
}
