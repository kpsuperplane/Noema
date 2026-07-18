use noema_providers::ProviderRegistry;
use noema_tasks::{
    NewTaskReview, RunKind, TASK_EXECUTOR_AGENT_ID, TaskRecord, TaskReviewCriterion,
    TaskReviewVerdict, TaskStatus, plan_review,
};
use rusqlite::{OptionalExtension, params};

use super::{
    events::{append_run_event_tx, append_task_event_tx},
    lifecycle::task_domain_error,
    provider_selection::pool_selection_tx,
};
use crate::{NoemaStore, StoreError, ids::allocate_id, provider_selections::prove_selection_ready};

impl NoemaStore {
    /// Commit a review while proving any newly queued executor is registered
    /// and ready through commit.
    pub async fn create_task_review_with_readiness(
        &self,
        input: NewTaskReview,
        lease_token: &str,
        registry: &ProviderRegistry,
    ) -> Result<TaskRecord, StoreError> {
        if lease_token.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "task review requires the active run lease".to_string(),
            });
        }
        let task =
            self.get_task(&input.task_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task not found: {}", input.task_id),
                })?;
        let criteria = self.list_task_validation_criteria(&task.task_id).await?;
        let expected_criterion_ids = criteria
            .iter()
            .map(|criterion| criterion.criterion_id.clone())
            .collect::<Vec<_>>();
        let input = input
            .normalized(&expected_criterion_ids)
            .map_err(task_domain_error)?;
        if existing_review_replay_for_store(self, &input)
            .await?
            .is_some()
        {
            return self.get_task(&input.task_id).await?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: format!("reviewed task disappeared: {}", input.task_id),
                }
            });
        }
        let review_plan = plan_review(
            task.revision_index,
            task.max_review_rounds,
            input.overall_verdict,
            &input.criteria,
        )
        .map_err(task_domain_error)?;
        let review_id = input
            .review_id
            .clone()
            .unwrap_or_else(|| allocate_id("review"));
        let execution_policy = self.get_task_execution_policy().await?;
        let next_status = review_plan.task_status;
        let next_revision = review_plan.revision_index;
        let next_run_id = review_plan.queue_executor.then(|| allocate_id("run"));
        let next_run_kind = review_plan.queue_executor.then_some(RunKind::Executor);
        let (_, _ready_selection) = self.with_immediate_transaction_retry(|transaction| {
            if existing_review_replay(transaction, &input)?.is_some() {
                return Ok(((), None));
            }
            let persisted_task_state = transaction
                .query_row(
                    "SELECT status, revision_index, max_review_rounds, latest_run_id, complexity, pool_entry_id FROM tasks WHERE task_id = ?1",
                    [&task.task_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task not found: {}", task.task_id),
                })?;
            let persisted_status = persisted_task_state
                .0
                .parse::<TaskStatus>()
                .map_err(|error| StoreError::InvalidEnum {
                    kind: "task_status",
                    value: error.to_string(),
                })?;
            let persisted_plan = plan_review(
                persisted_task_state.1,
                persisted_task_state.2,
                input.overall_verdict,
                &input.criteria,
            )
            .map_err(task_domain_error)?;
            if persisted_status != TaskStatus::Reviewing
                || persisted_task_state.3.as_deref() != Some(input.reviewer_run_id.as_str())
                || persisted_plan != review_plan
            {
                return Err(StoreError::InvariantViolation {
                    message: format!("task changed while reviewing: {}", task.task_id),
                });
            }
            let next_run_model = if persisted_plan.queue_executor {
                let complexity = persisted_task_state
                    .4
                    .parse()
                    .map_err(task_domain_error)?;
                Some(pool_selection_tx(
                    transaction,
                    complexity,
                    &persisted_task_state.5,
                )?)
            } else {
                None
            };
            let ready_selection = next_run_model
                .as_ref()
                .map(|selection| prove_selection_ready(selection, Some(registry)))
                .transpose()?;
            let fenced = transaction.execute(
                "UPDATE agent_runs SET status = 'completed', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND task_id = ?2 AND run_kind = 'reviewer' AND triggering_submission_id = ?3 AND lease_token = ?4 AND status = 'running' AND cancellation_requested = 0 AND EXISTS (SELECT 1 FROM tasks WHERE task_id = ?2 AND latest_run_id = ?1 AND status = 'reviewing')",
                rusqlite::params![input.reviewer_run_id, input.task_id, input.reviewed_submission_id, lease_token],
            )?;
            if fenced != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!(
                        "reviewer run lease, cancellation, or task state changed while reviewing: {}",
                        input.reviewer_run_id
                    ),
                });
            }
            transaction.execute(
                "INSERT INTO task_reviews (review_id, task_id, reviewer_run_id, reviewed_submission_id, overall_verdict, overall_feedback) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![review_id, task.task_id, input.reviewer_run_id, input.reviewed_submission_id, input.overall_verdict.as_str(), input.overall_feedback.trim()],
            )?;
            for criterion in &input.criteria {
                transaction.execute(
                    "INSERT INTO task_review_criteria (review_id, criterion_id, outcome, evidence_markdown, feedback) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![review_id, criterion.criterion_id, criterion.outcome.as_str(), criterion.evidence_markdown, criterion.feedback],
                )?;
            }
            if let (Some(run_id), Some(run_kind), Some(run_model)) =
                (&next_run_id, next_run_kind, &next_run_model)
            {
                transaction.execute(
                    "INSERT INTO agent_runs (run_id, task_id, run_kind, agent_id, attempt_index, revision_index, triggering_review_id, provider_kind, provider_account_id, provider_instance_key, selection_mode, model_profile, reasoning_effort, selection_source, max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, status) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 'queued')",
                    rusqlite::params![run_id, task.task_id, run_kind.as_str(), TASK_EXECUTOR_AGENT_ID, next_revision, review_id, run_model.provider_kind, run_model.provider_account_id, run_model.provider_instance_key.as_ref().map(ToString::to_string), run_model.selection_mode.as_str(), run_model.model_profile, run_model.reasoning_effort.map(noema_providers::ReasoningEffort::as_persistence_str), run_model.selection_source, execution_policy.max_provider_continuations, execution_policy.max_tool_calls, execution_policy.max_active_minutes, execution_policy.progress_audit_interval],
                )?;
                transaction.execute(
                    "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'run.queued', ?3, ?4)",
                    rusqlite::params![
                        allocate_id("event"),
                        run_id,
                        input.reviewer_run_id,
                        serde_json::json!({"run_kind": run_kind.as_str(), "revision_index": next_revision, "triggering_review_id": review_id}).to_string(),
                    ],
                )?;
            }
            let task_changed = transaction.execute(
                "UPDATE tasks SET status = ?2, revision_index = ?3, latest_run_id = COALESCE(?4, latest_run_id), final_submission_id = CASE WHEN ?2 = 'completed' THEN ?5 ELSE final_submission_id END, blocked_question = CASE WHEN ?2 = 'waiting_for_human' THEN ?6 ELSE NULL END, blocked_context = CASE WHEN ?2 = 'waiting_for_human' THEN 'The task reviewer requires human input before execution can continue.' ELSE NULL END, completed_at = CASE WHEN ?2 = 'completed' THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE completed_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?7 AND status = 'reviewing'",
                rusqlite::params![task.task_id, next_status.as_str(), next_revision, next_run_id, input.reviewed_submission_id, input.overall_feedback.trim(), input.reviewer_run_id],
            )?;
            if task_changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!("task changed while reviewing: {}", task.task_id),
                });
            }
            append_run_event_tx(
                transaction,
                &input.reviewer_run_id,
                "run.completed",
                &input.reviewer_run_id,
                serde_json::json!({"review_id": review_id}),
            )?;
            append_task_event_tx(transaction, &task.task_id, "task.review_created", &input.reviewer_run_id, serde_json::json!({"review_id": review_id, "verdict": input.overall_verdict.as_str(), "next_run_id": next_run_id}))?;
            append_task_event_tx(
                transaction,
                &task.task_id,
                &format!("task.{}", next_status.as_str()),
                &input.reviewer_run_id,
                serde_json::json!({
                    "review_id": review_id,
                    "reviewer_run_id": input.reviewer_run_id,
                    "status": next_status.as_str(),
                }),
            )?;
            Ok(((), ready_selection))
        }).await?;
        self.get_task(&task.task_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task disappeared: {}", task.task_id),
            })
    }
}

