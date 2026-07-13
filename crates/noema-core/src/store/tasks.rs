//! Durable one-off task records and state transitions.

#![allow(clippy::missing_errors_doc)]

use rusqlite::{OptionalExtension, params};

use crate::{
    DEFAULT_TASK_MAX_REVIEW_ROUNDS, ModelConfigSnapshot, NewTask, NewTaskReview, NewTaskSubmission,
    RunKind, TASK_EXECUTOR_AGENT_ID, TaskComplexity, TaskDomainError, TaskReviewCriterion,
    TaskReviewVerdict, TaskSource, TaskStatus, TaskValidationCriterion, provider::ReasoningEffort,
};

use super::{NoemaStore, StoreError, agent_runs::AgentRunRecord, ids::allocate_id};

/// Persisted task projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    /// Stable task id.
    pub task_id: String,
    /// Human-visible title.
    pub title: String,
    /// Immutable normalized request.
    pub request_markdown: String,
    /// Complexity tier.
    pub complexity: TaskComplexity,
    /// Current workflow state.
    pub status: TaskStatus,
    /// Owning human.
    pub owner_human_id: String,
    /// Source conversation/turn/item provenance.
    pub source: TaskSource,
    /// Creating agent.
    pub created_by_agent_id: String,
    /// Delegation tool call id.
    pub creation_tool_call_id: Option<String>,
    /// Selected pool entry.
    pub pool_entry_id: String,
    /// Executor model snapshot.
    pub executor_model: ModelConfigSnapshot,
    /// Reviewer model snapshot.
    pub reviewer_model: ModelConfigSnapshot,
    /// Current revision index.
    pub revision_index: i64,
    /// Maximum reviewed submissions.
    pub max_review_rounds: i64,
    /// Approved submission id.
    pub final_submission_id: Option<String>,
    /// Latest run id.
    pub latest_run_id: Option<String>,
    /// Human-facing question that must be answered before resuming.
    pub blocked_question: Option<String>,
    /// Executor summary retained while waiting for human input.
    pub blocked_context: Option<String>,
    /// Terminal reason.
    pub terminal_reason: Option<String>,
    /// Safe error code.
    pub error_code: Option<String>,
    /// Safe error message.
    pub error_message: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Completion timestamp.
    pub completed_at: Option<String>,
}

/// Persisted executor submission with criterion evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskSubmissionRecord {
    /// Stable submission id.
    pub submission_id: String,
    /// Owning task id.
    pub task_id: String,
    /// Executor run id.
    pub executor_run_id: String,
    /// Revision index.
    pub revision_index: i64,
    /// Short summary.
    pub summary: String,
    /// Complete result Markdown.
    pub result_markdown: String,
    /// Criterion evidence.
    pub criteria: Vec<crate::SubmissionCriterionEvidence>,
    /// Ordered governed artifact snapshots linked by this submission.
    pub artifacts: Vec<TaskSubmissionArtifactRecord>,
    /// Creation timestamp.
    pub created_at: String,
}

/// One governed artifact snapshot linked to an executor submission.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskSubmissionArtifactRecord {
    /// One-based order supplied by the executor.
    pub ordinal: i64,
    /// Durable artifact metadata.
    pub artifact: crate::ArtifactRecord,
    /// Immutable version captured when the submission was committed.
    pub version: crate::ArtifactVersionRecord,
}

/// Persisted adversarial review with per-criterion outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReviewRecord {
    /// Stable review id.
    pub review_id: String,
    /// Owning task id.
    pub task_id: String,
    /// Reviewer run id.
    pub reviewer_run_id: String,
    /// Submission under review.
    pub reviewed_submission_id: String,
    /// Overall verdict.
    pub overall_verdict: TaskReviewVerdict,
    /// Safe overall feedback.
    pub overall_feedback: String,
    /// Criterion outcomes.
    pub criteria: Vec<TaskReviewCriterion>,
    /// Creation timestamp.
    pub created_at: String,
}

