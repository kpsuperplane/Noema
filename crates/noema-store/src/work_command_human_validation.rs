//! Transaction-local validation for human gates and approved review lineage.

use noema_tasks::{GateResolutionKind, WorkDomainError};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{StoreError, work_commands::helpers};

pub(super) fn validate_gate_resolution(
    gate: &noema_tasks::TaskGateRecord,
    resolution: GateResolutionKind,
) -> Result<(), StoreError> {
    if gate
        .kind
        .allows_resolution(gate.recovery_reason, gate.retry_run_kind, resolution)
    {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::InvalidTransition))
    }
}

/// Verify that the task's latest review is an approval for its current,
/// current-generation contract and submission, with exact all-pass coverage.
pub(super) fn validate_current_approval_tx(
    transaction: &Transaction<'_>,
    task: &helpers::TaskState,
    review_id: &str,
    submission_id: &str,
    contract_id: &noema_tasks::TaskContractId,
) -> Result<(), StoreError> {
    let generation = i64::try_from(task.generation).map_err(|_| {
        StoreError::Work(noema_tasks::WorkDomainError::InvalidInput {
            field: "task.generation",
            message: "generation is outside SQLite's integer range".to_string(),
        })
    })?;
    let linked = transaction
        .query_row(
            "SELECT 1
               FROM task_reviews AS review
               JOIN task_execution_contracts AS contract
                 ON contract.contract_id = review.contract_id
                AND contract.task_id = review.task_id
                AND contract.task_generation = ?5
               JOIN task_submissions AS submission
                 ON submission.submission_id = review.reviewed_submission_id
                AND submission.task_id = review.task_id
                AND submission.contract_id = review.contract_id
              WHERE review.review_id = ?1
                AND review.task_id = ?2
                AND review.contract_id = ?3
                AND review.reviewed_submission_id = ?4
                AND review.overall_verdict = 'approve'",
            params![
                review_id,
                task.task_id.as_str(),
                contract_id.as_str(),
                submission_id,
                generation
            ],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if linked != Some(1) {
        return Err(StoreError::Work(
            noema_tasks::WorkDomainError::ReviewNotApproved,
        ));
    }
    let (expected, covered, total, passed): (i64, i64, i64, i64) = transaction.query_row(
        "SELECT
             (SELECT COUNT(*) FROM task_contract_criteria WHERE contract_id = ?1),
             (SELECT COUNT(*)
                FROM task_review_criteria AS criterion
                JOIN task_contract_criteria AS expected
                  ON expected.criterion_id = criterion.criterion_id
                 AND expected.contract_id = ?1
               WHERE criterion.review_id = ?2),
             (SELECT COUNT(*) FROM task_review_criteria WHERE review_id = ?2),
             (SELECT COUNT(*)
                FROM task_review_criteria AS criterion
                JOIN task_contract_criteria AS expected
                  ON expected.criterion_id = criterion.criterion_id
                 AND expected.contract_id = ?1
               WHERE criterion.review_id = ?2 AND criterion.outcome = 'pass')",
        params![contract_id.as_str(), review_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    if expected == 0 || expected != covered || expected != total || expected != passed {
        return Err(StoreError::Work(
            noema_tasks::WorkDomainError::ReviewNotApproved,
        ));
    }
    Ok(())
}
