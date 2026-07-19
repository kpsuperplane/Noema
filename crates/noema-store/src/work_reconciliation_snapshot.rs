//! Read-consistent durable snapshot derivation for Work reconciliation.

use std::str::FromStr;

use noema_tasks::{
    AgentRunRecord, RunKind, RunStatus, SafeErrorCode, TaskGateKind, TaskGateState, TaskId,
    TaskReviewVerdict, WorkEventKind, WorkEventPayload, WorkFailedRunFacts,
    WorkReconciliationSnapshot, reported_failure_facts,
};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{
    NoemaStore, StoreError, WorkReconciliationEnvelope,
    work_reads::{
        rows::load_gate,
        task::{LoadedWorkTask, load_task_facts},
    },
    work_runs::rows::load_run_tx,
};

impl NoemaStore {
    /// Load the bounded durable envelope consumed by the pure reconciler.
    /// Process state, provider output, and transcript text never participate.
    ///
    /// # Errors
    ///
    /// Returns an error when durable task, run, gate, submission, or review
    /// facts are inconsistent or cannot be read.
    pub async fn load_work_reconciliation_snapshot(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<WorkReconciliationEnvelope>, StoreError> {
        let task_id = task_id.clone();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let Some(facts) = load_task_facts(&transaction, &task_id)? else {
                return Ok(None);
            };
            derive_envelope(&transaction, facts).map(Some)
        })
        .await
    }
}

pub(super) fn derive_envelope(
    transaction: &Transaction<'_>,
    facts: LoadedWorkTask,
) -> Result<WorkReconciliationEnvelope, StoreError> {
    let runnable_run = load_runnable_run(transaction, &facts)?;
    let resolved_gate_resume_run_kind = if facts.active_gate.is_none() {
        load_resolved_gate_resume_kind(transaction, &facts)?
    } else {
        None
    };
    let contract = facts.current_contract.as_ref();
    let submission = facts.latest_submission.as_ref();
    let review = facts.latest_review.as_ref();
    let review_matches_submission = review.zip(submission).is_some_and(|(review, submission)| {
        contract.is_some_and(|contract| review.contract_id == contract.contract_id)
            && review.reviewed_submission_id == submission.submission_id
    });
    let submission_has_review = submission
        .map(|submission| {
            has_review_of_submission(transaction, &facts, submission.submission_id.as_str())
        })
        .transpose()?
        .unwrap_or(false);
    if submission_has_review && !review_matches_submission {
        return Err(invariant(
            "latest submission review is not reflected by the current review pointer",
        ));
    }
    let approved_review = review_matches_submission
        && review.is_some_and(|review| review.overall_verdict == TaskReviewVerdict::Approve);
    let review_requested_changes = review_matches_submission
        && review.is_some_and(|review| review.overall_verdict == TaskReviewVerdict::RequestChanges);
    let submission_waiting_for_review = submission.is_some_and(|submission| {
        contract.is_some_and(|contract| submission.contract_id == contract.contract_id)
    }) && !submission_has_review
        && runnable_run
            .as_ref()
            .is_none_or(|run| run.run_kind != RunKind::Reviewer);
    let review_rounds_exhausted = review_requested_changes
        && submission
            .zip(contract)
            .is_some_and(|(submission, contract)| {
                submission.review_round >= contract.execution_policy.max_review_rounds
            });
    let failed_run = facts
        .latest_run
        .as_ref()
        .filter(|run| matches!(run.status, RunStatus::Interrupted | RunStatus::Failed))
        .map(|run| load_failed_run_facts(transaction, run))
        .transpose()?;
    let snapshot = WorkReconciliationSnapshot {
        stage_behavior: facts.stage.system_behavior,
        has_current_contract: facts.current_contract.is_some(),
        has_open_gate: facts.active_gate.as_ref().is_some_and(|gate| {
            gate.task_generation == facts.task.generation && gate.state == TaskGateState::Open
        }),
        has_runnable_run: runnable_run.is_some(),
        resolved_gate_resume_run_kind,
        approved_review,
        // Planner completion creates the contract, completes the Planner, and
        // queues the Executor in one transaction; the schema has no plan-only row.
        planner_plan_ready: false,
        submission_waiting_for_review,
        review_requested_changes,
        review_rounds_exhausted,
        failed_run,
    };
    Ok(WorkReconciliationEnvelope {
        task: facts.task,
        stage: facts.stage,
        current_contract: facts.current_contract,
        active_gate: facts.active_gate,
        latest_run: facts.latest_run,
        latest_submission: facts.latest_submission,
        latest_review: facts.latest_review,
        snapshot,
    })
}

fn load_runnable_run(
    transaction: &Transaction<'_>,
    facts: &LoadedWorkTask,
) -> Result<Option<AgentRunRecord>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT run_id FROM agent_runs
         WHERE task_id = ?1 AND task_generation = ?2
           AND status IN ('queued', 'leased', 'running')
         ORDER BY created_at DESC, run_id DESC LIMIT 2",
    )?;
    let run_ids = statement
        .query_map(
            params![facts.task.task_id.as_str(), facts.task.generation],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if run_ids.len() > 1 {
        return Err(invariant(
            "task has more than one runnable current-generation run",
        ));
    }
    let run = run_ids
        .first()
        .map(|run_id| {
            load_run_tx(transaction, run_id)?.ok_or_else(|| invariant("runnable run is missing"))
        })
        .transpose()?;
    if run.as_ref().is_some_and(|run| {
        run.task_id != facts.task.task_id || run.task_generation != facts.task.generation
    }) {
        return Err(invariant("runnable run crosses the current task fence"));
    }
    Ok(run)
}