impl NoemaStore {
    /// Create a task, criteria, initial executor run, and creation event in one
    /// SQLite transaction.
    pub async fn create_task_with_executor(
        &self,
        input: NewTask,
    ) -> Result<(TaskRecord, AgentRunRecord), StoreError> {
        self.validate_task_model_snapshot(&input.executor_model)
            .await?;
        self.validate_task_model_snapshot(&input.reviewer_model)
            .await?;
        let input = input.normalized().map_err(task_domain_error)?;
        let execution_policy = self.get_task_execution_policy().await?;
        let pool = self
            .select_task_model_pool_entry(input.complexity, &input.pool_entry_id)
            .await?;
        if pool.model != input.executor_model {
            return Err(StoreError::InvariantViolation {
                message: "executor snapshot does not match selected pool entry".to_string(),
            });
        }
        self.require_agent(&input.created_by_agent_id).await?;
        let task_id = input.task_id.clone().unwrap_or_else(|| allocate_id("task"));
        if let (Some(conversation_id), Some(call_id)) = (
            input.source.conversation_id.as_deref(),
            input.creation_tool_call_id.as_deref(),
        ) && let Some(existing) = self
            .find_task_by_creation_call(conversation_id, call_id)
            .await?
        {
            let run = self
                .list_agent_runs_for_task(&existing.task_id)
                .await?
                .into_iter()
                .find(|run| run.run_kind == RunKind::Executor)
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("idempotent task has no executor run: {}", existing.task_id),
                })?;
            return Ok((existing, run));
        }
        let run_id = allocate_id("run");
        let criterion_ids = input
            .criteria
            .iter()
            .map(|criterion| {
                criterion
                    .criterion_id
                    .clone()
                    .unwrap_or_else(|| allocate_id("criterion"))
            })
            .collect::<Vec<_>>();
        let max_review_rounds = input
            .max_review_rounds
            .unwrap_or(DEFAULT_TASK_MAX_REVIEW_ROUNDS);
        let executor =
            input
                .executor_model
                .normalized()
                .map_err(|error| StoreError::InvariantViolation {
                    message: error.to_string(),
                })?;
        let reviewer =
            input
                .reviewer_model
                .normalized()
                .map_err(|error| StoreError::InvariantViolation {
                    message: error.to_string(),
                })?;
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                r#"INSERT INTO tasks (
                    task_id, title, request_markdown, complexity, status,
                    owner_human_id, source_conversation_id, source_turn_id,
                    source_item_id, created_by_agent_id, creation_tool_call_id,
                    pool_entry_id, executor_provider_kind, executor_provider_account_id,
                    executor_selection_mode, executor_model_profile, executor_reasoning_effort,
                    executor_selection_source, reviewer_provider_kind, reviewer_provider_account_id,
                    reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort,
                    reviewer_selection_source, max_review_rounds, latest_run_id
                ) VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                    ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)"#,
                params![
                    task_id,
                    input.title,
                    input.request_markdown,
                    input.complexity.as_str(),
                    input.owner_human_id,
                    input.source.conversation_id,
                    input.source.turn_id,
                    input.source.item_id,
                    input.created_by_agent_id,
                    input.creation_tool_call_id,
                    input.pool_entry_id,
                    executor.provider_kind,
                    executor.provider_account_id,
                    executor.selection_mode.as_str(),
                    executor.model_profile,
                    executor.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                    executor.selection_source,
                    reviewer.provider_kind,
                    reviewer.provider_account_id,
                    reviewer.selection_mode.as_str(),
                    reviewer.model_profile,
                    reviewer.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                    reviewer.selection_source,
                    max_review_rounds,
                    run_id,
                ],
            )?;
            for (criterion, criterion_id) in input.criteria.iter().zip(criterion_ids.iter()) {
                tx.execute(
                    "INSERT INTO task_validation_criteria (criterion_id, task_id, ordinal, description, expected_evidence) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![criterion_id, task_id, criterion.ordinal, criterion.description, criterion.expected_evidence],
                )?;
            }
            tx.execute(
                r#"INSERT INTO agent_runs (
                    run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                    provider_kind, provider_account_id, selection_mode, model_profile,
                    reasoning_effort, selection_source, max_provider_continuations,
                    max_tool_calls, max_active_minutes, progress_audit_interval, status
                ) VALUES (?1, ?2, 'executor', ?3, 0, 0, ?4, ?5, ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12, ?13, 'queued')"#,
                params![
                    run_id,
                    task_id,
                    TASK_EXECUTOR_AGENT_ID,
                    executor.provider_kind,
                    executor.provider_account_id,
                    executor.selection_mode.as_str(),
                    executor.model_profile,
                    executor.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                    executor.selection_source,
                    execution_policy.max_provider_continuations,
                    execution_policy.max_tool_calls,
                    execution_policy.max_active_minutes,
                    execution_policy.progress_audit_interval,
                ],
            )?;
            tx.execute(
                "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'run.queued', ?3, ?4)",
                params![
                    allocate_id("event"),
                    run_id,
                    input.created_by_agent_id,
                    serde_json::json!({"run_kind": "executor", "revision_index": 0}).to_string(),
                ],
            )?;
            tx.execute(
                "INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'task.created', ?3, ?4)",
                params![allocate_id("event"), task_id, input.created_by_agent_id, serde_json::json!({"run_id": run_id, "complexity": input.complexity.as_str()}).to_string()],
            )?;
            tx.commit()?;
            Ok(())
        }).await?;
        let task =
            self.get_task(&task_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("created task disappeared: {task_id}"),
                })?;
        let run =
            self.get_agent_run(&run_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("created run disappeared: {run_id}"),
                })?;
        Ok((task, run))
    }

    /// Find a task created by one source delegation call.
    pub async fn find_task_by_creation_call(
        &self,
        conversation_id: &str,
        call_id: &str,
    ) -> Result<Option<TaskRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT task_id, title, request_markdown, complexity, status, owner_human_id, source_conversation_id, source_turn_id, source_item_id, created_by_agent_id, creation_tool_call_id, pool_entry_id, executor_provider_kind, executor_provider_account_id, executor_selection_mode, executor_model_profile, executor_reasoning_effort, executor_selection_source, reviewer_provider_kind, reviewer_provider_account_id, reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort, reviewer_selection_source, revision_index, max_review_rounds, final_submission_id, latest_run_id, blocked_question, blocked_context, terminal_reason, error_code, error_message, created_at, updated_at, completed_at FROM tasks WHERE source_conversation_id = ?1 AND creation_tool_call_id = ?2 LIMIT 1",
                params![conversation_id, call_id],
                task_from_row,
            ).optional().map_err(StoreError::Sqlite)
        }).await
    }

    /// Return one task by id.
    pub async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT task_id, title, request_markdown, complexity, status, owner_human_id, source_conversation_id, source_turn_id, source_item_id, created_by_agent_id, creation_tool_call_id, pool_entry_id, executor_provider_kind, executor_provider_account_id, executor_selection_mode, executor_model_profile, executor_reasoning_effort, executor_selection_source, reviewer_provider_kind, reviewer_provider_account_id, reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort, reviewer_selection_source, revision_index, max_review_rounds, final_submission_id, latest_run_id, blocked_question, blocked_context, terminal_reason, error_code, error_message, created_at, updated_at, completed_at FROM tasks WHERE task_id = ?1 LIMIT 1",
                [task_id],
                task_from_row,
            ).optional().map_err(StoreError::Sqlite)
        }).await
    }

    /// Return immutable criteria in display order.
    pub async fn list_task_validation_criteria(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskValidationCriterion>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare("SELECT criterion_id, ordinal, description, expected_evidence FROM task_validation_criteria WHERE task_id = ?1 ORDER BY ordinal, criterion_id")?;
            let rows = statement.query_map([task_id], |row| Ok(TaskValidationCriterion { criterion_id: row.get(0)?, ordinal: row.get(1)?, description: row.get(2)?, expected_evidence: row.get(3)? }))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite)
        }).await
    }

    /// Enforce and persist one task state transition.
    pub async fn transition_task(
        &self,
        task_id: &str,
        next: TaskStatus,
        reason: Option<&str>,
    ) -> Result<TaskRecord, StoreError> {
        self.with_connection(|conn| {
            let current = conn.query_row("SELECT status FROM tasks WHERE task_id = ?1", [task_id], |row| row.get::<_, String>(0)).optional()?.ok_or_else(|| StoreError::InvariantViolation { message: format!("task not found: {task_id}") })?;
            let current = current.parse::<TaskStatus>().map_err(|error| StoreError::InvalidEnum { kind: "task_status", value: error.to_string() })?;
            if !current.can_transition_to(next) { return Err(StoreError::InvariantViolation { message: format!("invalid task transition {current} -> {next}") }); }
            let changed = conn.execute("UPDATE tasks SET status = ?2, terminal_reason = CASE WHEN ?2 IN ('failed', 'cancelled') THEN COALESCE(?3, terminal_reason) ELSE NULL END, completed_at = CASE WHEN ?2 = 'completed' THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE completed_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1", params![task_id, next.as_str(), reason])?;
            if changed != 1 { return Err(StoreError::InvariantViolation { message: format!("task transition lost race: {task_id}") }); }
            let next_sequence: i64 = conn.query_row("SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1", [task_id], |row| row.get(0))?;
            conn.execute("INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, 'system:task-runtime', ?5)", params![allocate_id("event"), task_id, next_sequence, format!("task.{}", next.as_str()), serde_json::json!({"from": current.as_str(), "to": next.as_str(), "reason": reason}).to_string()])?;
            Ok(())
        }).await?;
        self.get_task(task_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task disappeared: {task_id}"),
            })
    }

    /// Commit an executor submission and queue its reviewer in one transaction.
    pub async fn create_task_submission(
        &self,
        input: NewTaskSubmission,
        lease_token: &str,
    ) -> Result<(TaskSubmissionRecord, AgentRunRecord), StoreError> {
        if lease_token.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "task submission requires the active run lease".to_string(),
            });
        }
        let task =
            self.get_task(&input.task_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task not found: {}", input.task_id),
                })?;
        if task.status != TaskStatus::Executing && task.status != TaskStatus::RevisionRequested {
            return Err(StoreError::InvariantViolation {
                message: format!("task is not executable: {}", task.status),
            });
        }
        self.validate_task_model_snapshot(&task.reviewer_model)
            .await?;
        validate_submission_criteria(self, &input.task_id, &input.criteria).await?;
        let artifacts =
            validate_submission_artifacts(self, &input.task_id, &input.artifact_ids).await?;
        let execution_policy = self.get_task_execution_policy().await?;
        if let Some(existing_submission_id) = self
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT submission_id FROM task_submissions WHERE task_id = ?1 AND revision_index = ?2 LIMIT 1",
                    rusqlite::params![input.task_id, input.revision_index],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?
        {
            let submission = self
                .get_task_submission(&existing_submission_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("existing submission disappeared: {existing_submission_id}"),
                })?;
            let reviewer_run = self
                .list_agent_runs_for_task(&input.task_id)
                .await?
                .into_iter()
                .find(|run| {
                    run.run_kind == RunKind::Reviewer
                        && run.triggering_submission_id.as_deref()
                            == Some(existing_submission_id.as_str())
                })
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!(
                        "existing submission has no reviewer run: {existing_submission_id}"
                    ),
                })?;
            return Ok((submission, reviewer_run));
        }
        let submission_id = input
            .submission_id
            .unwrap_or_else(|| allocate_id("submission"));
        let reviewer_run_id = allocate_id("run");
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let fenced = tx.execute(
                "UPDATE agent_runs SET status = 'completed', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND task_id = ?2 AND run_kind = 'executor' AND revision_index = ?3 AND lease_token = ?4 AND status = 'running' AND cancellation_requested = 0 AND EXISTS (SELECT 1 FROM tasks WHERE task_id = ?2 AND latest_run_id = ?1 AND status IN ('executing', 'revision_requested'))",
                rusqlite::params![input.executor_run_id, input.task_id, input.revision_index, lease_token],
            )?;
            if fenced != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!(
                        "executor run lease, cancellation, or task state changed while submitting: {}",
                        input.executor_run_id
                    ),
                });
            }
            tx.execute(
                "INSERT INTO task_submissions (submission_id, task_id, executor_run_id, revision_index, summary, result_markdown) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![submission_id, input.task_id, input.executor_run_id, input.revision_index, input.summary.trim(), input.result_markdown.trim()],
            )?;
            for criterion in &input.criteria {
                tx.execute(
                    "INSERT INTO task_submission_criteria (submission_id, criterion_id, evidence_markdown) VALUES (?1, ?2, ?3)",
                    rusqlite::params![submission_id, criterion.criterion_id, criterion.evidence_markdown.trim()],
                )?;
            }
            for artifact in &artifacts {
                tx.execute(
                    "INSERT INTO task_submission_artifacts (submission_id, artifact_id, artifact_version_id) VALUES (?1, ?2, ?3)",
                    rusqlite::params![submission_id, artifact.artifact.artifact_id, artifact.version.artifact_version_id],
                )?;
            }
            tx.execute(
                "INSERT INTO agent_runs (run_id, task_id, run_kind, agent_id, attempt_index, revision_index, triggering_submission_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, status) VALUES (?1, ?2, 'reviewer', ?3, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'queued')",
                rusqlite::params![
                    reviewer_run_id,
                    task.task_id,
                    crate::TASK_REVIEWER_AGENT_ID,
                    input.revision_index,
                    submission_id,
                    task.reviewer_model.provider_kind,
                    task.reviewer_model.provider_account_id,
                    task.reviewer_model.selection_mode.as_str(),
                    task.reviewer_model.model_profile,
                    task.reviewer_model.reasoning_effort.map(crate::provider::ReasoningEffort::as_persistence_str),
                    task.reviewer_model.selection_source,
                    execution_policy.max_provider_continuations,
                    execution_policy.max_tool_calls,
                    execution_policy.max_active_minutes,
                    execution_policy.progress_audit_interval,
                ],
            )?;
            tx.execute(
                "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'run.queued', ?3, ?4)",
                rusqlite::params![
                    allocate_id("event"),
                    reviewer_run_id,
                    input.executor_run_id,
                    serde_json::json!({"run_kind": "reviewer", "revision_index": input.revision_index, "triggering_submission_id": submission_id}).to_string(),
                ],
            )?;
            let task_changed = tx.execute(
                "UPDATE tasks SET status = 'reviewing', latest_run_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?3 AND status IN ('executing', 'revision_requested')",
                rusqlite::params![task.task_id, reviewer_run_id, input.executor_run_id],
            )?;
            if task_changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!("task changed while submitting: {}", task.task_id),
                });
            }
            append_run_event_tx(
                &tx,
                &input.executor_run_id,
                "run.completed",
                &input.executor_run_id,
                serde_json::json!({"submission_id": submission_id}),
            )?;
            append_task_event_tx(&tx, &task.task_id, "task.submission_created", &input.executor_run_id, serde_json::json!({"submission_id": submission_id, "reviewer_run_id": reviewer_run_id}))?;
            tx.commit()?;
            Ok(())
        }).await?;
        let submission = self
            .get_task_submission(&submission_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("submission disappeared: {submission_id}"),
            })?;
        let run = self.get_agent_run(&reviewer_run_id).await?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: format!("reviewer run disappeared: {reviewer_run_id}"),
            }
        })?;
        Ok((submission, run))
    }

    /// Commit an adversarial review and derive the next task state.
    pub async fn create_task_review(
        &self,
        input: NewTaskReview,
        lease_token: &str,
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
        validate_review_criteria(&criteria, &input.criteria)?;
        let all_pass = input
            .criteria
            .iter()
            .all(|criterion| criterion.outcome == crate::CriterionOutcome::Pass);
        if input.overall_verdict == TaskReviewVerdict::Approve && !all_pass {
            return Err(StoreError::InvariantViolation {
                message: "review approval requires every criterion to pass".to_string(),
            });
        }
        if let Some(existing_reviewer_run_id) = self
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT reviewer_run_id FROM task_reviews WHERE task_id = ?1 AND reviewed_submission_id = ?2 LIMIT 1",
                    rusqlite::params![input.task_id, input.reviewed_submission_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?
        {
            if existing_reviewer_run_id != input.reviewer_run_id {
                return Err(StoreError::InvariantViolation {
                    message: format!(
                        "submission already has a completed review; continue with a new executor revision instead: {}",
                        input.reviewed_submission_id
                    ),
                });
            }
            return self
                .get_task(&input.task_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("reviewed task disappeared: {}", input.task_id),
                });
        }
        let review_id = input.review_id.unwrap_or_else(|| allocate_id("review"));
        let execution_policy = self.get_task_execution_policy().await?;
        let current_executor_model = if input.overall_verdict == TaskReviewVerdict::RequestChanges {
            Some(
                self.select_task_model_pool_entry(task.complexity, &task.pool_entry_id)
                    .await?
                    .model,
            )
        } else {
            None
        };
        let (next_status, next_revision, next_run_id, next_run_kind, next_run_model) =
            match input.overall_verdict {
                TaskReviewVerdict::Approve => {
                    (TaskStatus::Completed, task.revision_index, None, None, None)
                }
                TaskReviewVerdict::NeedsHuman => (
                    TaskStatus::WaitingForHuman,
                    task.revision_index,
                    None,
                    None,
                    None,
                ),
                TaskReviewVerdict::RequestChanges
                    if task.revision_index + 1 < task.max_review_rounds =>
                {
                    (
                        TaskStatus::RevisionRequested,
                        task.revision_index + 1,
                        Some(allocate_id("run")),
                        Some(RunKind::Executor),
                        current_executor_model,
                    )
                }
                TaskReviewVerdict::RequestChanges => (
                    TaskStatus::WaitingForHuman,
                    task.revision_index,
                    None,
                    None,
                    None,
                ),
            };
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let fenced = tx.execute(
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
            tx.execute(
                "INSERT INTO task_reviews (review_id, task_id, reviewer_run_id, reviewed_submission_id, overall_verdict, overall_feedback) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![review_id, task.task_id, input.reviewer_run_id, input.reviewed_submission_id, input.overall_verdict.as_str(), input.overall_feedback.trim()],
            )?;
            for criterion in &input.criteria {
                tx.execute(
                    "INSERT INTO task_review_criteria (review_id, criterion_id, outcome, evidence_markdown, feedback) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![review_id, criterion.criterion_id, criterion.outcome.as_str(), criterion.evidence_markdown, criterion.feedback],
                )?;
            }
            if let (Some(run_id), Some(run_kind), Some(run_model)) =
                (&next_run_id, next_run_kind, &next_run_model)
            {
                tx.execute(
                    "INSERT INTO agent_runs (run_id, task_id, run_kind, agent_id, attempt_index, revision_index, triggering_review_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, status) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'queued')",
                    rusqlite::params![run_id, task.task_id, run_kind.as_str(), TASK_EXECUTOR_AGENT_ID, next_revision, review_id, run_model.provider_kind, run_model.provider_account_id, run_model.selection_mode.as_str(), run_model.model_profile, run_model.reasoning_effort.map(crate::provider::ReasoningEffort::as_persistence_str), run_model.selection_source, execution_policy.max_provider_continuations, execution_policy.max_tool_calls, execution_policy.max_active_minutes, execution_policy.progress_audit_interval],
                )?;
                tx.execute(
                    "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'run.queued', ?3, ?4)",
                    rusqlite::params![
                        allocate_id("event"),
                        run_id,
                        input.reviewer_run_id,
                        serde_json::json!({"run_kind": run_kind.as_str(), "revision_index": next_revision, "triggering_review_id": review_id}).to_string(),
                    ],
                )?;
            }
            let task_changed = tx.execute(
                "UPDATE tasks SET status = ?2, revision_index = ?3, latest_run_id = COALESCE(?4, latest_run_id), final_submission_id = CASE WHEN ?2 = 'completed' THEN ?5 ELSE final_submission_id END, blocked_question = CASE WHEN ?2 = 'waiting_for_human' THEN ?6 ELSE NULL END, blocked_context = CASE WHEN ?2 = 'waiting_for_human' THEN 'The task reviewer requires human input before execution can continue.' ELSE NULL END, completed_at = CASE WHEN ?2 = 'completed' THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE completed_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?7 AND status = 'reviewing'",
                rusqlite::params![task.task_id, next_status.as_str(), next_revision, next_run_id, input.reviewed_submission_id, input.overall_feedback.trim(), input.reviewer_run_id],
            )?;
            if task_changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!("task changed while reviewing: {}", task.task_id),
                });
            }
            append_run_event_tx(
                &tx,
                &input.reviewer_run_id,
                "run.completed",
                &input.reviewer_run_id,
                serde_json::json!({"review_id": review_id}),
            )?;
            append_task_event_tx(&tx, &task.task_id, "task.review_created", &input.reviewer_run_id, serde_json::json!({"review_id": review_id, "verdict": input.overall_verdict.as_str(), "next_run_id": next_run_id}))?;
            append_task_event_tx(
                &tx,
                &task.task_id,
                &format!("task.{}", next_status.as_str()),
                &input.reviewer_run_id,
                serde_json::json!({
                    "review_id": review_id,
                    "reviewer_run_id": input.reviewer_run_id,
                    "status": next_status.as_str(),
                }),
            )?;
            tx.commit()?;
            Ok(())
        }).await?;
        self.get_task(&task.task_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task disappeared: {}", task.task_id),
            })
    }

    /// Return one submission with its criterion evidence.
    pub async fn get_task_submission(
        &self,
        submission_id: &str,
    ) -> Result<Option<TaskSubmissionRecord>, StoreError> {
        let base = self.with_connection(|conn| conn.query_row("SELECT submission_id, task_id, executor_run_id, revision_index, summary, result_markdown, created_at FROM task_submissions WHERE submission_id = ?1 LIMIT 1", [submission_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?))).optional().map_err(StoreError::Sqlite)).await?;
        let Some((
            submission_id,
            task_id,
            executor_run_id,
            revision_index,
            summary,
            result_markdown,
            created_at,
        )) = base
        else {
            return Ok(None);
        };
        let criteria = self.with_connection(|conn| { let mut statement = conn.prepare("SELECT criterion_id, evidence_markdown FROM task_submission_criteria WHERE submission_id = ?1 ORDER BY criterion_id")?; let rows = statement.query_map([submission_id.as_str()], |row| Ok(crate::SubmissionCriterionEvidence { criterion_id: row.get(0)?, evidence_markdown: row.get(1)? }))?; rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite) }).await?;
        let artifact_links = self.with_connection(|conn| { let mut statement = conn.prepare("SELECT artifact_id, artifact_version_id FROM task_submission_artifacts WHERE submission_id = ?1 ORDER BY rowid")?; let rows = statement.query_map([submission_id.as_str()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?; rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite) }).await?;
        let mut artifacts = Vec::with_capacity(artifact_links.len());
        for (index, (artifact_id, artifact_version_id)) in artifact_links.into_iter().enumerate() {
            let artifact = self.get_artifact(&artifact_id).await?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: format!("submission artifact disappeared: {artifact_id}"),
                }
            })?;
            let version = self
                .get_artifact_version(&artifact_version_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!(
                        "submission artifact version disappeared: {artifact_version_id}"
                    ),
                })?;
            artifacts.push(TaskSubmissionArtifactRecord {
                ordinal: i64::try_from(index + 1).unwrap_or(i64::MAX),
                artifact: artifact.artifact,
                version,
            });
        }
        Ok(Some(TaskSubmissionRecord {
            submission_id,
            task_id,
            executor_run_id,
            revision_index,
            summary,
            result_markdown,
            criteria,
            artifacts,
            created_at,
        }))
    }
}

