use noema_providers::{ProviderRegistry, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{TaskModelPoolEntry, provider_default_task_models};
use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError, global_task_model_pool_setting_id};
use crate::provider_selections::{
    SelectionEligibility, prove_selection_ready, resolve_provider_selection_tx,
};

impl NoemaStore {
    /// Ensure exactly one global executor model setting exists per tier.
    ///
    /// The selected default provider supplies initial values. Existing global
    /// settings remain user-controlled.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the provider defaults are incomplete, the
    /// default account is unavailable, or SQLite fails.
    pub async fn ensure_default_task_model_pool_settings(
        &self,
        default_provider_kind: &str,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        self.ensure_default_task_model_pool_settings_inner(default_provider_kind, None)
            .await
    }

    /// Ensure missing global executor settings while proving every newly
    /// referenced exact provider route is ready through commit.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the provider defaults or registry readiness
    /// are invalid, or when SQLite fails.
    pub async fn ensure_default_task_model_pool_settings_with_readiness(
        &self,
        default_provider_kind: &str,
        registry: &ProviderRegistry,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        self.ensure_default_task_model_pool_settings_inner(default_provider_kind, Some(registry))
            .await
    }

    async fn ensure_default_task_model_pool_settings_inner(
        &self,
        default_provider_kind: &str,
        registry: Option<&ProviderRegistry>,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        let defaults = provider_default_task_models(default_provider_kind);
        if defaults.len() != 3 {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "default task model provider {default_provider_kind} does not define all three tiers"
                ),
            });
        }
        let (_, _ready_selections) = self.with_immediate_transaction_retry(|transaction| {
            let mut ready_selections = Vec::new();
            let account = transaction
                .query_row(
                    "SELECT provider_kind, provider_account_id FROM provider_accounts WHERE provider_kind = ?1 AND is_active = 1 AND is_default = 1 LIMIT 1",
                    [default_provider_kind],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!(
                        "default task model provider account is unavailable: {default_provider_kind}"
                    ),
                })?;
            if account.0 == "local_models" {
                return Err(StoreError::ProviderInstanceUnavailable {
                    provider_instance_key: account.1,
                });
            }
            for default in &defaults {
                let pool_entry_id = global_task_model_pool_setting_id(default.complexity);
                let exists = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_model_pool_entries WHERE pool_entry_id = ?1)",
                    [&pool_entry_id],
                    |row| row.get::<_, bool>(0),
                )?;
                if exists {
                    continue;
                }
                let unresolved = ProviderSelectionSnapshot::explicit(
                    account.0.clone(),
                    account.1.clone(),
                    default.model_profile,
                    default.reasoning_effort,
                    Some("task_model_pool_setting".to_string()),
                );
                let selection = resolve_provider_selection_tx(
                    transaction,
                    &unresolved,
                    SelectionEligibility::Canonical,
                )?;
                ready_selections.push(prove_selection_ready(&selection, registry)?);
                transaction.execute(
                    r#"
                    INSERT INTO task_model_pool_entries (
                      pool_entry_id, complexity, label, provider_kind,
                      provider_account_id, provider_instance_key, model_profile, reasoning_effort,
                      enabled, sort_order
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)
                    "#,
                    params![
                        pool_entry_id,
                        default.complexity.as_str(),
                        default.label,
                        selection.provider_kind,
                        selection.provider_account_id,
                        selection.provider_instance_key.as_ref().map(ToString::to_string),
                        selection.model_profile,
                        selection.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                        true,
                    ],
                )?;
            }
            Ok(((), ready_selections))
        })
        .await?;
        self.list_task_model_pool_settings(None).await
    }
}