fn load_resolved_gate_resume_kind(
    transaction: &Transaction<'_>,
    facts: &LoadedWorkTask,
) -> Result<Option<RunKind>, StoreError> {
    let gate_id = transaction
        .query_row(
            "SELECT gate_id FROM task_gates
             WHERE task_id = ?1 AND task_generation = ?2 AND gate_state = 'resolved'
             ORDER BY resolved_at DESC, gate_id DESC LIMIT 1",
            params![facts.task.task_id.as_str(), facts.task.generation],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(gate_id) = gate_id else {
        return Ok(None);
    };
    let gate_id = noema_tasks::TaskGateId::new(gate_id).map_err(StoreError::Work)?;
    let gate = load_gate(transaction, &gate_id)?;
    let contract_id = facts
        .current_contract
        .as_ref()
        .map(|contract| &contract.contract_id);
    if gate.state != TaskGateState::Resolved
        || gate.task_id != facts.task.task_id
        || gate.task_generation != facts.task.generation
        || gate.contract_id.as_ref() != contract_id
    {
        return Err(invariant("resolved gate crosses the current task fence"));
    }
    match gate.kind {
        TaskGateKind::Recovery => Ok(gate.retry_run_kind),
        TaskGateKind::Clarification | TaskGateKind::Approval => {
            let originating_run_id = gate
                .originating_run_id
                .as_deref()
                .ok_or_else(|| invariant("resolved gate has no originating run"))?;
            let run = load_run_tx(transaction, originating_run_id)?
                .ok_or_else(|| invariant("resolved gate originating run is missing"))?;
            if run.task_id != facts.task.task_id
                || run.task_generation != facts.task.generation
                || run.contract_id != gate.contract_id
            {
                return Err(invariant("resolved gate crosses its originating run fence"));
            }
            Ok(Some(run.run_kind))
        }
    }
}

fn has_review_of_submission(
    transaction: &Transaction<'_>,
    facts: &LoadedWorkTask,
    submission_id: &str,
) -> Result<bool, StoreError> {
    let contract_id = facts
        .current_contract
        .as_ref()
        .map(|contract| contract.contract_id.as_str());
    transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM task_reviews
                WHERE reviewed_submission_id = ?1 AND task_id = ?2 AND contract_id = ?3
             )",
            params![submission_id, facts.task.task_id.as_str(), contract_id],
            |row| row.get(0),
        )
        .map_err(StoreError::Sqlite)
}

fn load_failed_run_facts(
    transaction: &Transaction<'_>,
    run: &AgentRunRecord,
) -> Result<WorkFailedRunFacts, StoreError> {
    let expected_kind = if run.status == RunStatus::Interrupted {
        WorkEventKind::RunInterrupted
    } else {
        WorkEventKind::RunFailed
    };
    let mut statement = transaction.prepare(
        "SELECT event_kind, payload_json FROM work_events
         WHERE run_id = ?1 AND event_kind = ?2
         ORDER BY event_sequence DESC LIMIT 2",
    )?;
    let events = statement
        .query_map(params![run.run_id, expected_kind.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if events.len() != 1 {
        return Err(invariant(
            "failed run does not have exactly one matching failure event",
        ));
    }
    let kind = WorkEventKind::from_str(&events[0].0).map_err(StoreError::Work)?;
    let value = serde_json::from_str(&events[0].1)?;
    let payload = WorkEventPayload::from_persisted(kind, value).map_err(StoreError::Work)?;
    let object = payload
        .as_value()
        .as_object()
        .ok_or_else(|| invariant("failure event payload is not an object"))?;
    let event_run_kind = field_str(object, "run_kind")
        .and_then(|value| RunKind::from_str(value).map_err(StoreError::Work))?;
    let generation = field_u64(object, "generation")?;
    let attempt_index = u32::try_from(field_u64(object, "attempt_index")?)
        .map_err(|_| invariant("failure event attempt index exceeds u32"))?;
    let error_code = field_str(object, "error_code")?;
    let retryable = object
        .get("retryable")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| invariant("failure event is missing retryable"))?;
    if kind != expected_kind
        || event_run_kind != run.run_kind
        || generation != run.task_generation
        || attempt_index != run.attempt_index
        || run.error_code.as_deref() != Some(error_code)
    {
        return Err(invariant("failure event crosses its run fence"));
    }
    let error_code = SafeErrorCode::new(error_code).map_err(StoreError::Work)?;
    Ok(reported_failure_facts(
        run.run_kind,
        run.status,
        &error_code,
        retryable,
        retryable && run.attempt_index >= run.execution_policy.max_automatic_retries,
    ))
}

fn field_str<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<&'a str, StoreError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invariant("failure event is missing a required string"))
}

fn field_u64(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<u64, StoreError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| invariant("failure event is missing a required integer"))
}

fn invariant(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