async fn validate_submission_artifacts(
    store: &NoemaStore,
    task_id: &str,
    artifact_ids: &[String],
) -> Result<Vec<TaskSubmissionArtifactRecord>, StoreError> {
    if artifact_ids.len() > 100 {
        return Err(StoreError::InvariantViolation {
            message: "task submission cannot link more than 100 artifacts".to_string(),
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut artifacts = Vec::with_capacity(artifact_ids.len());
    for (index, artifact_id) in artifact_ids.iter().enumerate() {
        if artifact_id.trim().is_empty() || !seen.insert(artifact_id.as_str()) {
            return Err(StoreError::InvariantViolation {
                message: "task submission artifact ids must be non-empty and unique".to_string(),
            });
        }
        let artifact = store.get_artifact(artifact_id).await?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: format!("task submission artifact not found: {artifact_id}"),
            }
        })?;
        if artifact.artifact.owner != crate::ArtifactOwnerRef::task(task_id) {
            return Err(StoreError::InvariantViolation {
                message: format!("artifact is not owned by task {task_id}: {artifact_id}"),
            });
        }
        artifacts.push(TaskSubmissionArtifactRecord {
            ordinal: i64::try_from(index + 1).unwrap_or(i64::MAX),
            artifact: artifact.artifact,
            version: artifact.current_version,
        });
    }
    Ok(artifacts)
}

