use noema_providers::{ProviderAccountStatus, ReasoningEffort, provider_account_instance_key};
use noema_tasks::{NewTaskModelPoolEntry, TaskComplexity, TaskModelPoolEntry};

use super::LEGACY_PROVIDER_DEFAULT_POOL_PREFIX;
use crate::tests::{ready_codex_registry, ready_provider_selection, test_store};

async fn authenticate_default_account(store: &crate::NoemaStore, provider_kind: &str) {
    let account_id = format!("provider_account:{provider_kind}:default");
    match provider_kind {
        "codex" => {
            store
                .ensure_default_provider_account()
                .await
                .expect("codex account");
        }
        "foundation_local" => {
            store
                .ensure_default_foundation_local_provider_account()
                .await
                .expect("foundation account");
        }
        _ => panic!("unsupported test provider: {provider_kind}"),
    }
    store
        .update_provider_account_status(
            &account_id,
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
}

#[tokio::test]
async fn provider_defaults_seed_one_global_setting_per_tier_idempotently() {
    let store = test_store().await;
    authenticate_default_account(&store, "codex").await;
    let registry = ready_codex_registry();

    let first = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
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
    let expected_key = provider_account_instance_key("provider_account:codex:default")
        .expect("hosted provider key");
    assert!(
        first
            .iter()
            .all(|entry| entry.model.provider_instance_key.as_ref() == Some(&expected_key))
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
async fn default_pool_creation_requires_an_authenticated_provider() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    let error = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect_err("unknown provider must not gain future task references");
    assert!(matches!(
        error,
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));

    store
        .update_provider_account_status(
            "provider_account:codex:default",
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated account");

    let error = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect_err("authenticated metadata is not runtime readiness");
    assert!(matches!(
        error,
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));
    let registry = ready_codex_registry();
    store
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("registered defaults");
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
    authenticate_default_account(&store, "codex").await;
    let registry = ready_codex_registry();
    let settings = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("defaults");
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
    assert!(matches!(
        error,
        crate::StoreError::ProviderInstanceUnavailable {
            provider_instance_key,
        } if provider_instance_key == "gpt-5.6-luna"
    ));
}

#[tokio::test]
async fn ensuring_defaults_preserves_user_edits() {
    let store = test_store().await;
    authenticate_default_account(&store, "codex").await;
    let registry = ready_codex_registry();
    let defaults = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("defaults");
    let simple = defaults
        .into_iter()
        .find(|entry| {
            entry.model.provider_kind == "codex" && entry.complexity == TaskComplexity::Simple
        })
        .expect("simple default");

    let input = NewTaskModelPoolEntry {
        pool_entry_id: Some(simple.pool_entry_id.clone()),
        complexity: TaskComplexity::Simple,
        label: Some("My fast model".to_string()),
        provider_kind: "codex".to_string(),
        provider_account_id: "provider_account:codex:default".to_string(),
        model_profile: "gpt-5.6-terra".to_string(),
        reasoning_effort: Some(ReasoningEffort::High),
        enabled: true,
        sort_order: 0,
    };
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            &input.provider_kind,
            &input.provider_account_id,
            &input.model_profile,
            input.reasoning_effort,
            Some("task_pool_test".to_string()),
        ));
    store
        .update_task_model_pool_entry_with_ready_selection(
            &simple.pool_entry_id,
            input,
            &ready_selection,
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
    authenticate_default_account(&store, "codex").await;
    authenticate_default_account(&store, "foundation_local").await;
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
                        let provider_instance_key = provider_account_instance_key(account_id)
                            .expect("hosted instance key");
                        conn.execute(
                            "INSERT INTO task_model_pool_entries (pool_entry_id, complexity, provider_kind, provider_account_id, provider_instance_key, model_profile, reasoning_effort, enabled, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0)",
                            rusqlite::params![
                                format!("{LEGACY_PROVIDER_DEFAULT_POOL_PREFIX}{account_id}:{complexity}"),
                                complexity,
                                provider_kind,
                                account_id,
                                provider_instance_key.as_str(),
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

#[tokio::test]
async fn stale_hosted_route_can_be_edited_and_disabled_without_readiness() {
    let store = test_store().await;
    authenticate_default_account(&store, "codex").await;
    let registry = ready_codex_registry();
    let simple = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("seed defaults")
        .into_iter()
        .find(|entry| entry.complexity == TaskComplexity::Simple)
        .expect("simple setting");
    drop(registry);

    let renamed = store
        .update_task_model_pool_entry(
            &simple.pool_entry_id,
            pool_update(&simple, true, Some("Unavailable but named")),
        )
        .await
        .expect("metadata-only edit retains the exact future reference");
    assert!(renamed.enabled);
    assert_eq!(renamed.model, simple.model);

    let mut redirected_disable = pool_update(&renamed, false, Some("Ambiguous redirect"));
    redirected_disable.model_profile = "gpt-different".to_string();
    let error = store
        .update_task_model_pool_entry(&renamed.pool_entry_id, redirected_disable)
        .await
        .expect_err("disabling cannot silently redirect the stored route");
    assert!(matches!(
        error,
        crate::StoreError::InvariantViolation { .. }
    ));

    let disabled = store
        .update_task_model_pool_entry(
            &renamed.pool_entry_id,
            pool_update(&renamed, false, Some("Disabled stale route")),
        )
        .await
        .expect("disabling removes the future reference");
    assert!(!disabled.enabled);
    assert_eq!(disabled.model, renamed.model);

    let error = store
        .update_task_model_pool_entry(
            &disabled.pool_entry_id,
            pool_update(&disabled, true, Some("Cannot re-enable")),
        )
        .await
        .expect_err("re-enabling establishes a future reference");
    assert!(matches!(
        error,
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));
}

#[tokio::test]
async fn runtime_retired_local_route_can_be_disabled_without_readiness() {
    let store = test_store().await;
    store
        .ensure_default_local_models_provider_account()
        .await
        .expect("local provider account");
    store
        .update_provider_account_status(
            noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated local account");
    let installation_id = "local_model_installation:retired-pool";
    let model_id = "retired-pool-model";
    let key = noema_providers::local_model_provider_instance_key(
        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        installation_id,
        model_id,
    )
    .expect("local provider key");
    store
        .with_connection(|connection| {
            connection.execute(
                r#"
                INSERT INTO local_model_installations (
                  installation_id, provider_instance_key, model_id, display_name,
                  source_kind, source_file, sha256, download_gb, expected_bytes,
                  downloaded_bytes, backend, status, blob_relative_path, is_active,
                  runtime_retired_at, installed_at
                ) VALUES (?1, ?2, ?3, 'Retired pool model', 'local_file',
                          'retired-pool.gguf', ?4, 1.0, 1, 1, 'metal', 'installed',
                          'models/blobs/retired-pool.gguf', 0,
                          strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                          strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                "#,
                rusqlite::params![installation_id, key.as_str(), model_id, "c".repeat(64)],
            )?;
            connection.execute(
                r#"
                INSERT INTO task_model_pool_entries (
                  pool_entry_id, complexity, provider_kind, provider_account_id,
                  provider_instance_key, model_profile, enabled, sort_order
                ) VALUES ('task_pool:setting:simple', 'simple', 'local_models', ?1,
                          ?2, ?3, 1, 0)
                "#,
                rusqlite::params![
                    noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
                    key.as_str(),
                    model_id,
                ],
            )?;
            Ok(())
        })
        .await
        .expect("retired local pool row");
    let existing = store
        .get_task_model_pool_entry("task_pool:setting:simple")
        .await
        .expect("pool read")
        .expect("local setting");

    let disabled = store
        .update_task_model_pool_entry(
            &existing.pool_entry_id,
            pool_update(&existing, false, Some("Retired local route")),
        )
        .await
        .expect("retired local route can be disabled");

    assert!(!disabled.enabled);
    assert_eq!(disabled.model, existing.model);
    assert_eq!(disabled.model.provider_instance_key.as_ref(), Some(&key));
}

fn pool_update(
    existing: &TaskModelPoolEntry,
    enabled: bool,
    label: Option<&str>,
) -> NewTaskModelPoolEntry {
    NewTaskModelPoolEntry {
        pool_entry_id: Some(existing.pool_entry_id.clone()),
        complexity: existing.complexity,
        label: label.map(str::to_string),
        provider_kind: existing.model.provider_kind.clone(),
        provider_account_id: existing.model.provider_account_id.clone(),
        model_profile: existing
            .model
            .model_profile
            .clone()
            .expect("pool entries have explicit model profiles"),
        reasoning_effort: existing.model.reasoning_effort,
        enabled,
        sort_order: existing.sort_order,
    }
}
