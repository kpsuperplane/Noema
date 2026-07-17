use std::sync::Arc;

use noema_capabilities::{CapabilityId, ToolName};

use super::*;
use crate::{
    LocalModelBackend, LocalModelEventRecord, LocalModelInstallationRecord,
    LocalModelInstallationStatus, LocalModelInstallationUpdate, LocalModelSourceKind,
    NewLocalModelInstallation, NewProviderAccount, PersistedProviderAccountRecord,
    ProviderCapabilityAccountReference, ProviderCapabilityAssignment,
    ProviderCapabilityAssignmentKey, RemovedLocalModelInstallation,
};

fn unsupported<T>(operation: &'static str) -> ProviderPersistenceFuture<'static, T> {
    Box::pin(async move { Err(ProviderPersistenceError::Persistence { operation }) })
}

struct AccountFake;

impl ProviderAccountPersistence for AccountFake {
    fn provider_account<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<PersistedProviderAccountRecord>> {
        unsupported("provider_account")
    }

    fn active_provider_account<'a>(
        &'a self,
        _provider_kind: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<PersistedProviderAccountRecord>> {
        unsupported("active_provider_account")
    }

    fn active_default_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>> {
        unsupported("active_default_provider_accounts")
    }

    fn active_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>> {
        unsupported("active_provider_accounts")
    }

    fn provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>> {
        unsupported("provider_accounts")
    }

    fn create_provider_account(
        &self,
        _request: NewProviderAccount,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord> {
        unsupported("create_provider_account")
    }

    fn update_provider_account(
        &self,
        _request: UpdateProviderAccountRequest,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord> {
        unsupported("update_provider_account")
    }

    fn delete_provider_account<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, bool> {
        unsupported("delete_provider_account")
    }
}

struct CapabilityFake;

impl ProviderCapabilityAssignmentPersistence for CapabilityFake {
    fn provider_capability_assignment<'a>(
        &'a self,
        _key: &'a ProviderCapabilityAssignmentKey,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderCapabilityAssignment>> {
        unsupported("provider_capability_assignment")
    }

    fn upsert_provider_capability_assignment(
        &self,
        _request: UpsertProviderCapabilityAssignmentRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderCapabilityAssignment> {
        unsupported("upsert_provider_capability_assignment")
    }
}

struct CatalogFake;

impl ProviderModelCatalogPersistence for CatalogFake {
    fn persist_provider_model_catalog(
        &self,
        _request: PersistProviderModelCatalogRequest,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord> {
        unsupported("persist_provider_model_catalog")
    }
}

struct InstallationFake;

impl LocalModelInstallationPersistence for InstallationFake {
    fn upsert_local_model_installation(
        &self,
        _input: NewLocalModelInstallation,
    ) -> ProviderPersistenceFuture<'_, LocalModelInstallationRecord> {
        unsupported("upsert_local_model_installation")
    }

    fn local_model_installation<'a>(
        &'a self,
        _installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>> {
        unsupported("local_model_installation")
    }

    fn installed_local_model<'a>(
        &'a self,
        _model_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>> {
        unsupported("installed_local_model")
    }

    fn local_model_installations(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelInstallationRecord>> {
        unsupported("local_model_installations")
    }

    fn update_local_model_installation<'a>(
        &'a self,
        _installation_id: &'a str,
        _update: LocalModelInstallationUpdate,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        unsupported("update_local_model_installation")
    }

    fn cancel_local_model_installation<'a>(
        &'a self,
        _installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        unsupported("cancel_local_model_installation")
    }

    fn remove_terminal_local_model_installation<'a>(
        &'a self,
        _installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation> {
        unsupported("remove_terminal_local_model_installation")
    }

    fn local_model_events(
        &self,
        _after_cursor: Option<u64>,
        _limit: u32,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelEventRecord>> {
        unsupported("local_model_events")
    }
}

struct ActivationFake;

impl LocalModelActivationPersistence for ActivationFake {
    fn activate_local_model_as_system_default<'a>(
        &'a self,
        _installation_id: &'a str,
        _ready_selection: &'a crate::ProviderReadySelection,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord> {
        unsupported("activate_local_model")
    }
}

#[tokio::test]
async fn every_persistence_port_is_object_safe_and_independently_erasable() {
    let account: ProviderAccountPersistenceHandle = Arc::new(AccountFake);
    assert!(account.provider_account("account").await.is_err());
    assert!(account.active_provider_account("codex").await.is_err());
    assert!(account.active_default_provider_accounts().await.is_err());
    assert!(account.active_provider_accounts().await.is_err());
    assert!(account.provider_accounts().await.is_err());
    assert!(
        account
            .create_provider_account(NewProviderAccount {
                provider_kind: "exa".to_string(),
                display_name: None,
                auth_method: crate::ProviderAuthMethod::SecretInput,
                status: crate::ProviderAccountStatus::Unauthenticated,
                metadata: serde_json::json!({}),
            })
            .await
            .is_err()
    );
    assert!(
        account
            .update_provider_account(UpdateProviderAccountRequest {
                provider_account_id: "account".to_string(),
                status: None,
                metadata: Some(serde_json::json!({})),
            })
            .await
            .is_err()
    );
    assert!(account.delete_provider_account("account").await.is_err());

    let capability: ProviderCapabilityAssignmentPersistenceHandle = Arc::new(CapabilityFake);
    let tool_name = ToolName::new("web.search").expect("tool");
    let key = ProviderCapabilityAssignmentKey::new(tool_name.clone(), CapabilityId::WebSearch)
        .expect("assignment key");
    assert!(
        capability
            .provider_capability_assignment(&key)
            .await
            .is_err()
    );
    assert!(
        capability
            .upsert_provider_capability_assignment(
                UpsertProviderCapabilityAssignmentRequest::new(
                    tool_name,
                    CapabilityId::WebSearch,
                    ProviderCapabilityAccountReference::persisted("account"),
                )
                .expect("assignment request"),
            )
            .await
            .is_err()
    );

    let catalog: ProviderModelCatalogPersistenceHandle = Arc::new(CatalogFake);
    assert!(
        catalog
            .persist_provider_model_catalog(PersistProviderModelCatalogRequest {
                provider_account_id: "account".to_string(),
                profiles: Vec::new(),
                refreshed_at_unix: 1,
                source: "test".to_string(),
                metadata_version: 1,
                client_version: "1.0.0".to_string(),
                client_version_refreshed_at_unix: None,
                resulting_status: crate::ProviderAccountStatus::Authenticated,
            })
            .await
            .is_err()
    );

    let installations: LocalModelInstallationPersistenceHandle = Arc::new(InstallationFake);
    assert!(
        installations
            .upsert_local_model_installation(local_model_installation())
            .await
            .is_err()
    );
    assert!(installations.local_model_installation("id").await.is_err());
    assert!(installations.installed_local_model("model").await.is_err());
    assert!(installations.local_model_installations().await.is_err());
    assert!(
        installations
            .update_local_model_installation(
                "id",
                LocalModelInstallationUpdate {
                    status: LocalModelInstallationStatus::Downloading,
                    downloaded_bytes: 1,
                    expected_bytes: Some(2),
                    sha256: None,
                    blob_relative_path: None,
                    error_code: None,
                    error_message: None,
                },
            )
            .await
            .is_err()
    );
    assert!(
        installations
            .cancel_local_model_installation("id")
            .await
            .is_err()
    );
    assert!(
        installations
            .remove_terminal_local_model_installation("id")
            .await
            .is_err()
    );
    assert!(installations.local_model_events(None, 1).await.is_err());

    let _activation: LocalModelActivationPersistenceHandle = Arc::new(ActivationFake);
}

fn local_model_installation() -> NewLocalModelInstallation {
    NewLocalModelInstallation {
        installation_id: "installation".to_string(),
        model_id: "model".to_string(),
        display_name: "Model".to_string(),
        source_kind: LocalModelSourceKind::Catalog,
        source_repo: Some("example/model".to_string()),
        source_revision: Some("a".repeat(40)),
        source_file: Some("model.gguf".to_string()),
        sha256: Some("b".repeat(64)),
        download_gb: 1.0,
        expected_bytes: Some(2),
        license: None,
        backend: LocalModelBackend::Cpu,
    }
}
