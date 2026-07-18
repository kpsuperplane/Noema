use crate::{StoreConfig, test_support};

pub(super) fn store_config(root: &std::path::Path) -> StoreConfig {
    StoreConfig::new(root.join("db/noema.sqlite3"))
}

pub(crate) async fn test_store() -> crate::NoemaStore {
    test_support::open_ephemeral_store()
        .await
        .expect("open store")
}

pub(crate) fn ready_provider_selection(
    selection: noema_providers::ProviderSelectionSnapshot,
) -> noema_providers::ProviderReadySelection {
    test_support::ready_provider_selection(selection).expect("prove ready selection")
}

pub(crate) fn ready_provider_registry(
    selection: &noema_providers::ProviderSelectionSnapshot,
) -> noema_providers::ProviderRegistry {
    test_support::ready_provider_registry(selection).expect("register test provider")
}

pub(crate) fn provider_selection(
    provider_kind: &str,
    provider_account_id: &str,
    model_profile: &str,
    source: &str,
) -> noema_providers::ProviderSelectionSnapshot {
    noema_providers::ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        None,
        Some(source.to_string()),
    )
}

pub(crate) fn exact_provider_selection(
    provider_kind: &str,
    provider_account_id: &str,
    model_profile: &str,
    provider_instance_key: noema_providers::ProviderInstanceKey,
    source: &str,
) -> noema_providers::ProviderSelectionSnapshot {
    let mut selection =
        provider_selection(provider_kind, provider_account_id, model_profile, source);
    selection.provider_instance_key = Some(provider_instance_key);
    selection
}

pub(crate) fn ready_codex_registry() -> noema_providers::ProviderRegistry {
    ready_provider_registry(&provider_selection(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        "store_test_runtime",
    ))
}

pub(crate) fn local_model_installation(
    installation_id: &str,
    model_id: &str,
    backend: noema_providers::LocalModelBackend,
) -> noema_providers::NewLocalModelInstallation {
    noema_providers::NewLocalModelInstallation {
        installation_id: installation_id.to_string(),
        model_id: model_id.to_string(),
        display_name: "Test local model".to_string(),
        source_kind: noema_providers::LocalModelSourceKind::Catalog,
        source_repo: Some("example/model".to_string()),
        source_revision: Some("a".repeat(40)),
        source_file: Some("model.gguf".to_string()),
        sha256: Some("b".repeat(64)),
        download_gb: 1.0,
        expected_bytes: Some(100),
        license: Some("Apache-2.0".to_string()),
        backend,
    }
}

pub(crate) async fn mark_local_model_installed(
    store: &crate::NoemaStore,
    installation: &noema_providers::LocalModelInstallationRecord,
) {
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
                    downloaded_bytes: if status
                        == noema_providers::LocalModelInstallationStatus::Downloading
                    {
                        50
                    } else {
                        100
                    },
                    expected_bytes: Some(100),
                    sha256: None,
                    blob_relative_path: (status
                        == noema_providers::LocalModelInstallationStatus::Installed)
                        .then(|| "models/blobs/model.gguf".to_string()),
                    error_code: None,
                    error_message: None,
                },
            )
            .await
            .expect("advance local-model installation");
    }
}

pub(crate) fn ready_local_selection(
    installation: &noema_providers::LocalModelInstallationRecord,
) -> noema_providers::ProviderReadySelection {
    ready_provider_selection(exact_provider_selection(
        "local_models",
        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &installation.model_id,
        installation.provider_instance_key.clone(),
        "local_model_test",
    ))
}
