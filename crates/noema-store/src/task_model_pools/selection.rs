use noema_providers::{ProviderKind, ProviderReadySelection, ProviderSelectionSnapshot};
use noema_tasks::{NewTaskModelPoolEntry, TaskModelPoolEntry, model_use_case};
use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError, rows::pool_entry_from_row};
use crate::provider_selections::resolve_new_canonical_selection_tx;

impl NoemaStore {
    /// Update an existing pool entry while retaining its stable id.
    ///
    /// Exact-route metadata edits and disabling are proof-free because they do
    /// not establish a future reference. Enabling or redirecting an entry
    /// requires [`Self::update_task_model_pool_entry_with_ready_selection`].
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the input is invalid, the requested entry or
    /// provider route is unavailable, or SQLite fails.
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
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the input is invalid, the entry is missing,
    /// the readiness proof does not match the route, or SQLite fails.
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
                           provider_account_id, provider_instance_key, selection_mode,
                           model_profile, reasoning_effort, fast_mode, enabled, sort_order,
                           created_at, updated_at
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
            let unresolved = preference_route(&input)?;
            let retains_exact_route = existing.preference == input.selection
                && requested_route_matches(&existing.model, &unresolved);
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
                    selection_mode = ?7,
                    model_profile = ?8,
                    reasoning_effort = ?9,
                    fast_mode = ?10,
                    enabled = ?11,
                    sort_order = ?12,
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
                    input.selection.as_str(),
                    input.selection.model_profile(),
                    input
                        .selection
                        .reasoning_effort()
                        .map(noema_providers::ReasoningEffort::as_persistence_str),
                    input.fast_mode,
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
}

fn preference_route(
    input: &NewTaskModelPoolEntry,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let provider_kind = input.provider_kind.parse::<ProviderKind>().map_err(|_| {
        StoreError::InvariantViolation {
            message: format!("unsupported model provider: {}", input.provider_kind),
        }
    })?;
    let (model_profile, reasoning_effort) = input
        .selection
        .resolve(provider_kind, model_use_case(input.complexity))
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "provider has no Noema recommendation for this task tier".to_string(),
        })?;
    let mut selection = ProviderSelectionSnapshot::explicit(
        input.provider_kind.clone(),
        input.provider_account_id.clone(),
        model_profile,
        reasoning_effort,
        Some("task_model_pool_setting".to_string()),
    );
    selection.fast_mode = input.fast_mode;
    Ok(selection)
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
        && existing.fast_mode == requested.fast_mode
}
