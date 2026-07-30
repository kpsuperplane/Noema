use std::sync::Arc;

use super::{
    GraphqlState,
    agents::{
        GraphqlModelPreferenceSelectionMode, GraphqlSaveAgentModelPreferenceInput, agents,
        save_agent_model_preference,
    },
    local_models::{
        GraphqlLocalModelInstallationStatus, GraphqlLocalModelRuntimeStatus,
        GraphqlSaveDefaultModelPreferenceInput, local_model_catalog, local_model_setup,
        save_default_model_preference,
    },
};

#[tokio::test]
async fn generic_default_preference_rejects_ambiguous_local_model_selection() {
    let store = crate::test_support::test_store().await;
    let state = GraphqlState::for_tests_with_store(store.clone());

    let error = save_default_model_preference(
        &state,
        GraphqlSaveDefaultModelPreferenceInput {
            provider_kind: noema_providers::ProviderKind::LocalModels
                .as_str()
                .to_string(),
            provider_account_id: noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID.to_string(),
            selection_mode: GraphqlModelPreferenceSelectionMode::ExplicitProfile,
            model_profile: Some("shared-model".to_string()),
            reasoning_effort: None,
        },
    )
    .await
    .expect_err("local changes require an exact installation");

    assert_eq!(
        error.message,
        "Activate a specific local model installation to change the local default"
    );
    assert!(
        store
            .get_default_model_preference()
            .await
            .expect("default preference")
            .is_none()
    );
}

#[tokio::test]
async fn local_model_catalog_setup_and_agent_preferences_follow_exact_installation_state() {
    let environment = crate::test_support::test_environment();
    let store = crate::test_support::test_store_for_environment(&environment).await;
    let installation = store
        .upsert_local_model_installation(noema_providers::NewLocalModelInstallation {
            installation_id: "local_model_installation:catalog:gemma-4-e4b-it:test".to_string(),
            model_id: "gemma-4-e4b-it".to_string(),
            display_name: "Gemma 4 E4B IT".to_string(),
            source_kind: noema_providers::LocalModelSourceKind::Catalog,
            source_repo: Some("ggml-org/gemma-4-E4B-it-GGUF".to_string()),
            source_revision: Some("0".repeat(40)),
            source_file: Some("gemma-4-E4B-it-Q4_K_M.gguf".to_string()),
            sha256: Some("1".repeat(64)),
            download_gb: 5.3,
            expected_bytes: Some(100),
            license: Some("Apache-2.0".to_string()),
            backend: noema_providers::LocalModelBackend::Metal,
        })
        .await
        .expect("queued installation");
    let persistence = Arc::new(store.clone());
    let manager = noema_providers::LocalModelManager::new(
        persistence.clone(),
        persistence.clone(),
        persistence,
        Arc::new(noema_providers::ProviderRegistry::new()),
        noema_home::NoemaPaths::from_noema_home(environment.root()).expect("test paths"),
        noema_providers::LocalModelManagerConfig {
            runtime_root: None,
            context_window_tokens: noema_providers::DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: noema_providers::DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: noema_providers::DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
            system_errors: None,
        },
    )
    .expect("local model manager");
    let state =
        GraphqlState::for_tests_with_store(store.clone()).with_local_model_manager(manager.clone());

    let catalog = local_model_catalog(&state).await.expect("catalog");
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].model_id, "gemma-4-e4b-it");
    assert_eq!(catalog[0].license, "Apache-2.0");
    assert!(catalog[0].is_recommended);
    assert!(catalog[0].selected_build.is_some());
    assert!(catalog[0].hardware_fit.is_some());

    let queued = local_model_setup(&state).await.expect("queued setup");
    assert_eq!(
        queued
            .installation
            .expect("recommended installation")
            .status,
        GraphqlLocalModelInstallationStatus::Queued
    );
    assert!(!queued.is_ready);

    for status in [
        noema_providers::LocalModelInstallationStatus::Downloading,
        noema_providers::LocalModelInstallationStatus::Verifying,
        noema_providers::LocalModelInstallationStatus::Installed,
    ] {
        store
            .update_local_model_installation(
                &installation.installation_id,
                noema_providers::LocalModelInstallationUpdate {
                    status,
                    downloaded_bytes: 100,
                    expected_bytes: Some(100),
                    sha256: None,
                    blob_relative_path: (status
                        == noema_providers::LocalModelInstallationStatus::Installed)
                        .then(|| "models/blobs/test.gguf".to_string()),
                    error_code: None,
                    error_message: None,
                },
            )
            .await
            .expect("installation transition");
    }
    let mut selection = noema_providers::ProviderSelectionSnapshot::explicit(
        "local_models",
        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        "gemma-4-e4b-it",
        None,
        Some("test_local_activation".to_string()),
    );
    selection.provider_instance_key = Some(installation.provider_instance_key.clone());
    let ready = crate::test_support::ready_provider_selection_in_registry(
        selection,
        manager.registry().as_ref(),
    );
    store
        .activate_local_model_as_system_default(&installation.installation_id, &ready)
        .await
        .expect("activate installation");

    let installed = local_model_setup(&state).await.expect("installed setup");
    assert_eq!(
        installed.installation.expect("active installation").status,
        GraphqlLocalModelInstallationStatus::Installed
    );
    assert_eq!(
        installed.runtime_status,
        GraphqlLocalModelRuntimeStatus::Inactive
    );
    assert!(!installed.is_ready);

    let primary = agents(&state)
        .await
        .expect("agents")
        .into_iter()
        .find(|agent| agent.agent_id == "agent:primary")
        .expect("primary agent");
    let local_models = primary
        .model_options
        .iter()
        .find(|option| option.provider_kind == "local_models")
        .expect("local model option");
    assert!(local_models.recommendations.is_empty());
    assert_eq!(local_models.profiles[0].disabled_reason, None);

    let saved = save_agent_model_preference(
        &state,
        GraphqlSaveAgentModelPreferenceInput {
            agent_id: "agent:primary".to_string(),
            provider_account_id: noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID.to_string(),
            selection_mode: GraphqlModelPreferenceSelectionMode::ExplicitProfile,
            model_profile: Some("gemma-4-e4b-it".to_string()),
            reasoning_effort: None,
        },
    )
    .await
    .expect("save local model preference");
    assert_eq!(saved.provider_kind, "local_models");
    assert_eq!(saved.model_profile.as_deref(), Some("gemma-4-e4b-it"));
}
