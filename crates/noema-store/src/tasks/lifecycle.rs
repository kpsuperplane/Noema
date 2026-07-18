use noema_providers::{ProviderRegistry, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{
    AgentRunRecord, DEFAULT_TASK_MAX_REVIEW_ROUNDS, NewTask, TASK_EXECUTOR_AGENT_ID,
    TaskDomainError, TaskRecord, TaskSource, TaskStatus, TaskValidationCriterion,
};
use rusqlite::{OptionalExtension, params};

use super::provider_selection::{pool_selection_tx, reviewer_preference_tx};
use crate::{
    NoemaStore, StoreError,
    ids::allocate_id,
    provider_selections::prove_selection_ready,
    sqlite::{parse_column, reasoning_column},
};

const TASK_COLUMNS: &str = "task_id, title, request_markdown, complexity, status, owner_human_id, source_conversation_id, source_turn_id, source_item_id, created_by_agent_id, creation_tool_call_id, pool_entry_id, executor_provider_kind, executor_provider_account_id, executor_provider_instance_key, executor_selection_mode, executor_model_profile, executor_reasoning_effort, executor_selection_source, reviewer_provider_kind, reviewer_provider_account_id, reviewer_provider_instance_key, reviewer_selection_mode, reviewer_model_profile, reviewer_reasoning_effort, reviewer_selection_source, revision_index, max_review_rounds, final_submission_id, latest_run_id, blocked_question, blocked_context, terminal_reason, error_code, error_message, created_at, updated_at, completed_at";

impl NoemaStore {
    /// Create a task while proving every newly referenced executor and reviewer
    /// instance is registered and ready through commit.
    pub async fn create_task_with_executor_with_readiness(
        &self,
        input: NewTask,
        registry: &ProviderRegistry,
    ) -> Result<(TaskRecord, AgentRunRecord), StoreError> {
        let input = input.normalized().map_err(task_domain_error)?;
        let execution_policy = self.get_task_execution_policy().await?;
        self.require_agent(&input.created_by_agent_id).await?;
        let task_id = input.task_id.clone().unwrap_or_else(|| allocate_id("task"));
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
        let ((task_id, run_id), _ready_selections) = self.with_immediate_transaction_retry(|transaction| {
            if let (Some(conversation_id), Some(call_id)) = (
                input.source.conversation_id.as_deref(),
                input.creation_tool_call_id.as_deref(),
            ) && let Some(existing_task_id) = transaction
                .query_row(
                    "SELECT task_id FROM tasks WHERE source_conversation_id = ?1 AND creation_tool_call_id = ?2 LIMIT 1",
                    params![conversation_id, call_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                let existing_run_id = transaction
                    .query_row(
                        "SELECT run_id FROM agent_runs WHERE task_id = ?1 AND run_kind = 'executor' ORDER BY created_at, run_id LIMIT 1",
                        [&existing_task_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                    .ok_or_else(|| StoreError::InvariantViolation {
                        message: format!("idempotent task has no executor run: {existing_task_id}"),
                    })?;
                return Ok(((existing_task_id, existing_run_id), Vec::new()));
            }
            let executor = pool_selection_tx(
                transaction,
                input.complexity,
                &input.pool_entry_id,
            )?;
            let reviewer = reviewer_preference_tx(transaction)?;
            let ready_selections = vec![
                prove_selection_ready(&executor, Some(registry))?,
                prove_selection_ready(&reviewer, Some(registry))?,
            ];
            transaction.execute(
                r#"INSERT INTO tasks (
                    task_id, title, request_markdown, complexity, status,
                    owner_human_id, source_conversation_id, source_turn_id,
                    source_item_id, created_by_agent_id, creation_tool_call_id,
                    pool_entry_id, executor_provider_kind, executor_provider_account_id,
                    executor_provider_instance_key, executor_selection_mode,
                    executor_model_profile, executor_reasoning_effort,
                    executor_selection_source, reviewer_provider_kind,
                    reviewer_provider_account_id, reviewer_provider_instance_key,
                    reviewer_selection_mode, reviewer_model_profile,
                    reviewer_reasoning_effort, reviewer_selection_source,
                    max_review_rounds, latest_run_id
                ) VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                    ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                    ?24, ?25, ?26, ?27)"#,
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
                    executor.provider_instance_key.as_ref().map(ToString::to_string),
                    executor.selection_mode.as_str(),
                    executor.model_profile,
                    executor.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                    executor.selection_source,
                    reviewer.provider_kind,
                    reviewer.provider_account_id,
                    reviewer.provider_instance_key.as_ref().map(ToString::to_string),
                    reviewer.selection_mode.as_str(),
                    reviewer.model_profile,
                    reviewer.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                    reviewer.selection_source,
                    max_review_rounds,
                    run_id,
                ],
            )?;
            for (criterion, criterion_id) in input.criteria.iter().zip(criterion_ids.iter()) {
                transaction.execute(
                    "INSERT INTO task_validation_criteria (criterion_id, task_id, ordinal, description, expected_evidence) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![criterion_id, task_id, criterion.ordinal, criterion.description, criterion.expected_evidence],
                )?;
            }
            transaction.execute(
                r#"INSERT INTO agent_runs (
                    run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                    provider_kind, provider_account_id, provider_instance_key,
                    selection_mode, model_profile, reasoning_effort, selection_source,
                    max_provider_continuations, max_tool_calls, max_active_minutes,
                    progress_audit_interval, status
                ) VALUES (?1, ?2, 'executor', ?3, 0, 0, ?4, ?5, ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12, ?13, ?14, 'queued')"#,
                params![
                    run_id,
                    task_id,
                    TASK_EXECUTOR_AGENT_ID,
                    executor.provider_kind,
                    executor.provider_account_id,
                    executor.provider_instance_key.as_ref().map(ToString::to_string),
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
            transaction.execute(
                "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'run.queued', ?3, ?4)",
                params![
                    allocate_id("event"),
                    run_id,
                    input.created_by_agent_id,
                    serde_json::json!({"run_kind": "executor", "revision_index": 0}).to_string(),
                ],
            )?;
            transaction.execute(
                "INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'task.created', ?3, ?4)",
                params![allocate_id("event"), task_id, input.created_by_agent_id, serde_json::json!({"run_id": run_id, "complexity": input.complexity.as_str()}).to_string()],
            )?;
            Ok(((task_id.clone(), run_id.clone()), ready_selections))
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

    /// Return one task by id.
    pub async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE task_id = ?1 LIMIT 1"),
                [task_id],
                task_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
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
            let tx = conn.transaction()?;
            let current = tx.query_row("SELECT status FROM tasks WHERE task_id = ?1", [task_id], |row| row.get::<_, String>(0)).optional()?.ok_or_else(|| StoreError::InvariantViolation { message: format!("task not found: {task_id}") })?;
            let current = current.parse::<TaskStatus>().map_err(|error| StoreError::InvalidEnum { kind: "task_status", value: error.to_string() })?;
            if !current.can_transition_to(next) { return Err(StoreError::InvariantViolation { message: format!("invalid task transition {current} -> {next}") }); }
            let changed = tx.execute("UPDATE tasks SET status = ?2, terminal_reason = CASE WHEN ?2 IN ('failed', 'cancelled') THEN COALESCE(?3, terminal_reason) ELSE NULL END, completed_at = CASE WHEN ?2 = 'completed' THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE completed_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND status = ?4", params![task_id, next.as_str(), reason, current.as_str()])?;
            if changed != 1 { return Err(StoreError::InvariantViolation { message: format!("task transition lost race: {task_id}") }); }
            let next_sequence: i64 = tx.query_row("SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM task_events WHERE task_id = ?1", [task_id], |row| row.get(0))?;
            tx.execute("INSERT INTO task_events (event_id, task_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, 'system:task-runtime', ?5)", params![allocate_id("event"), task_id, next_sequence, format!("task.{}", next.as_str()), serde_json::json!({"from": current.as_str(), "to": next.as_str(), "reason": reason}).to_string()])?;
            tx.commit()?;
            Ok(())
        }).await?;
        self.get_task(task_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task disappeared: {task_id}"),
            })
    }
}

pub(super) fn task_domain_error(error: TaskDomainError) -> StoreError {
    StoreError::InvariantViolation {
        message: error.to_string(),
    }
}

fn task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRecord> {
    let complexity = parse_column(row, 3)?;
    let status = parse_column(row, 4)?;
    let executor_selection_mode = parse_column(row, 15)?;
    let reviewer_selection_mode = parse_column(row, 22)?;
    let executor_reasoning_effort = reasoning_column(row, 17)?;
    let reviewer_reasoning_effort = reasoning_column(row, 24)?;
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
        executor_model: ProviderSelectionSnapshot {
            provider_instance_key: Some(parse_column(row, 14)?),
            provider_kind: row.get(12)?,
            provider_account_id: row.get(13)?,
            selection_mode: executor_selection_mode,
            model_profile: row.get(16)?,
            reasoning_effort: executor_reasoning_effort,
            selection_source: row.get(18)?,
        },
        reviewer_model: ProviderSelectionSnapshot {
            provider_instance_key: Some(parse_column(row, 21)?),
            provider_kind: row.get(19)?,
            provider_account_id: row.get(20)?,
            selection_mode: reviewer_selection_mode,
            model_profile: row.get(23)?,
            reasoning_effort: reviewer_reasoning_effort,
            selection_source: row.get(25)?,
        },
        revision_index: row.get(26)?,
        max_review_rounds: row.get(27)?,
        final_submission_id: row.get(28)?,
        latest_run_id: row.get(29)?,
        blocked_question: row.get(30)?,
        blocked_context: row.get(31)?,
        terminal_reason: row.get(32)?,
        error_code: row.get(33)?,
        error_message: row.get(34)?,
        created_at: row.get(35)?,
        updated_at: row.get(36)?,
        completed_at: row.get(37)?,
    })
}
