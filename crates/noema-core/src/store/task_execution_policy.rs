//! Provider-independent safety limits for durable task execution.

#![allow(clippy::missing_errors_doc)]

use rusqlite::params;

use noema_tasks::TaskExecutionPolicy;

use super::{NoemaStore, StoreError};

impl NoemaStore {
    /// Return the single global task execution policy.
    pub async fn get_task_execution_policy(&self) -> Result<TaskExecutionPolicy, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval FROM task_execution_policy WHERE policy_id = 'default'",
                [],
                |row| {
                    Ok(TaskExecutionPolicy {
                        max_provider_continuations: row.get(0)?,
                        max_tool_calls: row.get(1)?,
                        max_active_minutes: row.get(2)?,
                        progress_audit_interval: row.get(3)?,
                    })
                },
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Replace the global task execution policy used by future and resumed runs.
    pub async fn update_task_execution_policy(
        &self,
        policy: TaskExecutionPolicy,
    ) -> Result<TaskExecutionPolicy, StoreError> {
        let policy = policy
            .validated()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })?;
        self.with_connection(|conn| {
            conn.execute(
                "UPDATE task_execution_policy SET max_provider_continuations = ?1, max_tool_calls = ?2, max_active_minutes = ?3, progress_audit_interval = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE policy_id = 'default'",
                params![
                    policy.max_provider_continuations,
                    policy.max_tool_calls,
                    policy.max_active_minutes,
                    policy.progress_audit_interval,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_task_execution_policy().await
    }
}
