use noema_tasks::{
    RunKind, RunTerminalKind, TaskGateKind, TaskRecoveryReason, TaskReviewVerdict,
    TaskStageChangeReason, WorkDomainError, WorkEventPayload, WorkflowStageBehavior,
    WorkflowStageId,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::super::SubmitTaskReview;
use super::terminal_helpers::{
    OpenGate, bump_task_to_waiting_tx, child_run_event_tx, contract_criterion_ids_tx,
    insert_gate_tx, load_running_fence_tx, load_terminal_run_identity_tx, mark_run_completed_tx,
    mark_run_waiting_tx, notification_queued_event_for_run_tx, run_scope,
    task_execution_policy_for_task, validate_namespace,
};
use crate::{
    StoreError,
    ids::allocate_id,
    work_commands::{WorkCommandService, helpers},
    work_events::append_work_event_tx,
    work_notifications::enqueue_work_notification_tx,
};

pub(super) fn submit_review_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    command: &SubmitTaskReview,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> Result<helpers::CommandWrite, StoreError> {
    let (run, mut task) = load_running_fence_tx(transaction, &command.fence, RunKind::Reviewer)?;
    let contract_id = command
        .fence
        .contract_id
        .as_ref()
        .ok_or(StoreError::Work(WorkDomainError::ContractRequired))?;
    let submission_id = run
        .triggering_submission_id
        .clone()
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    let criterion_ids = contract_criterion_ids_tx(transaction, contract_id)?;
    let review = command.review.clone();
    let review = review
        .normalized(&criterion_ids)
        .map_err(StoreError::Work)?;
    if review.task_id != task.task_id
        || review.contract_id != *contract_id
        || review.reviewer_run_id != run.run_id
        || review.reviewed_submission_id != submission_id
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let review_id = review
        .review_id
        .clone()
        .unwrap_or_else(|| allocate_id("review"));
    validate_namespace(&review_id, "review:", "review.review_id")?;
    if task.stage_behavior != WorkflowStageBehavior::Active {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    validate_review_lineage_tx(transaction, &run, &review, &submission_id)?;
    transaction.execute(
        "INSERT INTO task_reviews (review_id, task_id, contract_id, reviewer_run_id, reviewed_submission_id, review_attempt_index, supersedes_review_id, overall_verdict, human_gate_kind, overall_feedback) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            review_id,
            task.task_id.as_str(),
            contract_id.as_str(),
            run.run_id,
            submission_id,
            review.review_attempt_index,
            review.supersedes_review_id,
            review.overall_verdict.as_str(),
            review.human_gate_kind.map(|kind| kind.as_str()),
            review.overall_feedback,
        ],
    )?;
    let persisted_review_id: String = transaction.query_row(
        "SELECT review_id FROM task_reviews WHERE reviewer_run_id = ?1",
        [run.run_id.as_str()],
        |row| row.get(0),
    )?;
    for criterion in &review.criteria {
        transaction.execute(
            "INSERT INTO task_review_criteria (review_id, criterion_id, outcome, evidence_markdown, feedback) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![persisted_review_id, criterion.criterion_id, criterion.outcome.as_str(), criterion.evidence_markdown, criterion.feedback],
        )?;
    }
    transaction.execute(
        "UPDATE tasks SET latest_review_id = ?2, latest_submission_id = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?4",
        params![task.task_id.as_str(), persisted_review_id, submission_id, task.generation],
    )?;
    let _review_created_event = append_work_event_tx(
        transaction,
        run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
        WorkEventPayload::review_created(
            persisted_review_id.clone(),
            submission_id.clone(),
            contract_id.clone(),
            run.review_round,
            review.review_attempt_index,
            review.supersedes_review_id.clone(),
            review.overall_verdict,
        )
        .map_err(StoreError::Work)?,
    )?;
    match review.overall_verdict {
        TaskReviewVerdict::Approve => {
            mark_run_completed_tx(transaction, &run, &command.fence)?;
            let _completed_event = append_work_event_tx(
                transaction,
                run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
                WorkEventPayload::run_completed(
                    run.run_kind,
                    run.task_generation,
                    RunTerminalKind::Review,
                )
                .map_err(StoreError::Work)?,
            )?;
            let mut write = helpers::complete_review_tx(
                transaction,
                &mut task,
                &persisted_review_id,
                &submission_id,
                helpers::CommandEventContext {
                    actor_id,
                    causation_id,
                    correlation_id,
                },
                Some(&run.run_id),
            )?;
            write.run_id = Some(run.run_id);
            Ok(write)
        }
        TaskReviewVerdict::RequestChanges => {
            mark_run_completed_tx(transaction, &run, &command.fence)?;
            let _completed_event = append_work_event_tx(
                transaction,
                run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
                WorkEventPayload::run_completed(
                    run.run_kind,
                    run.task_generation,
                    RunTerminalKind::Review,
                )
                .map_err(StoreError::Work)?,
            )?;
            if run.review_round
                >= task_execution_policy_for_task(transaction, &task, contract_id)?
                    .max_review_rounds
            {
                let gate_id = insert_gate_tx(
                    transaction,
                    &task,
                    OpenGate {
                        gate_kind: TaskGateKind::Recovery,
                        prompt: &review.overall_feedback,
                        context: "Review rounds exhausted.",
                        suggested_answers: &[],
                        actor_id,
                        originating_run_id: Some(&run.run_id),
                        recovery_reason: Some(TaskRecoveryReason::ReviewRoundsExhausted),
                        retry_run_kind: Some(RunKind::Executor),
                    },
                )?;
                let _revision = bump_task_to_waiting_tx(transaction, &mut task)?;
                let _gate_event = append_work_event_tx(
                    transaction,
                    run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
                    WorkEventPayload::gate_opened(
                        gate_id.clone(),
                        task.generation,
                        TaskGateKind::Recovery,
                        Some(run.run_id.clone()),
                        Some(TaskRecoveryReason::ReviewRoundsExhausted),
                        Some(RunKind::Executor),
                    )
                    .map_err(StoreError::Work)?,
                )?;
                let mut event = append_work_event_tx(
                    transaction,
                    run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
                    WorkEventPayload::task_stage_changed(
                        task.revision,
                        task.generation,
                        WorkflowStageId::new("stage:personal:doing").map_err(StoreError::Work)?,
                        task.stage_id.clone(),
                        TaskStageChangeReason::GateOpened,
                    )
                    .map_err(StoreError::Work)?,
                )?;
                if let Some(notification_event) = enqueue_work_notification_tx(
                    transaction,
                    &event,
                    noema_tasks::NotificationKind::TaskRecovery,
                    &serde_json::json!({
                        "task_id": task.task_id.as_str(),
                        "gate_id": gate_id.as_str(),
                        "action_needed": true,
                    }),
                )? {
                    event = notification_event;
                }
                return Ok(helpers::task_write(event, task.task_id)
                    .contract(Some(contract_id.clone()))
                    .gate(Some(gate_id))
                    .run(Some(run.run_id)));
            }
            let revision = helpers::increment(task.revision, "task.revision")?;
            let changed = transaction.execute(
                "UPDATE tasks SET revision = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?3 AND revision = ?4",
                params![task.task_id.as_str(), revision, task.generation, task.revision],
            )?;
            if changed != 1 {
                return Err(StoreError::Work(WorkDomainError::StaleRevision));
            }
            task.revision = revision;
            let next_review_round = helpers::increment(run.review_round, "run.review_round")?;
            let (child_run_id, child_event) = helpers::queue_run_tx(
                transaction,
                service.provider_registry.as_ref(),
                &task,
                helpers::QueueRun {
                    run_kind: RunKind::Executor,
                    contract_id: Some(contract_id),
                    planner_complexity: None,
                    review_round: next_review_round,
                    attempt_index: 0,
                    parent_run_id: Some(&run.run_id),
                    triggering_submission_id: None,
                    triggering_review_id: Some(&persisted_review_id),
                    event: helpers::CommandEventContext {
                        actor_id,
                        causation_id,
                        correlation_id,
                    },
                },
            )?;
            let event = child_event;
            Ok(helpers::task_write(event, task.task_id)
                .contract(Some(contract_id.clone()))
                .run(Some(child_run_id)))
        }
        TaskReviewVerdict::NeedsHuman => {
            let gate_kind = review
                .human_gate_kind
                .ok_or(StoreError::Work(WorkDomainError::GateRequired))?;
            let gate_id = insert_gate_tx(
                transaction,
                &task,
                OpenGate {
                    gate_kind,
                    prompt: &review.overall_feedback,
                    context: "Reviewer requested human input.",
                    suggested_answers: &[],
                    actor_id,
                    originating_run_id: Some(&run.run_id),
                    recovery_reason: None,
                    retry_run_kind: None,
                },
            )?;
            let _revision = bump_task_to_waiting_tx(transaction, &mut task)?;
            mark_run_waiting_tx(transaction, &run, &command.fence)?;
            let _waiting_event = append_work_event_tx(
                transaction,
                run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
                WorkEventPayload::run_waiting_for_approval(
                    run.run_kind,
                    run.task_generation,
                    gate_id.clone(),
                    gate_kind,
                )
                .map_err(StoreError::Work)?,
            )?;
            let _gate_event = append_work_event_tx(
                transaction,
                run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
                WorkEventPayload::gate_opened(
                    gate_id.clone(),
                    task.generation,
                    gate_kind,
                    Some(run.run_id.clone()),
                    None,
                    None,
                )
                .map_err(StoreError::Work)?,
            )?;
            let mut event = append_work_event_tx(
                transaction,
                run_scope(&task, (actor_id, causation_id, correlation_id), &run.run_id),
                WorkEventPayload::task_stage_changed(
                    task.revision,
                    task.generation,
                    WorkflowStageId::new("stage:personal:doing").map_err(StoreError::Work)?,
                    task.stage_id.clone(),
                    TaskStageChangeReason::GateOpened,
                )
                .map_err(StoreError::Work)?,
            )?;
            if let Some(notification_event) = enqueue_work_notification_tx(
                transaction,
                &event,
                noema_tasks::NotificationKind::TaskWaiting,
                &serde_json::json!({
                    "task_id": task.task_id.as_str(),
                    "gate_id": gate_id.as_str(),
                    "action_needed": true,
                }),
            )? {
                event = notification_event;
            }
            Ok(helpers::task_write(event, task.task_id)
                .contract(Some(contract_id.clone()))
                .gate(Some(gate_id))
                .run(Some(run.run_id)))
        }
    }
}

pub(super) fn replay_tx(
    transaction: &Transaction<'_>,
    command: &SubmitTaskReview,
) -> Result<Option<helpers::CommandWrite>, StoreError> {
    let run = load_terminal_run_identity_tx(transaction, &command.fence, Some(RunKind::Reviewer))?;
    let contract_id = command
        .fence
        .contract_id
        .as_ref()
        .ok_or(StoreError::Work(WorkDomainError::ContractRequired))?;
    let submission_id = run
        .triggering_submission_id
        .clone()
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    let criterion_ids = contract_criterion_ids_tx(transaction, contract_id)?;
    let review = command
        .review
        .clone()
        .normalized(&criterion_ids)
        .map_err(StoreError::Work)?;
    if review.task_id != run.task_id
        || review.contract_id != *contract_id
        || review.reviewer_run_id != run.run_id
        || review.reviewed_submission_id != submission_id
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let existing_review_id: Option<String> = transaction
        .query_row(
            "SELECT review_id FROM task_reviews WHERE reviewer_run_id = ?1",
            [run.run_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(existing_review_id) = existing_review_id else {
        return Ok(None);
    };
    if review
        .review_id
        .as_deref()
        .is_some_and(|id| id != existing_review_id)
        || !review_matches_tx(transaction, &existing_review_id, &review)?
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    let (event, gate_id, result_run_id) =
        replay_review_branch_tx(transaction, &run, &run.task_id, &review)?;
    Ok(Some(
        helpers::task_write(event, run.task_id)
            .contract(Some(contract_id.clone()))
            .gate(gate_id)
            .run(Some(result_run_id)),
    ))
}

fn replay_review_branch_tx(
    transaction: &Transaction<'_>,
    run: &noema_tasks::AgentRunRecord,
    task_id: &noema_tasks::TaskId,
    review: &noema_tasks::NewTaskReview,
) -> Result<
    (
        noema_tasks::WorkEventRecord,
        Option<noema_tasks::TaskGateId>,
        String,
    ),
    StoreError,
> {
    let notification = |kind| {
        notification_queued_event_for_run_tx(transaction, &run.run_id, kind)?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: format!(
                    "review run {} has no original notification marker",
                    run.run_id
                ),
            }
        })
    };
    match review.overall_verdict {
        TaskReviewVerdict::Approve => Ok((
            notification(noema_tasks::NotificationKind::TaskCompleted)?,
            None,
            run.run_id.clone(),
        )),
        TaskReviewVerdict::NeedsHuman => {
            let gate_id = replay_gate_id_tx(transaction, &run.run_id, task_id)?;
            Ok((
                notification(noema_tasks::NotificationKind::TaskWaiting)?,
                Some(gate_id),
                run.run_id.clone(),
            ))
        }
        TaskReviewVerdict::RequestChanges => {
            let recovery_gate: Option<String> = transaction
                .query_row(
                    "SELECT gate_id FROM task_gates WHERE originating_run_id = ?1 AND task_id = ?2 AND gate_kind = 'recovery' AND recovery_reason = 'review_rounds_exhausted' AND retry_run_kind = 'executor' ORDER BY opened_at, gate_id LIMIT 1",
                    params![run.run_id, task_id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(gate_id) = recovery_gate {
                return Ok((
                    notification(noema_tasks::NotificationKind::TaskRecovery)?,
                    Some(noema_tasks::TaskGateId::new(gate_id).map_err(StoreError::Work)?),
                    run.run_id.clone(),
                ));
            }
            let (child_run_id, child_event) = child_run_event_tx(transaction, &run.run_id)?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("review run {} has no committed change branch", run.run_id),
                })?;
            Ok((child_event, None, child_run_id))
        }
    }
}

