//! Provider-independent safety limits for durable task execution.

use noema_tasks::TaskExecutionPolicy;
use rusqlite::{Connection, params};

use super::{NoemaStore, StoreError};

impl NoemaStore {
    /// Return the single global task execution policy.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the default policy row is unavailable,
    /// malformed, or outside the domain's safety bounds.
    pub async fn get_task_execution_policy(&self) -> Result<TaskExecutionPolicy, StoreError> {
        self.with_connection(|connection| load_task_execution_policy(connection))
            .await
    }

    /// Replace the global task execution policy used by future and resumed runs.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] without mutating the current policy when `policy`
    /// violates a safety bound or the atomic SQLite update cannot be completed.
    pub async fn update_task_execution_policy(
        &self,
        policy: TaskExecutionPolicy,
    ) -> Result<TaskExecutionPolicy, StoreError> {
        let policy = policy.validated().map_err(StoreError::Work)?;
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE task_execution_policy
                 SET max_provider_continuations = ?1,
                     max_tool_calls = ?2,
                     max_active_minutes = ?3,
                     progress_audit_interval = ?4,
                     max_automatic_retries = ?5,
                     max_review_rounds = ?6,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE policy_id = 'default'",
                params![
                    policy.max_provider_continuations,
                    policy.max_tool_calls,
                    policy.max_active_minutes,
                    policy.progress_audit_interval,
                    policy.max_automatic_retries,
                    policy.max_review_rounds,
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "default task execution policy row is missing".to_string(),
                });
            }
            load_task_execution_policy(transaction)
        })
        .await
    }
}

fn load_task_execution_policy(connection: &Connection) -> Result<TaskExecutionPolicy, StoreError> {
    connection
        .query_row(
            "SELECT max_provider_continuations, max_tool_calls, max_active_minutes,
                    progress_audit_interval, max_automatic_retries, max_review_rounds
             FROM task_execution_policy WHERE policy_id = 'default'",
            [],
            |row| {
                Ok(TaskExecutionPolicy {
                    max_provider_continuations: row.get(0)?,
                    max_tool_calls: row.get(1)?,
                    max_active_minutes: row.get(2)?,
                    progress_audit_interval: row.get(3)?,
                    max_automatic_retries: row.get(4)?,
                    max_review_rounds: row.get(5)?,
                })
            },
        )
        .map_err(StoreError::Sqlite)?
        .validated()
        .map_err(StoreError::Work)
}

#[cfg(test)]
mod tests {
    use noema_tasks::{MAX_TASK_AUTOMATIC_RETRIES, WorkDomainError};

    use super::*;

    #[tokio::test]
    async fn policy_round_trips_all_six_fields() {
        let store = crate::test_support::open_ephemeral_store()
            .await
            .expect("open store");
        assert_eq!(
            store
                .get_task_execution_policy()
                .await
                .expect("default policy"),
            TaskExecutionPolicy::default()
        );
        let updated = TaskExecutionPolicy {
            max_provider_continuations: 42,
            max_tool_calls: 210,
            max_active_minutes: 90,
            progress_audit_interval: 14,
            max_automatic_retries: 7,
            max_review_rounds: 9,
        };

        assert_eq!(
            store
                .update_task_execution_policy(updated)
                .await
                .expect("update policy"),
            updated
        );
        assert_eq!(
            store
                .get_task_execution_policy()
                .await
                .expect("read updated policy"),
            updated
        );
    }

    #[tokio::test]
    async fn invalid_policy_is_rejected_without_mutation() {
        let store = crate::test_support::open_ephemeral_store()
            .await
            .expect("open store");
        let before = store
            .get_task_execution_policy()
            .await
            .expect("default policy");
        let invalid = TaskExecutionPolicy {
            max_automatic_retries: MAX_TASK_AUTOMATIC_RETRIES + 1,
            ..before
        };

        let error = store
            .update_task_execution_policy(invalid)
            .await
            .expect_err("invalid policy must fail");
        assert!(matches!(
            error,
            StoreError::Work(WorkDomainError::InvalidInput {
                field: "execution_policy.max_automatic_retries",
                ..
            })
        ));
        assert_eq!(
            store
                .get_task_execution_policy()
                .await
                .expect("policy after rejected update"),
            before
        );
    }
}