async fn validate_submission_criteria(
    store: &NoemaStore,
    task_id: &str,
    submitted: &[crate::SubmissionCriterionEvidence],
) -> Result<(), StoreError> {
    let expected = store.list_task_validation_criteria(task_id).await?;
    let mut ids = submitted
        .iter()
        .map(|criterion| criterion.criterion_id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    let mut expected_ids = expected
        .iter()
        .map(|criterion| criterion.criterion_id.as_str())
        .collect::<Vec<_>>();
    expected_ids.sort_unstable();
    if submitted.len() != expected.len()
        || ids != expected_ids
        || submitted
            .iter()
            .any(|criterion| criterion.evidence_markdown.trim().is_empty())
    {
        return Err(StoreError::InvariantViolation {
            message:
                "submission must include non-empty evidence for every task criterion exactly once"
                    .to_string(),
        });
    }
    Ok(())
}

fn validate_review_criteria(
    expected: &[TaskValidationCriterion],
    submitted: &[TaskReviewCriterion],
) -> Result<(), StoreError> {
    let mut ids = submitted
        .iter()
        .map(|criterion| criterion.criterion_id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    let mut expected_ids = expected
        .iter()
        .map(|criterion| criterion.criterion_id.as_str())
        .collect::<Vec<_>>();
    expected_ids.sort_unstable();
    if submitted.len() != expected.len() || ids != expected_ids {
        return Err(StoreError::InvariantViolation {
            message: "review must include exactly one result for every task criterion".to_string(),
        });
    }
    Ok(())
}

fn append_task_event_tx(
    tx: &rusqlite::Transaction<'_>,
    task_id: &str,
    kind: &str,
    actor_id: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1",
        [task_id],
        |row| row.get(0),
    )?;
    tx.execute("INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", rusqlite::params![allocate_id("event"), task_id, sequence, kind, actor_id, payload.to_string()])?;
    Ok(())
}

fn append_run_event_tx(
    tx: &rusqlite::Transaction<'_>,
    run_id: &str,
    kind: &str,
    actor_id: &str,
    payload: serde_json::Value,
) -> Result<(), rusqlite::Error> {
    let sequence: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM run_events WHERE run_id = ?1",
        [run_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            allocate_id("event"),
            run_id,
            sequence,
            kind,
            actor_id,
            payload.to_string()
        ],
    )?;
    Ok(())
}

