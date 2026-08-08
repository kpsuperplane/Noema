use noema_providers::{ProviderAccountStatus, ReasoningEffort, provider_account_instance_key};
use noema_tasks::{
    NewTaskModelPoolEntry, TaskComplexity, TaskModelPoolEntry, is_global_task_model_pool_setting_id,
};

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
async fn usable_defaults_require_an_authenticated_provider() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    let unavailable_registry = noema_providers::ProviderRegistry::new();
    assert!(matches!(
        store
            .ensure_default_task_model_pool_settings_with_readiness("codex", &unavailable_registry)
            .await
            .expect_err("unready provider"),
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));
    authenticate_default_account(&store, "codex").await;
    assert!(matches!(
        store
            .ensure_default_task_model_pool_settings_with_readiness("codex", &unavailable_registry)
            .await
            .expect_err("authenticated metadata is not runtime readiness"),
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));
    let registry = ready_codex_registry();

    let first = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("defaults");
    let second = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("idempotent defaults");

    assert_eq!(first, second);
    assert_eq!(first.len(), 3);
    assert!(
        first
            .iter()
            .all(|entry| is_global_task_model_pool_setting_id(&entry.pool_entry_id))
    );
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
    assert_eq!(
        simple.preference,
        noema_providers::ModelPreferenceSelection::NoemaRecommended
    );

    let input = NewTaskModelPoolEntry {
        pool_entry_id: Some(simple.pool_entry_id.clone()),
        complexity: TaskComplexity::Simple,
        label: Some("My fast model".to_string()),
        provider_kind: "codex".to_string(),
        provider_account_id: "provider_account:codex:default".to_string(),
        selection: noema_providers::ModelPreferenceSelection::ExplicitProfile {
            model_profile: "gpt-5.6-terra".to_string(),
            reasoning_effort: Some(ReasoningEffort::High),
        },
        enabled: true,
        sort_order: 0,
    };
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            &input.provider_kind,
            &input.provider_account_id,
            "gpt-5.6-terra",
            Some(ReasoningEffort::High),
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
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("defaults after override");
    let edited = entries
        .into_iter()
        .find(|entry| entry.pool_entry_id == simple.pool_entry_id)
        .expect("edited default");
    assert_eq!(edited.label.as_deref(), Some("My fast model"));
    assert_eq!(edited.model.model_profile.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(edited.model.reasoning_effort, Some(ReasoningEffort::High));
    assert!(matches!(
        edited.preference,
        noema_providers::ModelPreferenceSelection::ExplicitProfile { .. }
    ));
}

#[tokio::test]
async fn unavailable_exact_route_edits_cannot_redirect_or_reenable_it() {
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
                  provider_instance_key, selection_mode, model_profile, enabled, sort_order
                ) VALUES ('task_pool:setting:simple', 'simple', 'local_models', ?1,
                          ?2, 'explicit_profile', ?3, 1, 0)
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

    assert_unavailable_route_edit_rules(&store, existing, "different-model").await;

    let hosted = test_store().await;
    authenticate_default_account(&hosted, "codex").await;
    let registry = ready_codex_registry();
    let simple = hosted
        .ensure_default_task_model_pool_settings_with_readiness("codex", &registry)
        .await
        .expect("hosted defaults")
        .into_iter()
        .find(|entry| entry.complexity == TaskComplexity::Simple)
        .expect("hosted simple setting");
    drop(registry);
    assert_unavailable_route_edit_rules(&hosted, simple, "gpt-different").await;
}

async fn assert_unavailable_route_edit_rules(
    store: &crate::NoemaStore,
    existing: TaskModelPoolEntry,
    redirected_profile: &str,
) {
    let renamed = store
        .update_task_model_pool_entry(
            &existing.pool_entry_id,
            pool_update(&existing, true, Some("Unavailable but named")),
        )
        .await
        .expect("metadata-only edit retains the exact future reference");
    assert_eq!(renamed.model, existing.model);

    let mut redirected_disable = pool_update(&renamed, false, Some("Ambiguous redirect"));
    redirected_disable.selection = noema_providers::ModelPreferenceSelection::ExplicitProfile {
        model_profile: redirected_profile.to_string(),
        reasoning_effort: existing.model.reasoning_effort,
    };
    assert!(matches!(
        store
            .update_task_model_pool_entry(&renamed.pool_entry_id, redirected_disable)
            .await
            .expect_err("disabling cannot redirect the route"),
        crate::StoreError::InvariantViolation { .. }
    ));

    let disabled = store
        .update_task_model_pool_entry(
            &renamed.pool_entry_id,
            pool_update(&renamed, false, Some("Retired local route")),
        )
        .await
        .expect("retired local route can be disabled");

    assert!(!disabled.enabled);
    assert_eq!(disabled.model, renamed.model);
    assert!(matches!(
        store
            .update_task_model_pool_entry(
                &disabled.pool_entry_id,
                pool_update(&disabled, true, Some("Cannot re-enable")),
            )
            .await
            .expect_err("re-enabling establishes a future reference"),
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));
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
        selection: existing.preference.clone(),
        enabled,
        sort_order: existing.sort_order,
    }
}
