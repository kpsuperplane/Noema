//! Contract-only local-model operations for API tests.

use std::sync::Arc;

use noema_providers::{
    HuggingFaceLocalModelImport, LocalFileModelImport, LocalHardwareProfile, LocalModelBackend,
    LocalModelBuild, LocalModelCatalogEntry, LocalModelCatalogSnapshot,
    LocalModelCatalogSnapshotEntry, LocalModelEventRecord, LocalModelInstallationRecord,
    LocalModelManagement, LocalModelManagementFuture, LocalModelManager, LocalModelManagerError,
    LocalModelManagerEventStream, LocalModelReconstructionReport, LocalModelRuntimeStatus,
    ManagedLocalModelStatus, ProviderRegistry, ProviderRegistryHandle,
    RemovedLocalModelInstallation,
};

pub(crate) fn local_model_manager(store: &noema_store::NoemaStore) -> LocalModelManager {
    local_model_manager_with_status(store, LocalModelRuntimeStatus::Stopped)
}

pub(crate) fn local_model_manager_with_status(
    store: &noema_store::NoemaStore,
    runtime_status: LocalModelRuntimeStatus,
) -> LocalModelManager {
    LocalModelManager::from_operations(Arc::new(TestLocalModelOperations {
        store: store.clone(),
        registry: Arc::new(ProviderRegistry::new()),
        runtime_status,
    }))
}

struct TestLocalModelOperations {
    store: noema_store::NoemaStore,
    registry: ProviderRegistryHandle,
    runtime_status: LocalModelRuntimeStatus,
}

impl LocalModelManagement for TestLocalModelOperations {
    fn registry(&self) -> ProviderRegistryHandle {
        self.registry.clone()
    }

    fn runtime_status(&self) -> LocalModelRuntimeStatus {
        self.runtime_status.clone()
    }

    fn installations(
        &self,
    ) -> LocalModelManagementFuture<
        '_,
        Result<Vec<LocalModelInstallationRecord>, LocalModelManagerError>,
    > {
        Box::pin(async move {
            self.store
                .list_local_model_installations()
                .await
                .map_err(store_error)
        })
    }

    fn events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> LocalModelManagementFuture<'_, Result<Vec<LocalModelEventRecord>, LocalModelManagerError>>
    {
        Box::pin(async move {
            self.store
                .list_local_model_events(after_cursor, limit)
                .await
                .map_err(store_error)
        })
    }

    fn managed_instances(&self) -> LocalModelManagementFuture<'_, Vec<ManagedLocalModelStatus>> {
        Box::pin(async { Vec::new() })
    }

    fn catalog_snapshot(&self) -> Result<LocalModelCatalogSnapshot, LocalModelManagerError> {
        Ok(test_catalog())
    }

    fn preferred_import_backend(&self) -> Result<LocalModelBackend, LocalModelManagerError> {
        Ok(LocalModelBackend::Metal)
    }

    fn install_catalog_model(
        &self,
        _model_id: String,
        _file: Option<String>,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async { Err(unsupported("install_catalog_model")) })
    }

    fn import_hugging_face(
        &self,
        _input: HuggingFaceLocalModelImport,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async { Err(unsupported("import_hugging_face")) })
    }

    fn import_local_file(
        &self,
        _input: LocalFileModelImport,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async { Err(unsupported("import_local_file")) })
    }

    fn cancel_installation(
        &self,
        _installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async { Err(unsupported("cancel_installation")) })
    }

    fn remove(
        &self,
        _installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<RemovedLocalModelInstallation, LocalModelManagerError>>
    {
        Box::pin(async { Err(unsupported("remove")) })
    }

    fn activate(
        &self,
        _installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async { Err(unsupported("activate")) })
    }

    fn retry_active_installation(
        &self,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelRuntimeStatus, LocalModelManagerError>>
    {
        let status = self.runtime_status.clone();
        Box::pin(async move { Ok(status) })
    }

    fn reconstruct_persisted_instances(
        &self,
    ) -> LocalModelManagementFuture<
        '_,
        Result<LocalModelReconstructionReport, LocalModelManagerError>,
    > {
        Box::pin(async { Ok(LocalModelReconstructionReport::default()) })
    }

    fn subscribe_events(
        &self,
        _after: Option<String>,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelManagerEventStream, LocalModelManagerError>>
    {
        Box::pin(async {
            let stream: LocalModelManagerEventStream = Box::pin(futures_util::stream::empty());
            Ok(stream)
        })
    }

    fn begin_shutdown(&self) -> LocalModelManagementFuture<'_, ()> {
        Box::pin(async {})
    }

    fn shutdown(&self) -> LocalModelManagementFuture<'_, Result<(), LocalModelManagerError>> {
        Box::pin(async { Ok(()) })
    }
}

fn test_catalog() -> LocalModelCatalogSnapshot {
    let build = LocalModelBuild {
        file: "gemma-4-E4B-it-Q4_K_M.gguf".to_string(),
        sha256: "1".repeat(64),
        download_gb: 5.3,
        backends: vec![LocalModelBackend::Metal],
        min_ram_gb: 16,
        min_vram_gb: None,
    };
    let hardware = LocalHardwareProfile::new(LocalModelBackend::Metal, 32, None, true);
    LocalModelCatalogSnapshot {
        entries: vec![LocalModelCatalogSnapshotEntry {
            model: LocalModelCatalogEntry {
                id: "gemma-4-e4b-it".to_string(),
                name: "Gemma 4 E4B IT".to_string(),
                license: "Apache-2.0".to_string(),
                priority: 100,
                repo: "ggml-org/gemma-4-E4B-it-GGUF".to_string(),
                revision: "0".repeat(40),
                builds: vec![build.clone()],
            },
            selected_build: Some(build),
            compatible_hardware: Some(hardware),
            compatibility_explanation: Some(
                "Recommended because this test machine satisfies the model requirements."
                    .to_string(),
            ),
            is_recommended: true,
        }],
    }
}

fn store_error(error: noema_store::StoreError) -> LocalModelManagerError {
    LocalModelManagerError::Runtime {
        operation: "test_store",
        message: error.to_string(),
    }
}

fn unsupported(operation: &'static str) -> LocalModelManagerError {
    LocalModelManagerError::Runtime {
        operation,
        message: "operation is not configured in this contract test".to_string(),
    }
}
