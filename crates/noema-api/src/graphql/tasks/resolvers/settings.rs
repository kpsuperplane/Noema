use async_graphql::Result;

use crate::graphql::{
    agents::{
        provider_disabled_reason, require_selectable_profile, selectable_model_account,
        selectable_profiles_from_account, validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
    tasks::{
        GraphqlTaskComplexity, GraphqlTaskExecutionPolicy, GraphqlTaskExecutionPolicyInput,
        GraphqlTaskModelPoolEntry, GraphqlTaskModelPoolEntryInput,
    },
};

use super::require_owner;

/// Resolve the human-controlled executor model pool.
pub(in crate::graphql) async fn task_model_pools(
    state: &GraphqlState,
    complexity: Option<GraphqlTaskComplexity>,
) -> Result<Vec<GraphqlTaskModelPoolEntry>> {
    state
        .store()?
        .list_task_model_pool_settings(complexity.map(Into::into))
        .await
        .map_err(graphql_error)
        .map(|entries| entries.into_iter().map(Into::into).collect())
}

/// Resolve global task execution limits shared by every complexity tier.
pub(in crate::graphql) async fn task_execution_policy(
    state: &GraphqlState,
) -> Result<GraphqlTaskExecutionPolicy> {
    state
        .store()?
        .get_task_execution_policy()
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

/// Replace the user-controlled limits while preserving Work-owned policy bounds.
pub(in crate::graphql) async fn update_task_execution_policy(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlTaskExecutionPolicyInput,
) -> Result<GraphqlTaskExecutionPolicy> {
    require_owner(principal_subject)?;
    let store = state.store()?;
    let current = store
        .get_task_execution_policy()
        .await
        .map_err(graphql_error)?;
    let policy = noema_tasks::TaskExecutionPolicy {
        max_provider_continuations: positive_limit(
            input.max_provider_continuations,
            "maxProviderContinuations",
        )?,
        max_tool_calls: positive_limit(input.max_tool_calls, "maxToolCalls")?,
        max_active_minutes: positive_limit(input.max_active_minutes, "maxActiveMinutes")?,
        progress_audit_interval: positive_limit(
            input.progress_audit_interval,
            "progressAuditInterval",
        )?,
        max_automatic_retries: current.max_automatic_retries,
        max_review_rounds: current.max_review_rounds,
    };
    store
        .update_task_execution_policy(policy)
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

/// Replace one executor model-pool entry for the local human.
pub(in crate::graphql) async fn update_task_model_pool_entry(
    state: &GraphqlState,
    principal_subject: &str,
    pool_entry_id: String,
    input: GraphqlTaskModelPoolEntryInput,
) -> Result<GraphqlTaskModelPoolEntry> {
    require_owner(principal_subject)?;
    let store = state.store()?;
    let normalized_pool_entry_id = pool_entry_id.trim().to_string();
    let requested_reasoning_effort = input.reasoning_effort.map(Into::into);
    let existing = store
        .get_task_model_pool_entry(&normalized_pool_entry_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("task model pool entry was not found"))?;
    let retains_exact_route = existing.model.provider_kind
        == input.provider_kind.trim().to_ascii_lowercase()
        && existing.model.provider_account_id == input.provider_account_id.trim()
        && existing.model.model_profile.as_deref() == Some(input.model_profile.trim())
        && existing.model.reasoning_effort == requested_reasoning_effort;
    if !input.enabled && !retains_exact_route {
        return Err(async_graphql::Error::new(
            "A disabled task model entry must retain its existing provider route",
        ));
    }
    if retains_exact_route && (existing.enabled || !input.enabled) {
        let model_profile = existing.model.model_profile.ok_or_else(|| {
            async_graphql::Error::new("task model pool entry has no model profile")
        })?;
        let pool_entry = noema_tasks::NewTaskModelPoolEntry {
            pool_entry_id: Some(normalized_pool_entry_id.clone()),
            complexity: input.complexity.into(),
            label: input.label,
            provider_kind: existing.model.provider_kind,
            provider_account_id: existing.model.provider_account_id,
            model_profile,
            reasoning_effort: existing.model.reasoning_effort,
            enabled: input.enabled,
            sort_order: i64::from(input.sort_order),
        };
        return store
            .update_task_model_pool_entry(&normalized_pool_entry_id, pool_entry)
            .await
            .map(Into::into)
            .map_err(graphql_error);
    }
    let account = selectable_model_account(state, &input.provider_account_id).await?;
    if let Some(reason) = provider_disabled_reason(&account) {
        return Err(async_graphql::Error::new(reason));
    }
    if input.provider_kind != account.provider_kind {
        return Err(async_graphql::Error::new(
            "provider kind does not match provider account",
        ));
    }
    let profiles = selectable_profiles_from_account(store, &account).await?;
    let profile = require_selectable_profile(&profiles, &input.model_profile)?;
    let reasoning_effort = validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
    let ready_selection = crate::graphql::provider_selection::prove_ready_selection(
        state,
        &account.provider_kind,
        &account.provider_account_id,
        &input.model_profile,
        reasoning_effort,
        "graphql_task_model_pool",
    )
    .await?;
    let pool_entry = noema_tasks::NewTaskModelPoolEntry {
        pool_entry_id: Some(normalized_pool_entry_id.clone()),
        complexity: input.complexity.into(),
        label: input.label,
        provider_kind: account.provider_kind,
        provider_account_id: account.provider_account_id,
        model_profile: input.model_profile,
        reasoning_effort,
        enabled: input.enabled,
        sort_order: i64::from(input.sort_order),
    };
    store
        .update_task_model_pool_entry_with_ready_selection(
            &normalized_pool_entry_id,
            pool_entry,
            &ready_selection,
        )
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

fn positive_limit(value: i32, field: &'static str) -> Result<u32> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| super::invalid_input_error(field))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn policy_update_preserves_work_owned_bounds() {
        let store = crate::test_support::test_store().await;
        let initial = noema_tasks::TaskExecutionPolicy {
            max_provider_continuations: 80,
            max_tool_calls: 400,
            max_active_minutes: 120,
            progress_audit_interval: 20,
            max_automatic_retries: 7,
            max_review_rounds: 9,
        };
        store
            .update_task_execution_policy(initial)
            .await
            .expect("seed policy");
        let state = GraphqlState::for_tests_with_store(store.clone());

        let updated = update_task_execution_policy(
            &state,
            "human:local",
            GraphqlTaskExecutionPolicyInput {
                max_provider_continuations: 60,
                max_tool_calls: 300,
                max_active_minutes: 90,
                progress_audit_interval: 15,
            },
        )
        .await
        .expect("update policy");

        assert_eq!(updated.max_provider_continuations, 60);
        assert_eq!(updated.max_tool_calls, 300);
        assert_eq!(updated.max_active_minutes, 90);
        assert_eq!(updated.progress_audit_interval, 15);
        assert_eq!(updated.max_automatic_retries, 7);
        assert_eq!(updated.max_review_rounds, 9);
        assert_eq!(
            store
                .get_task_execution_policy()
                .await
                .expect("read policy"),
            noema_tasks::TaskExecutionPolicy {
                max_provider_continuations: 60,
                max_tool_calls: 300,
                max_active_minutes: 90,
                progress_audit_interval: 15,
                max_automatic_retries: 7,
                max_review_rounds: 9,
            }
        );
    }

    #[tokio::test]
    async fn stale_pool_route_metadata_can_be_edited_and_disabled() {
        let store = crate::test_support::test_store().await;
        crate::test_support::authenticated_default_provider(&store).await;
        let provider_registry = crate::test_support::ready_test_provider_registry();
        let entry = store
            .ensure_default_task_model_pool_settings_with_readiness(
                "codex",
                provider_registry.as_ref(),
            )
            .await
            .expect("task model settings")
            .into_iter()
            .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
            .expect("simple task model");
        let pool_entry_id = entry.pool_entry_id.clone();
        let state = GraphqlState::for_tests_with_store(store.clone())
            .with_provider_registry(std::sync::Arc::new(noema_providers::ProviderRegistry::new()));
        let input = GraphqlTaskModelPoolEntryInput {
            complexity: entry.complexity.into(),
            label: Some("Unavailable but editable".to_string()),
            provider_kind: entry.model.provider_kind,
            provider_account_id: entry.model.provider_account_id,
            model_profile: entry.model.model_profile.expect("model profile"),
            reasoning_effort: entry.model.reasoning_effort.map(Into::into),
            enabled: true,
            sort_order: i32::try_from(entry.sort_order).expect("sort order"),
        };

        let edited = update_task_model_pool_entry(
            &state,
            "human:local",
            pool_entry_id.clone(),
            input.clone(),
        )
        .await
        .expect("edit stale route metadata");
        assert!(edited.enabled);
        assert_eq!(edited.label.as_deref(), Some("Unavailable but editable"));

        let disabled = update_task_model_pool_entry(
            &state,
            "human:local",
            pool_entry_id.clone(),
            GraphqlTaskModelPoolEntryInput {
                label: edited.label,
                enabled: false,
                ..input
            },
        )
        .await
        .expect("disable stale route");

        assert!(!disabled.enabled);
        assert!(
            !store
                .get_task_model_pool_entry(&pool_entry_id)
                .await
                .expect("pool entry")
                .expect("persisted entry")
                .enabled
        );
    }
}
