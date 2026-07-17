use noema_providers::{ProviderAccountStatus, ReasoningEffort};
use noema_tasks::{NewTaskModelPoolEntry, TaskComplexity, TaskModelPoolEntry};

use super::LEGACY_PROVIDER_DEFAULT_POOL_PREFIX;
use crate::tests::test_store;

#[tokio::test]
async fn provider_defaults_seed_one_global_setting_per_tier_idempotently() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");

    let first = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("defaults");
    let second = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("idempotent defaults");

    assert_eq!(first, second);
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(TaskModelPoolEntry::is_global_setting));
    assert!(
        first
            .iter()
            .all(|entry| entry.model.provider_kind == "codex")
    );
    assert!(first.iter().any(|entry| {
        entry.complexity == TaskComplexity::Simple
            && entry.model.model_profile.as_deref() == Some("gpt-5.6-luna")
            && entry.model.reasoning_effort == Some(ReasoningEffort::Medium)
    }));
    assert!(first.iter().any(|entry| {
        entry.complexity == TaskComplexity::Medium
            && entry.model.model_profile.as_deref() == Some("gpt-5.6-luna")
            && entry.model.reasoning_effort == Some(ReasoningEffort::XHigh)
    }));
    assert!(first.iter().any(|entry| {
        entry.complexity == TaskComplexity::Difficult
            && entry.model.model_profile.as_deref() == Some("gpt-5.6-sol")
            && entry.model.reasoning_effort == Some(ReasoningEffort::High)
    }));
}

#[tokio::test]
async fn usable_defaults_require_an_authenticated_provider() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("defaults");

    assert!(
        store
            .list_usable_task_model_pool_entries()
            .await
            .expect("unavailable defaults")
            .is_empty()
    );

    store
        .update_provider_account_status(
            "provider_account:codex:default",
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated account");

    let usable = store
        .list_usable_task_model_pool_entries()
        .await
        .expect("usable defaults");
    assert_eq!(usable.len(), 3);
    assert!(usable.iter().all(|entry| entry.enabled));
}

#[tokio::test]
async fn selection_rejects_a_profile_missing_from_the_provider_catalog() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    let settings = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("defaults");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated account");
    store
        .update_provider_account_metadata(
            "provider_account:codex:default",
            serde_json::json!({"profiles": [{"id": "gpt-live"}]}),
        )
        .await
        .expect("catalog");
    let simple = settings
        .iter()
        .find(|entry| entry.complexity == TaskComplexity::Simple)
        .expect("simple setting");

    let error = store
        .select_task_model_pool_entry(TaskComplexity::Simple, &simple.pool_entry_id)
        .await
        .expect_err("stale model must be rejected");
    assert!(error.to_string().contains("is not available"));
}

#[tokio::test]
async fn ensuring_defaults_preserves_user_edits() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    let defaults = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("defaults");
    let simple = defaults
        .into_iter()
        .find(|entry| {
            entry.model.provider_kind == "codex" && entry.complexity == TaskComplexity::Simple
        })
        .expect("simple default");

    store
        .update_task_model_pool_entry(
            &simple.pool_entry_id,
            NewTaskModelPoolEntry {
                pool_entry_id: Some(simple.pool_entry_id.clone()),
                complexity: TaskComplexity::Simple,
                label: Some("My fast model".to_string()),
                provider_kind: "codex".to_string(),
                provider_account_id: "provider_account:codex:default".to_string(),
                model_profile: "gpt-5.6-terra".to_string(),
                reasoning_effort: Some(ReasoningEffort::High),
                enabled: true,
                sort_order: 0,
            },
        )
        .await
        .expect("override");

    let entries = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("defaults after override");
    let edited = entries
        .into_iter()
        .find(|entry| entry.pool_entry_id == simple.pool_entry_id)
        .expect("edited default");
    assert_eq!(edited.label.as_deref(), Some("My fast model"));
    assert_eq!(edited.model.model_profile.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(edited.model.reasoning_effort, Some(ReasoningEffort::High));
}

#[tokio::test]
async fn provider_scoped_defaults_are_consolidated_into_three_global_settings() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
            .with_connection(|conn| {
                for (provider_kind, account_id, profile, effort) in [
                    (
                        "codex",
                        "provider_account:codex:default",
                        "gpt-5.6-luna",
                        Some("medium"),
                    ),
                    (
                        "foundation_local",
                        "provider_account:foundation_local:default",
                        "default",
                        None,
                    ),
                ] {
                    for complexity in ["simple", "medium", "difficult"] {
                        conn.execute(
                            "INSERT INTO task_model_pool_entries (pool_entry_id, complexity, provider_kind, provider_account_id, model_profile, reasoning_effort, enabled, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0)",
                            rusqlite::params![
                                format!("{LEGACY_PROVIDER_DEFAULT_POOL_PREFIX}{account_id}:{complexity}"),
                                complexity,
                                provider_kind,
                                account_id,
                                profile,
                                effort,
                            ],
                        )?;
                    }
                }
                Ok(())
            })
            .await
            .expect("legacy defaults");

    let settings = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("global settings");
    let all_entries = store
        .list_task_model_pool_entries(None)
        .await
        .expect("all entries");

    assert_eq!(settings.len(), 3);
    assert_eq!(all_entries, settings);
    assert!(
        settings
            .iter()
            .all(|entry| entry.model.provider_kind == "codex")
    );
}
