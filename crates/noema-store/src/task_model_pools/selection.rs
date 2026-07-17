use noema_providers::{ProviderAccountStatus, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{NewTaskModelPoolEntry, TaskComplexity, TaskModelPoolEntry};
use rusqlite::params;

use super::{NoemaStore, StoreError};

impl NoemaStore {
    /// Update an existing pool entry while retaining its stable id.
    pub async fn update_task_model_pool_entry(
        &self,
        pool_entry_id: &str,
        input: NewTaskModelPoolEntry,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        let input = input
            .normalized()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })?;
        self.validate_pool_account(&input.provider_kind, &input.provider_account_id)
            .await?;
        let existing = self
            .get_task_model_pool_entry(pool_entry_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task model pool entry not found: {pool_entry_id}"),
            })?;
        if !existing.is_global_setting() || existing.complexity != input.complexity {
            return Err(StoreError::InvariantViolation {
                message: "task model pool settings have stable complexity tiers".to_string(),
            });
        }
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE task_model_pool_entries
                SET complexity = ?2,
                    label = ?3,
                    provider_kind = ?4,
                    provider_account_id = ?5,
                    model_profile = ?6,
                    reasoning_effort = ?7,
                    enabled = ?8,
                    sort_order = ?9,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE pool_entry_id = ?1
                "#,
                params![
                    existing.pool_entry_id,
                    input.complexity.as_str(),
                    input.label,
                    input.provider_kind,
                    input.provider_account_id,
                    input.model_profile,
                    input
                        .reasoning_effort
                        .map(ReasoningEffort::as_persistence_str),
                    input.enabled,
                    input.sort_order,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_task_model_pool_entry(pool_entry_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("updated task model pool entry disappeared: {pool_entry_id}"),
            })
    }

    /// Select an enabled exact entry from the requested tier.
    pub async fn select_task_model_pool_entry(
        &self,
        complexity: TaskComplexity,
        pool_entry_id: &str,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        let entry = self
            .get_task_model_pool_entry(pool_entry_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task model pool entry not found: {pool_entry_id}"),
            })?;
        if entry.complexity != complexity {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "task model pool entry {pool_entry_id} belongs to {}, not {}",
                    entry.complexity, complexity
                ),
            });
        }
        if !entry.enabled {
            return Err(StoreError::InvariantViolation {
                message: format!("task model pool entry is disabled: {pool_entry_id}"),
            });
        }
        self.validate_task_model_snapshot(&entry.model).await?;
        Ok(entry)
    }

    /// Verify that a task model still belongs to an authenticated account and,
    /// when a catalog is available, that the exact profile is advertised.
    pub async fn validate_task_model_snapshot(
        &self,
        model: &ProviderSelectionSnapshot,
    ) -> Result<(), StoreError> {
        self.validate_usable_pool_account(
            &model.provider_kind,
            &model.provider_account_id,
            model.model_profile.as_deref(),
        )
        .await
    }

    async fn validate_pool_account(
        &self,
        provider_kind: &str,
        provider_account_id: &str,
    ) -> Result<(), StoreError> {
        let account = self
            .get_provider_account(provider_account_id)
            .await?
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            })?;
        if !account.is_active || account.provider_kind != provider_kind {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "provider account {} is not an active {} account",
                    provider_account_id, provider_kind
                ),
            });
        }
        Ok(())
    }

    async fn validate_usable_pool_account(
        &self,
        provider_kind: &str,
        provider_account_id: &str,
        model_profile: Option<&str>,
    ) -> Result<(), StoreError> {
        self.validate_pool_account(provider_kind, provider_account_id)
            .await?;
        let account = self
            .get_provider_account(provider_account_id)
            .await?
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            })?;
        if account.status != ProviderAccountStatus::Authenticated {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "provider account {provider_account_id} is not authenticated for task execution"
                ),
            });
        }
        if let Some(model_profile) = model_profile
            && let Some(profiles) = account
                .metadata
                .get("profiles")
                .and_then(serde_json::Value::as_array)
            && !profiles.is_empty()
            && !profiles.iter().any(|profile| {
                profile.get("id").and_then(serde_json::Value::as_str) == Some(model_profile)
            })
        {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "model profile {model_profile} is not available for provider account {provider_account_id}"
                ),
            });
        }
        Ok(())
    }
}
