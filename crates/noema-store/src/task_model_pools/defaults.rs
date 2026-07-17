use noema_providers::ReasoningEffort;
use noema_tasks::{TaskModelPoolEntry, provider_default_task_models};
use rusqlite::params;

use super::{
    LEGACY_PROVIDER_DEFAULT_POOL_PREFIX, NoemaStore, StoreError, global_task_model_pool_setting_id,
};

impl NoemaStore {
    /// Ensure exactly one global executor model setting exists per tier.
    ///
    /// The selected default provider supplies initial values. Existing global
    /// settings remain user-controlled, while older provider-scoped rows are
    /// consolidated and retired.
    pub async fn ensure_default_task_model_pool_settings(
        &self,
        default_provider_kind: &str,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        let account = self
            .active_provider_account(default_provider_kind)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!(
                    "default task model provider account is unavailable: {default_provider_kind}"
                ),
            })?;
        let defaults = provider_default_task_models(default_provider_kind);
        if defaults.len() != 3 {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "default task model provider {default_provider_kind} does not define all three tiers"
                ),
            });
        }
        let existing = self.list_task_model_pool_entries(None).await?;
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            for default in defaults {
                let pool_entry_id = global_task_model_pool_setting_id(default.complexity);
                if existing
                    .iter()
                    .any(|entry| entry.pool_entry_id == pool_entry_id)
                {
                    continue;
                }
                let migrated = existing
                    .iter()
                    .find(|entry| {
                        entry.complexity == default.complexity
                            && !entry
                                .pool_entry_id
                                .starts_with(LEGACY_PROVIDER_DEFAULT_POOL_PREFIX)
                    })
                    .or_else(|| {
                        existing.iter().find(|entry| {
                            entry.complexity == default.complexity
                                && entry.model.provider_account_id == account.provider_account_id
                        })
                    });
                if let Some(migrated) = migrated {
                    transaction.execute(
                        "UPDATE tasks SET pool_entry_id = ?1 WHERE pool_entry_id = ?2",
                        params![pool_entry_id, migrated.pool_entry_id],
                    )?;
                    transaction.execute(
                        "UPDATE task_model_pool_entries SET pool_entry_id = ?1, sort_order = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE pool_entry_id = ?2",
                        params![pool_entry_id, migrated.pool_entry_id],
                    )?;
                    continue;
                }
                transaction.execute(
                    r#"
                    INSERT INTO task_model_pool_entries (
                      pool_entry_id, complexity, label, provider_kind,
                      provider_account_id, model_profile, reasoning_effort,
                      enabled, sort_order
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
                    "#,
                    params![
                        pool_entry_id,
                        default.complexity.as_str(),
                        default.label,
                        account.provider_kind,
                        account.provider_account_id,
                        default.model_profile,
                        default
                            .reasoning_effort
                            .map(ReasoningEffort::as_persistence_str),
                        true,
                    ],
                )?;
            }
            for entry in &existing {
                if entry.is_global_setting() {
                    continue;
                }
                let references: i64 = transaction.query_row(
                    "SELECT COUNT(*) FROM tasks WHERE pool_entry_id = ?1",
                    [&entry.pool_entry_id],
                    |row| row.get(0),
                )?;
                if references > 0 {
                    transaction.execute(
                        "UPDATE task_model_pool_entries SET enabled = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE pool_entry_id = ?1",
                        [&entry.pool_entry_id],
                    )?;
                } else {
                    transaction.execute(
                        "DELETE FROM task_model_pool_entries WHERE pool_entry_id = ?1",
                        [&entry.pool_entry_id],
                    )?;
                }
            }
            transaction.commit()?;
            Ok(())
        })
        .await?;
        self.list_task_model_pool_settings(None).await
    }
}
