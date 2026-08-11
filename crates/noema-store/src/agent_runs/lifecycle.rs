use std::collections::BTreeSet;

use noema_tasks::{
    AgentRunRecord, RunKind, RunStatus, TaskGateKind, TaskRecoveryReason, WorkDomainError,
    WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::super::{
    PlanTerminal, WorkRunFence,
    rows::{self, load_run_tx},
};
use crate::{
    StoreError,
    ids::allocate_id,
    run_items::finish_agent_run_records_tx,
    work_commands::helpers,
    work_events::{WORK_EVENT_COLUMNS, WorkEventScope, decode_work_event_record},
};

pub(super) struct PlannerTerminalReplay {
    pub event: noema_tasks::WorkEventRecord,
    pub contract_id: Option<noema_tasks::TaskContractId>,
    pub gate_id: Option<noema_tasks::TaskGateId>,
    pub run_id: String,
}

pub(super) fn load_running_fence_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
    expected_kind: RunKind,
) -> Result<(AgentRunRecord, helpers::TaskState), StoreError> {
    let (run, task) = load_fenced_run_tx(transaction, fence, Some(expected_kind), false)?;
    if run.status != RunStatus::Running {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    Ok((run, task))
}

/// Load only the immutable run identity needed to recognize a committed
/// terminal replay. Current task generation and lease checks belong to fresh
/// writes and happen only after replay lookup misses.
pub(super) fn load_terminal_run_identity_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
    expected_kind: Option<RunKind>,
) -> Result<AgentRunRecord, StoreError> {
    let run = load_run_tx(transaction, &fence.run_id)?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    if run.task_generation != fence.task_generation {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    if run.contract_id != fence.contract_id {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    if expected_kind.is_some_and(|kind| run.run_kind != kind) {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    Ok(run)
}

/// Recognize a committed Planner terminal after its lease was cleared. The
/// immutable contract or originating gate is the idempotency record; a live
/// lease is still mandatory when neither durable terminal shape exists.
pub(super) fn replay_plan_terminal_tx(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
    terminal: &PlanTerminal,
) -> Result<Option<PlannerTerminalReplay>, StoreError> {
    let complete_child: Option<(String, String)> = transaction
        .query_row(
            "SELECT run_id, contract_id FROM agent_runs WHERE parent_run_id = ?1 AND run_kind = 'executor' AND contract_id IS NOT NULL ORDER BY created_at, run_id LIMIT 1",
            [run.run_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let gate_id: Option<String> = transaction
        .query_row(
            "SELECT gate_id FROM task_gates WHERE originating_run_id = ?1 ORDER BY opened_at, gate_id LIMIT 1",
            [run.run_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if complete_child.is_none() && gate_id.is_none() {
        return Ok(None);
    }
    match (terminal, complete_child, gate_id) {
        (PlanTerminal::Complete(plan), Some((child_run_id, contract_id)), None) => {
            if !complete_plan_matches_tx(transaction, run, &contract_id, plan)? {
                return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
            }
            Ok(Some(PlannerTerminalReplay {
                event: run_queued_event_tx(transaction, &child_run_id)?,
                contract_id: Some(
                    noema_tasks::TaskContractId::new(contract_id).map_err(StoreError::Work)?,
                ),
                gate_id: None,
                run_id: child_run_id,
            }))
        }
        (
            PlanTerminal::BlockingQuestion {
                prompt_markdown,
                context_markdown,
                suggested_answers,
                gate_kind,
            },
            None,
            Some(gate_id),
        ) => {
            if !blocking_plan_matches_tx(
                transaction,
                run,
                &gate_id,
                prompt_markdown,
                context_markdown,
                suggested_answers,
                *gate_kind,
            )? {
                return Err(StoreError::Work(WorkDomainError::IdempotencyConflict));
            }
            let event = notification_queued_event_for_run_tx(
                transaction,
                &run.run_id,
                noema_tasks::NotificationKind::TaskWaiting,
            )?
            .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
            Ok(Some(PlannerTerminalReplay {
                event,
                contract_id: None,
                gate_id: Some(noema_tasks::TaskGateId::new(gate_id).map_err(StoreError::Work)?),
                run_id: run.run_id.clone(),
            }))
        }
        _ => Err(StoreError::Work(WorkDomainError::IdempotencyConflict)),
    }
}

fn complete_plan_matches_tx(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
    contract_id: &str,
    plan: &crate::CompletePlan,
) -> Result<bool, StoreError> {
    let Some((task_id, generation, origin, request, execution_plan, complexity, supersedes)) =
        transaction
            .query_row(
                "SELECT task_id, task_generation, origin, request_markdown, execution_plan_markdown, complexity, supersedes_contract_id FROM task_execution_contracts WHERE contract_id = ?1",
                [contract_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .optional()?
    else {
        return Ok(false);
    };
    if task_id != run.task_id.as_str()
        || u64::try_from(generation).ok() != Some(run.task_generation)
        || origin != "planned"
        || request != plan.request_markdown
        || execution_plan.as_deref() != Some(plan.execution_plan_markdown.as_str())
        || complexity != plan.complexity.as_str()
        || supersedes.is_some()
    {
        return Ok(false);
    }
    let stored = transaction
        .prepare(
            "SELECT criterion_id, ordinal, description, expected_evidence FROM task_contract_criteria WHERE contract_id = ?1 ORDER BY ordinal, criterion_id",
        )?
        .query_map([contract_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let expected = normalize_new_criteria_for_store(&plan.criteria)?;
    Ok(stored.len() == expected.len()
        && stored.iter().zip(expected.iter()).all(
            |((stored_id, ordinal, description, evidence), expected)| {
                expected
                    .criterion_id
                    .as_deref()
                    .is_none_or(|expected_id| expected_id == stored_id)
                    && u32::try_from(*ordinal).ok() == Some(expected.ordinal)
                    && description == &expected.description
                    && evidence == &expected.expected_evidence
            },
        ))
}

fn blocking_plan_matches_tx(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
    gate_id: &str,
    prompt: &str,
    context: &str,
    suggested_answers: &[String],
    gate_kind: TaskGateKind,
) -> Result<bool, StoreError> {
    let suggested_answers_json = serde_json::to_string(suggested_answers)?;
    transaction
        .query_row(
            "SELECT task_id = ?2 AND task_generation = ?3 AND contract_id IS NULL AND gate_kind = ?4 AND recovery_reason IS NULL AND retry_run_kind IS NULL AND prompt_markdown = ?5 AND context_markdown = ?6 AND suggested_answers_json = ?7 AND originating_run_id = ?8 FROM task_gates WHERE gate_id = ?1",
            params![gate_id, run.task_id.as_str(), run.task_generation, gate_kind.as_str(), prompt, context, suggested_answers_json, run.run_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
        .map_err(StoreError::Sqlite)
}

pub(super) fn load_fenced_run_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
    expected_kind: Option<RunKind>,
    allow_expired_lease: bool,
) -> Result<(AgentRunRecord, helpers::TaskState), StoreError> {
    let run = load_run_tx(transaction, &fence.run_id)?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    if run.task_generation != fence.task_generation
        || run.contract_id != fence.contract_id
        || run.lease_token.as_deref() != Some(fence.lease_token.as_str())
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let lease_valid: bool = transaction.query_row(
        if allow_expired_lease {
            "SELECT lease_expires_at IS NOT NULL AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM agent_runs WHERE run_id = ?1 AND lease_token = ?2"
        } else {
            "SELECT lease_expires_at IS NOT NULL AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM agent_runs WHERE run_id = ?1 AND lease_token = ?2"
        },
        params![fence.run_id.as_str(), fence.lease_token.as_str()],
        |row| row.get(0),
    )?;
    if !lease_valid {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    if expected_kind.is_some_and(|kind| run.run_kind != kind) {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let task = helpers::load_task_state_tx(transaction, &run.task_id)?;
    if task.generation != fence.task_generation {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    Ok((run, task))
}

pub(super) fn mark_run_completed_tx(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
    fence: &WorkRunFence,
) -> Result<(), StoreError> {
    rows::execute_fenced_update_tx(
        transaction,
        "UPDATE agent_runs SET status = 'completed', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'running' AND lease_token = ?2 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AND task_generation = ?3 AND (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) = ?3 AND cancellation_requested = 0",
        params![run.run_id, fence.lease_token, fence.task_generation],
    )?;
    finish_agent_run_records_tx(transaction, &run.run_id, RunStatus::Completed)
}

pub(super) fn mark_run_waiting_tx(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
    fence: &WorkRunFence,
) -> Result<(), StoreError> {
    rows::execute_fenced_update_tx(
        transaction,
        "UPDATE agent_runs SET status = 'waiting_for_approval', lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, ended_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'running' AND lease_token = ?2 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AND task_generation = ?3 AND (SELECT generation FROM tasks WHERE task_id = agent_runs.task_id) = ?3 AND cancellation_requested = 0",
        params![run.run_id, fence.lease_token, fence.task_generation],
    )
}

pub(super) fn bump_task_to_waiting_tx(
    transaction: &Transaction<'_>,
    task: &mut helpers::TaskState,
) -> Result<u64, StoreError> {
    let revision = helpers::increment(task.revision, "task.revision")?;
    let changed = transaction.execute(
        "UPDATE tasks SET stage_id = 'stage:personal:waiting', active_gate_id = (SELECT gate_id FROM task_gates WHERE task_id = ?1 AND task_generation = ?2 AND gate_state = 'open'), queued_at = NULL, revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND generation = ?2 AND revision = ?4",
        params![task.task_id.as_str(), task.generation, revision, task.revision],
    )?;
    if changed != 1 {
        return Err(StoreError::Work(WorkDomainError::StaleRevision));
    }
    task.stage_id = WorkflowStageId::new("stage:personal:waiting").map_err(StoreError::Work)?;
    task.stage_behavior = WorkflowStageBehavior::HumanGate;
    task.active_gate_id = transaction.query_row("SELECT gate_id FROM task_gates WHERE task_id = ?1 AND task_generation = ?2 AND gate_state = 'open'", params![task.task_id.as_str(), task.generation], |row| row.get::<_, String>(0)).optional()?.map(noema_tasks::TaskGateId::new).transpose().map_err(StoreError::Work)?;
    task.revision = revision;
    Ok(revision)
}

pub(super) struct OpenGate<'a> {
    pub gate_kind: TaskGateKind,
    pub prompt: &'a str,
    pub context: &'a str,
    pub suggested_answers: &'a [String],
    pub actor_id: &'a str,
    pub originating_run_id: Option<&'a str>,
    pub recovery_reason: Option<TaskRecoveryReason>,
    pub retry_run_kind: Option<RunKind>,
}

pub(super) fn insert_gate_tx(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
    request: OpenGate<'_>,
) -> Result<noema_tasks::TaskGateId, StoreError> {
    let gate_id = noema_tasks::TaskGateId::new(allocate_id("gate")).map_err(StoreError::Work)?;
    let suggested_answers_json = serde_json::to_string(request.suggested_answers)?;
    transaction.execute(
        "INSERT INTO task_gates (gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, context_markdown, suggested_answers_json, opened_by_actor_id, originating_run_id) VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![gate_id.as_str(), task.task_id.as_str(), task.generation, task.current_contract_id.as_ref().map(ToString::to_string), request.gate_kind.as_str(), request.recovery_reason.map(|value| value.as_str()), request.retry_run_kind.map(|value| value.as_str()), request.prompt, request.context, suggested_answers_json, request.actor_id, request.originating_run_id],
    )?;
    Ok(gate_id)
}

pub(super) fn contract_criterion_ids_tx(
    transaction: &Transaction<'_>,
    contract_id: &noema_tasks::TaskContractId,
) -> Result<Vec<String>, StoreError> {
    let mut statement = transaction.prepare("SELECT criterion_id FROM task_contract_criteria WHERE contract_id = ?1 ORDER BY ordinal, criterion_id")?;
    let ids = statement
        .query_map([contract_id.as_str()], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty() {
        Err(StoreError::Work(WorkDomainError::ContractRequired))
    } else {
        Ok(ids)
    }
}

pub(super) fn normalize_new_criteria_for_store(
    values: &[noema_tasks::NewTaskValidationCriterion],
) -> Result<Vec<noema_tasks::NewTaskValidationCriterion>, StoreError> {
    if values.is_empty() {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "plan.criteria",
            message: "at least one criterion is required".to_string(),
        }));
    }
    let mut result = Vec::with_capacity(values.len());
    let mut ordinals = BTreeSet::new();
    let mut descriptions = BTreeSet::new();
    for value in values {
        let value = value.normalized().map_err(StoreError::Work)?;
        if !ordinals.insert(value.ordinal) || !descriptions.insert(value.description.clone()) {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "plan.criteria",
                message: "criteria ordinals and descriptions must be unique".to_string(),
            }));
        }
        result.push(value);
    }
    result.sort_by_key(|value| value.ordinal);
    Ok(result)
}

pub(super) fn task_execution_policy_for_task(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
    contract_id: &noema_tasks::TaskContractId,
) -> Result<noema_tasks::TaskExecutionPolicy, StoreError> {
    if task.current_contract_id.as_ref() != Some(contract_id) {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    let values = transaction.query_row("SELECT max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, max_automatic_retries, max_review_rounds FROM task_execution_contracts WHERE contract_id = ?1", [contract_id.as_str()], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?)))?;
    Ok(noema_tasks::TaskExecutionPolicy {
        max_provider_continuations: helpers::positive_u32(
            values.0,
            "contract.max_provider_continuations",
        )?,
        max_tool_calls: helpers::positive_u32(values.1, "contract.max_tool_calls")?,
        max_active_minutes: helpers::positive_u32(values.2, "contract.max_active_minutes")?,
        progress_audit_interval: helpers::positive_u32(
            values.3,
            "contract.progress_audit_interval",
        )?,
        max_automatic_retries: helpers::nonnegative_u32(
            values.4,
            "contract.max_automatic_retries",
        )?,
        max_review_rounds: helpers::positive_u32(values.5, "contract.max_review_rounds")?,
    })
}

pub(super) fn validate_namespace(
    value: &str,
    prefix: &str,
    field: &'static str,
) -> Result<(), StoreError> {
    if value.starts_with(prefix) {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::InvalidInput {
            field,
            message: format!("identifier must use the {prefix} namespace"),
        }))
    }
}

pub(super) fn latest_run_event_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<noema_tasks::WorkEventRecord, StoreError> {
    transaction
        .query_row(
            &format!("SELECT {WORK_EVENT_COLUMNS} FROM work_events WHERE run_id = ?1 ORDER BY event_sequence DESC LIMIT 1"),
            [run_id],
            decode_work_event_record,
        )
        .map_err(StoreError::Sqlite)
}

fn run_queued_event_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<noema_tasks::WorkEventRecord, StoreError> {
    transaction
        .query_row(
            &format!("SELECT {WORK_EVENT_COLUMNS} FROM work_events WHERE run_id = ?1 AND event_kind = 'run.queued' ORDER BY event_sequence LIMIT 1"),
            [run_id],
            decode_work_event_record,
        )
        .map_err(StoreError::Sqlite)
}

pub(super) fn notification_queued_event_for_run_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
    kind: noema_tasks::NotificationKind,
) -> Result<Option<noema_tasks::WorkEventRecord>, StoreError> {
    transaction
        .query_row(
            &format!("SELECT {WORK_EVENT_COLUMNS} FROM work_events WHERE run_id = ?1 AND event_kind = 'notification.queued' AND json_extract(payload_json, '$.notification_kind') = ?2 ORDER BY event_sequence LIMIT 1"),
            params![run_id, kind.as_str()],
            decode_work_event_record,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

pub(super) fn child_run_event_tx(
    transaction: &Transaction<'_>,
    parent_run_id: &str,
) -> Result<Option<(String, noema_tasks::WorkEventRecord)>, StoreError> {
    let child_run_id: Option<String> = transaction
        .query_row(
            "SELECT run_id FROM agent_runs WHERE parent_run_id = ?1 ORDER BY created_at, run_id LIMIT 1",
            [parent_run_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(child_run_id) = child_run_id else {
        return Ok(None);
    };
    let event = run_queued_event_tx(transaction, &child_run_id)?;
    Ok(Some((child_run_id, event)))
}

pub(super) fn submission_matches_tx(
    transaction: &Transaction<'_>,
    submission_id: &str,
    submission: &noema_tasks::NewTaskSubmission,
) -> Result<bool, StoreError> {
    let Some((task_id, contract_id, executor_run_id, review_round, summary, result_markdown)) =
        transaction
            .query_row(
                "SELECT task_id, contract_id, executor_run_id, review_round, summary, result_markdown FROM task_submissions WHERE submission_id = ?1",
                [submission_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()?
    else {
        return Ok(false);
    };
    if submission
        .submission_id
        .as_deref()
        .is_some_and(|id| id != submission_id)
        || task_id != submission.task_id.as_str()
        || contract_id != submission.contract_id.as_str()
        || executor_run_id != submission.executor_run_id
        || u32::try_from(review_round).ok() != Some(submission.review_round)
        || summary != submission.summary
        || result_markdown != submission.result_markdown
    {
        return Ok(false);
    }
    let stored_criteria = transaction
        .prepare(
            "SELECT criterion_id, evidence_markdown FROM task_submission_criteria WHERE submission_id = ?1 ORDER BY criterion_id",
        )?
        .query_map([submission_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let expected_criteria = submission
        .criteria
        .iter()
        .map(|criterion| {
            (
                criterion.criterion_id.clone(),
                criterion.evidence_markdown.clone(),
            )
        })
        .collect::<Vec<_>>();
    if stored_criteria != expected_criteria {
        return Ok(false);
    }
    let stored_citations = transaction
        .prepare(
            "SELECT title, url, start_index, end_index FROM task_submission_citations WHERE submission_id = ?1 ORDER BY ordinal",
        )?
        .query_map([submission_id], |row| {
            let start_index = row
                .get::<_, Option<i64>>(2)?
                .map(usize::try_from)
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?;
            let end_index = row
                .get::<_, Option<i64>>(3)?
                .map(usize::try_from)
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?;
            Ok(noema_tasks::TaskSubmissionCitation {
                title: row.get(0)?,
                url: row.get(1)?,
                start_index,
                end_index,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if stored_citations != submission.citations {
        return Ok(false);
    }
    let stored_artifacts = transaction
        .prepare(
            "SELECT artifact_id FROM task_submission_artifacts WHERE submission_id = ?1 ORDER BY ordinal",
        )?
        .query_map([submission_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stored_artifacts == submission.artifact_ids)
}

pub(super) fn scope(
    task: &helpers::TaskState,
    event: (&str, Option<&str>, &str),
    run_id: Option<&str>,
) -> WorkEventScope {
    let (actor_id, causation_id, correlation_id) = event;
    rows::event_scope(task, run_id, actor_id, causation_id, correlation_id)
}

pub(super) fn run_scope(
    task: &helpers::TaskState,
    event: (&str, Option<&str>, &str),
    run_id: &str,
) -> WorkEventScope {
    scope(task, event, Some(run_id))
}
