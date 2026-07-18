use noema_tasks::{TaskComplexity, TaskModelPoolEntry, is_global_task_model_pool_setting_id};
use rusqlite::OptionalExtension;

use super::{NoemaStore, StoreError, rows::pool_entry_from_row};

impl NoemaStore {
    /// Return one pool entry by stable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite cannot read or decode the entry.
    pub async fn get_task_model_pool_entry(
        &self,
        pool_entry_id: &str,
    ) -> Result<Option<TaskModelPoolEntry>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT pool_entry_id, complexity, label, provider_kind,
                       provider_account_id, provider_instance_key, model_profile, reasoning_effort,
                       enabled, sort_order, created_at, updated_at
                FROM task_model_pool_entries
                WHERE pool_entry_id = ?1
                LIMIT 1
                "#,
                [pool_entry_id],
                pool_entry_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List the three global task-executor settings.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite cannot read or decode the settings.
    pub async fn list_task_model_pool_settings(
        &self,
        complexity: Option<TaskComplexity>,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        let entries = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    r#"
                SELECT pool_entry_id, complexity, label, provider_kind,
                       provider_account_id, provider_instance_key, model_profile, reasoning_effort,
                       enabled, sort_order, created_at, updated_at
                FROM task_model_pool_entries
                WHERE (?1 IS NULL OR complexity = ?1)
                ORDER BY complexity, sort_order, label, pool_entry_id
                "#,
                )?;
                let rows = statement.query_map(
                    [complexity.map(TaskComplexity::as_str)],
                    pool_entry_from_row,
                )?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        Ok(entries
            .into_iter()
            .filter(|entry| is_global_task_model_pool_setting_id(&entry.pool_entry_id))
            .collect())
    }

    /// List enabled pool entries backed by an authenticated provider account.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when settings cannot be read or their provider
    /// selections cannot be validated.
    pub async fn list_usable_task_model_pool_entries(
        &self,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        let entries = self.list_task_model_pool_settings(None).await?;
        let mut usable = Vec::new();
        for entry in entries.into_iter().filter(|entry| entry.enabled) {
            if self
                .validate_task_model_snapshot(&entry.model)
                .await
                .is_ok()
            {
                usable.push(entry);
            }
        }
        Ok(usable)
    }
}
