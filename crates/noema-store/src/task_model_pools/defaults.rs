use noema_providers::{
    ModelPreferenceSelection, ProviderKind, ProviderRegistry, ProviderSelectionSnapshot,
    noema_model_recommendation,
};
use noema_tasks::{TaskComplexity, TaskModelPoolEntry, model_use_case};
use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError, global_task_model_pool_setting_id};
use crate::provider_selections::{
    SelectionEligibility, prove_selection_ready, resolve_provider_selection_tx,
};

impl NoemaStore {
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
        let provider_kind = default_provider_kind.parse::<ProviderKind>().map_err(|_| {
            StoreError::InvariantViolation {
                message: format!(
                    "unsupported default task model provider: {default_provider_kind}"
                ),
            }
        })?;
        let complexities = [
            TaskComplexity::Simple,
            TaskComplexity::Medium,
            TaskComplexity::Difficult,
        ];
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
            for complexity in complexities {
                let pool_entry_id = global_task_model_pool_setting_id(complexity);
                let exists = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_model_pool_entries WHERE pool_entry_id = ?1)",
                    [&pool_entry_id],
                    |row| row.get::<_, bool>(0),
                )?;
                if exists {
                    continue;
                }
                let recommendation = noema_model_recommendation(
                    provider_kind.clone(),
                    model_use_case(complexity),
                );
                let preference = if recommendation.is_some() {
                    ModelPreferenceSelection::NoemaRecommended
                } else {
                    ModelPreferenceSelection::ExplicitProfile {
                        model_profile: "default".to_string(),
                        reasoning_effort: None,
                    }
                };
                let (model_profile, reasoning_effort) = recommendation.map_or_else(
                    || ("default", None),
                    |value| (value.model_profile, value.reasoning_effort),
                );
                let unresolved = ProviderSelectionSnapshot::explicit(
                    account.0.clone(),
                    account.1.clone(),
                    model_profile,
                    reasoning_effort,
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
                      provider_account_id, provider_instance_key, selection_mode,
                      model_profile, reasoning_effort, enabled, sort_order
                    )
                    VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, 1, 0)
                    "#,
                    params![
                        pool_entry_id,
                        complexity.as_str(),
                        selection.provider_kind,
                        selection.provider_account_id,
                        selection.provider_instance_key.as_ref().map(ToString::to_string),
                        preference.as_str(),
                        preference.model_profile(),
                        preference.reasoning_effort().map(noema_providers::ReasoningEffort::as_persistence_str),
                    ],
                )?;
            }
            Ok(((), ready_selections))
        })
        .await?;
        self.list_task_model_pool_settings(None).await
    }
}
