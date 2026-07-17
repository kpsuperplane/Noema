use noema_providers::ReasoningEffort;
use noema_tasks::{AgentRunRecord, NewAgentRun};
use rusqlite::{OptionalExtension, params};

use super::RUN_COLUMNS;
use crate::{NoemaStore, StoreError, agent_run_rows::run_from_row, ids::allocate_id};

impl NoemaStore {
    /// Queue a new background run.
    pub async fn create_agent_run(&self, input: NewAgentRun) -> Result<AgentRunRecord, StoreError> {
        let model = input.model.normalized_for_persistence().map_err(|error| {
            StoreError::InvariantViolation {
                message: error.to_string(),
            }
        })?;
        let execution_policy =
            input
                .execution_policy
                .validated()
                .map_err(|error| StoreError::InvariantViolation {
                    message: error.to_string(),
                })?;
        if input.task_id.trim().is_empty() || input.agent_id.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "agent run task and agent ids are required".to_string(),
            });
        }
        if input.revision_index < 0 || input.attempt_index < 0 {
            return Err(StoreError::InvariantViolation {
                message: "agent run indexes cannot be negative".to_string(),
            });
        }
        let run_id = input.run_id.unwrap_or_else(|| allocate_id("run"));
        self.with_connection(|conn| {
            conn.execute(
                r#"INSERT INTO agent_runs (
                    run_id, task_id, run_kind, agent_id, attempt_index, revision_index,
                    parent_run_id, triggering_submission_id, triggering_review_id,
                    provider_kind, provider_account_id, selection_mode, model_profile,
                    reasoning_effort, selection_source, max_provider_continuations,
                    max_tool_calls, max_active_minutes, progress_audit_interval,
                    status, priority
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, 'queued', ?20)"#,
                params![
                    run_id,
                    input.task_id,
                    input.run_kind.as_str(),
                    input.agent_id,
                    input.attempt_index,
                    input.revision_index,
                    input.parent_run_id,
                    input.triggering_submission_id,
                    input.triggering_review_id,
                    model.provider_kind,
                    model.provider_account_id,
                    model.selection_mode.as_str(),
                    model.model_profile,
                    model.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                    model.selection_source,
                    execution_policy.max_provider_continuations,
                    execution_policy.max_tool_calls,
                    execution_policy.max_active_minutes,
                    execution_policy.progress_audit_interval,
                    input.priority,
                ],
            )?;
            Ok(())
        }).await?;
        self.get_agent_run(&run_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("created run disappeared: {run_id}"),
            })
    }

    /// Return one run by id.
    pub async fn get_agent_run(&self, run_id: &str) -> Result<Option<AgentRunRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                &format!("SELECT {RUN_COLUMNS} FROM agent_runs WHERE run_id = ?1"),
                [run_id],
                run_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List runs for one task in creation order.
    pub async fn list_agent_runs_for_task(
        &self,
        task_id: &str,
    ) -> Result<Vec<AgentRunRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM agent_runs WHERE task_id = ?1 ORDER BY created_at, run_id"
        ))?;
            let rows = statement.query_map([task_id], run_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }
}