fn validate_review_lineage_tx(
    transaction: &Transaction<'_>,
    run: &noema_tasks::AgentRunRecord,
    review: &noema_tasks::NewTaskReview,
    submission_id: &str,
) -> Result<(), StoreError> {
    let submission_round: i64 = transaction
        .query_row(
            "SELECT review_round FROM task_submissions WHERE submission_id = ?1",
            [submission_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    if i64::from(run.review_round) != submission_round {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    let previous: Option<(String, i64, String)> = transaction
        .query_row(
            "SELECT review_id, review_attempt_index, overall_verdict FROM task_reviews WHERE reviewed_submission_id = ?1 ORDER BY review_attempt_index DESC, review_id DESC LIMIT 1",
            [submission_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let expected_attempt = previous
        .as_ref()
        .map_or(1, |(_, attempt, _)| attempt.saturating_add(1));
    let expected_supersedes = run.triggering_review_id.as_deref().or_else(|| {
        previous.as_ref().and_then(|(review_id, _, verdict)| {
            (verdict == TaskReviewVerdict::NeedsHuman.as_str()).then_some(review_id.as_str())
        })
    });
    if i64::from(review.review_attempt_index) != expected_attempt
        || review.supersedes_review_id.as_deref() != expected_supersedes
    {
        return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
    }
    Ok(())
}

fn replay_gate_id_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
    task_id: &noema_tasks::TaskId,
) -> Result<noema_tasks::TaskGateId, StoreError> {
    let gate_id: String = transaction
        .query_row(
            "SELECT gate_id FROM task_gates WHERE originating_run_id = ?1 AND task_id = ?2 ORDER BY rowid LIMIT 1",
            params![run_id, task_id.as_str()],
            |row| row.get(0),
        )
        .map_err(StoreError::Sqlite)?;
    noema_tasks::TaskGateId::new(gate_id).map_err(StoreError::Work)
}

fn review_matches_tx(
    transaction: &Transaction<'_>,
    review_id: &str,
    review: &noema_tasks::NewTaskReview,
) -> Result<bool, StoreError> {
    let Some((task_id, contract_id, reviewer_run_id, submission_id, attempt, supersedes, verdict, gate_kind, feedback)) = transaction.query_row(
        "SELECT task_id, contract_id, reviewer_run_id, reviewed_submission_id, review_attempt_index, supersedes_review_id, overall_verdict, human_gate_kind, overall_feedback FROM task_reviews WHERE review_id = ?1",
        [review_id],
        |row| Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
            row.get::<_, String>(3)?, row.get::<_, i64>(4)?, row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?, row.get::<_, Option<String>>(7)?, row.get::<_, String>(8)?,
        )),
    ).optional()? else { return Ok(false); };
    if task_id != review.task_id.as_str()
        || contract_id != review.contract_id.as_str()
        || reviewer_run_id != review.reviewer_run_id
        || submission_id != review.reviewed_submission_id
        || u32::try_from(attempt).ok() != Some(review.review_attempt_index)
        || supersedes != review.supersedes_review_id
        || verdict != review.overall_verdict.as_str()
        || gate_kind.as_deref() != review.human_gate_kind.map(|kind| kind.as_str())
        || feedback != review.overall_feedback
    {
        return Ok(false);
    }
    let stored_criteria = transaction
        .prepare("SELECT criterion_id, outcome, evidence_markdown, feedback FROM task_review_criteria WHERE review_id = ?1 ORDER BY criterion_id")?
        .query_map([review_id], |row| Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?,
        )))?
        .collect::<Result<Vec<_>, _>>()?;
    let expected_criteria = review
        .criteria
        .iter()
        .map(|criterion| {
            (
                criterion.criterion_id.clone(),
                criterion.outcome.as_str().to_string(),
                criterion.evidence_markdown.clone(),
                criterion.feedback.clone(),
            )
        })
        .collect::<Vec<_>>();
    Ok(stored_criteria == expected_criteria)
}