fn task_domain_error(error: TaskDomainError) -> StoreError {
    StoreError::InvariantViolation {
        message: error.to_string(),
    }
}

fn task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRecord> {
    let complexity = row
        .get::<_, String>(3)?
        .parse::<TaskComplexity>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let status = row
        .get::<_, String>(4)?
        .parse::<TaskStatus>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let executor_selection_mode = row
        .get::<_, String>(14)?
        .parse::<crate::ModelSelectionMode>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                14,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let reviewer_selection_mode = row
        .get::<_, String>(20)?
        .parse::<crate::ModelSelectionMode>()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                20,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let executor_reasoning_effort = parse_reasoning(row.get::<_, Option<String>>(16)?.as_deref())?;
    let reviewer_reasoning_effort = parse_reasoning(row.get::<_, Option<String>>(22)?.as_deref())?;
    Ok(TaskRecord {
        task_id: row.get(0)?,
        title: row.get(1)?,
        request_markdown: row.get(2)?,
        complexity,
        status,
        owner_human_id: row.get(5)?,
        source: TaskSource {
            conversation_id: row.get(6)?,
            turn_id: row.get(7)?,
            item_id: row.get(8)?,
        },
        created_by_agent_id: row.get(9)?,
        creation_tool_call_id: row.get(10)?,
        pool_entry_id: row.get(11)?,
        executor_model: ModelConfigSnapshot {
            provider_kind: row.get(12)?,
            provider_account_id: row.get(13)?,
            selection_mode: executor_selection_mode,
            model_profile: row.get(15)?,
            reasoning_effort: executor_reasoning_effort,
            selection_source: row.get(17)?,
        },
        reviewer_model: ModelConfigSnapshot {
            provider_kind: row.get(18)?,
            provider_account_id: row.get(19)?,
            selection_mode: reviewer_selection_mode,
            model_profile: row.get(21)?,
            reasoning_effort: reviewer_reasoning_effort,
            selection_source: row.get(23)?,
        },
        revision_index: row.get(24)?,
        max_review_rounds: row.get(25)?,
        final_submission_id: row.get(26)?,
        latest_run_id: row.get(27)?,
        blocked_question: row.get(28)?,
        blocked_context: row.get(29)?,
        terminal_reason: row.get(30)?,
        error_code: row.get(31)?,
        error_message: row.get(32)?,
        created_at: row.get(33)?,
        updated_at: row.get(34)?,
        completed_at: row.get(35)?,
    })
}

fn parse_reasoning(value: Option<&str>) -> rusqlite::Result<Option<ReasoningEffort>> {
    value
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid reasoning effort",
                    )),
                )
            })
        })
        .transpose()
}
