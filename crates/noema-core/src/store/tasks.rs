//! Durable one-off task records and state transitions.

use rusqlite::{OptionalExtension, params};

use crate::{
    DEFAULT_TASK_MAX_REVIEW_ROUNDS, ModelConfigSnapshot, NewTask, RunKind, TASK_EXECUTOR_AGENT_ID,
    TaskComplexity, TaskDomainError, TaskSource, TaskStatus, TaskValidationCriterion,
    provider::ReasoningEffort,
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

impl NoemaStore {
    /// Create a task, criteria, initial executor run, and creation event in one
    /// SQLite transaction.
    pub async fn create_task_with_executor(
        &self,
        input: NewTask,
    ) -> Result<(TaskRecord, AgentRunRecord), StoreError> {
        let input = input.normalized().map_err(task_domain_error)?;
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
        ) {
            if let Some(existing) = self
                .find_task_by_creation_call(conversation_id, call_id)
                .await?
            {
                let run = self
                    .list_agent_runs_for_task(&existing.task_id)
                    .await?
                    .into_iter()
                    .find(|run| run.run_kind == RunKind::Executor)
                    .ok_or_else(|| StoreError::InvariantViolation {
                        message: format!(
                            "idempotent task has no executor run: {}",
                            existing.task_id
                        ),
                    })?;
                return Ok((existing, run));
            }
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
                    reasoning_effort, selection_source, status
                ) VALUES (?1, ?2, 'executor', ?3, 0, 0, ?4, ?5, ?6, ?7, ?8, ?9, 'queued')"#,
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
                "SELECT task_id, title, request_markdown, complexity, status, owner_human_id, source_conversation_id, source_turn_id, source_item_id, created_by_agent_id, creation_tool_call_id, pool_entry_id, executor_provider_kind, executor_provider_account_id, executor_selection_mode, executor_model_profile, executor_reasoning_effort, executor_selection_source, reviewer_provider_kind, reviewer_provider_account_id, reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort, reviewer_selection_source, revision_index, max_review_rounds, final_submission_id, latest_run_id, terminal_reason, error_code, error_message, created_at, updated_at, completed_at FROM tasks WHERE source_conversation_id = ?1 AND creation_tool_call_id = ?2 LIMIT 1",
                params![conversation_id, call_id],
                task_from_row,
            ).optional().map_err(StoreError::Sqlite)
        }).await
    }

    /// Return one task by id.
    pub async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT task_id, title, request_markdown, complexity, status, owner_human_id, source_conversation_id, source_turn_id, source_item_id, created_by_agent_id, creation_tool_call_id, pool_entry_id, executor_provider_kind, executor_provider_account_id, executor_selection_mode, executor_model_profile, executor_reasoning_effort, executor_selection_source, reviewer_provider_kind, reviewer_provider_account_id, reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort, reviewer_selection_source, revision_index, max_review_rounds, final_submission_id, latest_run_id, terminal_reason, error_code, error_message, created_at, updated_at, completed_at FROM tasks WHERE task_id = ?1 LIMIT 1",
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
            let changed = conn.execute("UPDATE tasks SET status = ?2, terminal_reason = COALESCE(?3, terminal_reason), completed_at = CASE WHEN ?2 = 'completed' THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE completed_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1", params![task_id, next.as_str(), reason])?;
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
        terminal_reason: row.get(28)?,
        error_code: row.get(29)?,
        error_message: row.get(30)?,
        created_at: row.get(31)?,
        updated_at: row.get(32)?,
        completed_at: row.get(33)?,
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