async fn existing_review_replay_for_store(
    store: &NoemaStore,
    input: &NewTaskReview,
) -> Result<Option<String>, StoreError> {
    store
        .with_connection(|conn| {
            let tx = conn.transaction()?;
            let replay = existing_review_replay(&tx, input)?;
            tx.commit()?;
            Ok(replay)
        })
        .await
}

fn existing_review_replay(
    conn: &rusqlite::Connection,
    input: &NewTaskReview,
) -> Result<Option<String>, StoreError> {
    let existing = conn
        .query_row(
            "SELECT review_id, reviewer_run_id, overall_verdict, overall_feedback
             FROM task_reviews
             WHERE task_id = ?1 AND reviewed_submission_id = ?2
             LIMIT 1",
            params![input.task_id, input.reviewed_submission_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((review_id, reviewer_run_id, verdict, feedback)) = existing else {
        return Ok(None);
    };
    let verdict = verdict
        .parse::<TaskReviewVerdict>()
        .map_err(task_domain_error)?;
    let criteria = {
        let mut statement = conn.prepare(
            "SELECT criterion_id, outcome, evidence_markdown, feedback
             FROM task_review_criteria
             WHERE review_id = ?1
             ORDER BY criterion_id",
        )?;
        let rows = statement
            .query_map([review_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(criterion_id, outcome, evidence_markdown, feedback)| {
                Ok(TaskReviewCriterion {
                    criterion_id,
                    outcome: outcome
                        .parse::<noema_tasks::CriterionOutcome>()
                        .map_err(task_domain_error)?,
                    evidence_markdown,
                    feedback,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?
    };

    let supplied_id_matches = input
        .review_id
        .as_ref()
        .is_none_or(|supplied| supplied == &review_id);
    let exact = supplied_id_matches
        && reviewer_run_id == input.reviewer_run_id
        && verdict == input.overall_verdict
        && feedback == input.overall_feedback
        && criteria == input.criteria;
    if !exact {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "submission already has a different completed review; continue with a new executor revision instead: {}",
                input.reviewed_submission_id
            ),
        });
    }
    Ok(Some(review_id))
}
