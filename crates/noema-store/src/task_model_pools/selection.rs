use noema_providers::{ProviderReadySelection, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{NewTaskModelPoolEntry, TaskComplexity, TaskModelPoolEntry};
use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError, rows::pool_entry_from_row};
use crate::provider_selections::{
    SelectionEligibility, resolve_new_canonical_selection_tx, validate_provider_selection_tx,
};

impl NoemaStore {
    /// Update an existing pool entry while retaining its stable id.
    ///
    /// Exact-route metadata edits and disabling are proof-free because they do
    /// not establish a future reference. Enabling or redirecting an entry
    /// requires [`Self::update_task_model_pool_entry_with_ready_selection`].
    pub async fn update_task_model_pool_entry(
        &self,
        pool_entry_id: &str,
        input: NewTaskModelPoolEntry,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        self.update_task_model_pool_entry_inner(pool_entry_id, input, None)
            .await
    }

    /// Update a pool entry while retaining a registry readiness lease through
    /// commit. Enabling a disabled entry or redirecting an enabled entry must
    /// use this API.
    pub async fn update_task_model_pool_entry_with_ready_selection(
        &self,
        pool_entry_id: &str,
        input: NewTaskModelPoolEntry,
        ready_selection: &ProviderReadySelection,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        self.update_task_model_pool_entry_inner(pool_entry_id, input, Some(ready_selection))
            .await
    }

    async fn update_task_model_pool_entry_inner(
        &self,
        pool_entry_id: &str,
        input: NewTaskModelPoolEntry,
        ready_selection: Option<&ProviderReadySelection>,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        let input = input
            .normalized()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })?;
        if !noema_tasks::is_global_task_model_pool_setting_id(pool_entry_id) {
            return Err(StoreError::InvariantViolation {
                message: "task model pool settings have stable complexity tiers".to_string(),
            });
        }
        self.with_immediate_transaction_retry(|transaction| {
            let existing = transaction
                .query_row(
                    r#"
                    SELECT pool_entry_id, complexity, label, provider_kind,
                           provider_account_id, provider_instance_key, model_profile,
                           reasoning_effort, enabled, sort_order, created_at, updated_at
                    FROM task_model_pool_entries
                    WHERE pool_entry_id = ?1
                    LIMIT 1
                    "#,
                    [pool_entry_id],
                    pool_entry_from_row,
                )
                .optional()?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task model pool entry not found: {pool_entry_id}"),
                })?;
            if existing.complexity != input.complexity {
                return Err(StoreError::InvariantViolation {
                    message: "task model pool settings have stable complexity tiers".to_string(),
                });
            }
            let unresolved = ProviderSelectionSnapshot::explicit(
                input.provider_kind.clone(),
                input.provider_account_id.clone(),
                input.model_profile.clone(),
                input.reasoning_effort,
                Some("task_model_pool_setting".to_string()),
            );
            let retains_exact_route = requested_route_matches(&existing.model, &unresolved);
            if !input.enabled && !retains_exact_route {
                return Err(StoreError::InvariantViolation {
                    message:
                        "disabling a task model pool entry cannot also change its provider route"
                            .to_string(),
                });
            }
            let selection = if retains_exact_route && (existing.enabled || !input.enabled) {
                existing.model
            } else {
                resolve_new_canonical_selection_tx(transaction, &unresolved, ready_selection)?
            };
            transaction.execute(
                r#"
                UPDATE task_model_pool_entries
                SET complexity = ?2,
                    label = ?3,
                    provider_kind = ?4,
                    provider_account_id = ?5,
                    provider_instance_key = ?6,
                    model_profile = ?7,
                    reasoning_effort = ?8,
                    enabled = ?9,
                    sort_order = ?10,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE pool_entry_id = ?1
                "#,
                params![
                    pool_entry_id,
                    input.complexity.as_str(),
                    input.label,
                    selection.provider_kind,
                    selection.provider_account_id,
                    selection
                        .provider_instance_key
                        .as_ref()
                        .map(ToString::to_string),
                    selection.model_profile,
                    selection
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
    pub(crate) async fn validate_task_model_snapshot(
        &self,
        model: &ProviderSelectionSnapshot,
    ) -> Result<(), StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            validate_provider_selection_tx(transaction, model, SelectionEligibility::Canonical)
                .map(|_| ())
        })
        .await
    }
}

fn requested_route_matches(
    existing: &ProviderSelectionSnapshot,
    requested: &ProviderSelectionSnapshot,
) -> bool {
    existing.provider_kind == requested.provider_kind
        && existing.provider_account_id == requested.provider_account_id
        && existing.selection_mode == requested.selection_mode
        && existing.model_profile == requested.model_profile
        && existing.reasoning_effort == requested.reasoning_effort
}
