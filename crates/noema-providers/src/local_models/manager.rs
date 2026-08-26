//! Local-model installation, process, routing, and shutdown control plane.

use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use tokio::{
    sync::{Mutex, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use noema_home::{NoemaPaths, SystemErrorLogger};

use crate::{
    DegradedLocalModelInstance, LocalModelActivationPersistenceHandle, LocalModelCatalogSnapshot,
    LocalModelEventRecord, LocalModelInstallationPersistenceHandle, LocalModelInstallationRecord,
    LocalModelLifecyclePersistenceHandle, LocalModelManagement, LocalModelManagementFuture,
    LocalModelManager, LocalModelManagerConfig, LocalModelManagerError, LocalModelManagerEvent,
    LocalModelManagerEventRecord, LocalModelManagerEventStream, LocalModelReconstructionReport,
    LocalModelRuntimeStatus, ProviderInstanceKey, ProviderRegistration, ProviderRegistryHandle,
};

use super::{LocalModelInstallError, LocalModelInstaller};
use process::ManagedProcess;

mod events;
mod lifecycle;
mod lifecycle_validation;
mod process;
mod reaper;
mod workers;

#[cfg(test)]
pub(super) mod tests;

const LIFECYCLE_RUNNING: u8 = 0;
const LIFECYCLE_SHUTTING_DOWN: u8 = 1;
const LIFECYCLE_STOPPED: u8 = 2;
const DEFAULT_REAPER_INTERVAL: Duration = Duration::from_secs(30);
const DEFAULT_REAPER_BATCH_SIZE: usize = 8;

impl From<LocalModelInstallError> for LocalModelManagerError {
    fn from(error: LocalModelInstallError) -> Self {
        Self::Installation {
            message: error.to_string(),
        }
    }
}

/// Concrete feature-gated local-model control-plane service.
#[derive(Clone)]
pub(super) struct LocalModelManagerService {
    inner: Arc<ManagerInner>,
}

struct ManagerInner {
    installer: LocalModelInstaller,
    paths: NoemaPaths,
    installations: LocalModelInstallationPersistenceHandle,
    activation: LocalModelActivationPersistenceHandle,
    lifecycle_persistence: LocalModelLifecyclePersistenceHandle,
    registry: ProviderRegistryHandle,
    local_model_config: LocalModelManagerConfig,
    #[cfg(test)]
    fake_process_factory: Option<Arc<tests::fakes::FakeProcessFactory>>,
    system_errors: Option<SystemErrorLogger>,
    control: Mutex<()>,
    instances: StdMutex<HashMap<ProviderInstanceKey, ManagedInstance>>,
    workers: Mutex<HashMap<String, InstallationWorker>>,
    active_status_forwarder: Mutex<Option<ActiveStatusForwarder>>,
    reaper: Mutex<Option<ReaperWorker>>,
    reaper_interval: Duration,
    reaper_batch_size: usize,
    degraded: StdMutex<HashMap<ProviderInstanceKey, DegradedLocalModelInstance>>,
    runtime_status_tx: watch::Sender<LocalModelRuntimeStatus>,
    lifecycle_tx: watch::Sender<u8>,
    lifecycle: AtomicU8,
}

#[derive(Clone)]
struct ManagedInstance {
    installation: LocalModelInstallationRecord,
    registration: ProviderRegistration,
    process: Arc<ManagedProcess>,
}

struct InstallationWorker {
    cancellation: CancellationToken,
    completion: watch::Receiver<bool>,
}

struct ActiveStatusForwarder {
    cancellation: CancellationToken,
    task: JoinHandle<()>,
}

struct ReaperWorker {
    cancellation: CancellationToken,
    trigger: Arc<tokio::sync::Notify>,
    task: JoinHandle<()>,
}

impl LocalModelManager {
    /// Creates the concrete local-model management service without reconstructing persisted state.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] when configuration is invalid or the
    /// installer HTTP client cannot be built.
    pub fn new(
        installations: LocalModelInstallationPersistenceHandle,
        activation: LocalModelActivationPersistenceHandle,
        lifecycle_persistence: LocalModelLifecyclePersistenceHandle,
        registry: ProviderRegistryHandle,
        paths: NoemaPaths,
        config: LocalModelManagerConfig,
    ) -> Result<Self, LocalModelManagerError> {
        LocalModelManagerService::new(
            installations,
            activation,
            lifecycle_persistence,
            registry,
            paths,
            config,
        )
        .map(|service| Self::from_operations(Arc::new(service)))
    }
}

impl LocalModelManagerService {
    /// Creates a manager without starting the persisted active process.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] when configuration is invalid or the
    /// installer HTTP client cannot be built.
    pub fn new(
        installations: LocalModelInstallationPersistenceHandle,
        activation: LocalModelActivationPersistenceHandle,
        lifecycle_persistence: LocalModelLifecyclePersistenceHandle,
        registry: ProviderRegistryHandle,
        paths: NoemaPaths,
        config: LocalModelManagerConfig,
    ) -> Result<Self, LocalModelManagerError> {
        validate_config(&config)?;
        let system_errors = config.system_errors.clone();
        Self::build(
            installations,
            activation,
            lifecycle_persistence,
            registry,
            paths,
            config,
            #[cfg(test)]
            None,
            system_errors,
            DEFAULT_REAPER_INTERVAL,
            DEFAULT_REAPER_BATCH_SIZE,
        )
    }

    #[cfg(test)]
    fn new_with_factory(
        installations: LocalModelInstallationPersistenceHandle,
        activation: LocalModelActivationPersistenceHandle,
        lifecycle_persistence: LocalModelLifecyclePersistenceHandle,
        registry: ProviderRegistryHandle,
        paths: NoemaPaths,
        process_factory: Arc<tests::fakes::FakeProcessFactory>,
        system_errors: Option<SystemErrorLogger>,
    ) -> Result<Self, LocalModelManagerError> {
        let config = LocalModelManagerConfig {
            runtime_root: None,
            context_window_tokens: 1,
            timeout_seconds: 1,
            startup_timeout_seconds: 1,
            system_errors: system_errors.clone(),
        };
        Self::build(
            installations,
            activation,
            lifecycle_persistence,
            registry,
            paths,
            config,
            Some(process_factory),
            system_errors,
            DEFAULT_REAPER_INTERVAL,
            DEFAULT_REAPER_BATCH_SIZE,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        installations: LocalModelInstallationPersistenceHandle,
        activation: LocalModelActivationPersistenceHandle,
        lifecycle_persistence: LocalModelLifecyclePersistenceHandle,
        registry: ProviderRegistryHandle,
        paths: NoemaPaths,
        local_model_config: LocalModelManagerConfig,
        #[cfg(test)] fake_process_factory: Option<Arc<tests::fakes::FakeProcessFactory>>,
        system_errors: Option<SystemErrorLogger>,
        reaper_interval: Duration,
        reaper_batch_size: usize,
    ) -> Result<Self, LocalModelManagerError> {
        let installer = LocalModelInstaller::new(Arc::clone(&installations), paths.clone())?;
        let (runtime_status_tx, _) = watch::channel(LocalModelRuntimeStatus::Stopped);
        let (lifecycle_tx, _) = watch::channel(LIFECYCLE_RUNNING);
        Ok(Self {
            inner: Arc::new(ManagerInner {
                installer,
                paths,
                installations,
                activation,
                lifecycle_persistence,
                registry,
                local_model_config,
                #[cfg(test)]
                fake_process_factory,
                system_errors,
                control: Mutex::new(()),
                instances: StdMutex::new(HashMap::new()),
                workers: Mutex::new(HashMap::new()),
                active_status_forwarder: Mutex::new(None),
                reaper: Mutex::new(None),
                reaper_interval,
                reaper_batch_size,
                degraded: StdMutex::new(HashMap::new()),
                runtime_status_tx,
                lifecycle_tx,
                lifecycle: AtomicU8::new(LIFECYCLE_RUNNING),
            }),
        })
    }

    /// Returns the shared exact-instance registry used by runtime consumers.
    #[must_use]
    pub fn registry(&self) -> ProviderRegistryHandle {
        Arc::clone(&self.inner.registry)
    }

    /// Returns the active process status without waiting for a change.
    #[must_use]
    pub fn runtime_status(&self) -> LocalModelRuntimeStatus {
        self.inner.runtime_status_tx.borrow().clone()
    }

    /// Lists durable installations in repository presentation order.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError::Persistence`] when the repository read fails.
    pub async fn installations(
        &self,
    ) -> Result<Vec<LocalModelInstallationRecord>, LocalModelManagerError> {
        self.inner
            .installations
            .local_model_installations()
            .await
            .map_err(Into::into)
    }

    /// Returns durable installation events after an optional exclusive cursor.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError::Persistence`] when the repository read fails.
    pub async fn events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> Result<Vec<LocalModelEventRecord>, LocalModelManagerError> {
        self.inner
            .installations
            .local_model_events(after_cursor, limit)
            .await
            .map_err(Into::into)
    }

    #[cfg(test)]
    pub(super) fn managed_instance_count(&self) -> usize {
        self.inner.instances.lock().expect("instances lock").len()
    }

    fn ensure_accepting_work(&self) -> Result<(), LocalModelManagerError> {
        if self.inner.lifecycle.load(Ordering::Acquire) == LIFECYCLE_RUNNING {
            Ok(())
        } else {
            Err(LocalModelManagerError::ShuttingDown)
        }
    }
}

impl LocalModelManagement for LocalModelManagerService {
    fn registry(&self) -> ProviderRegistryHandle {
        LocalModelManagerService::registry(self)
    }

    fn runtime_status(&self) -> LocalModelRuntimeStatus {
        LocalModelManagerService::runtime_status(self)
    }

    fn installations(
        &self,
    ) -> LocalModelManagementFuture<
        '_,
        Result<Vec<LocalModelInstallationRecord>, LocalModelManagerError>,
    > {
        Box::pin(LocalModelManagerService::installations(self))
    }

    fn events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> LocalModelManagementFuture<'_, Result<Vec<LocalModelEventRecord>, LocalModelManagerError>>
    {
        Box::pin(LocalModelManagerService::events(self, after_cursor, limit))
    }

    fn catalog_snapshot(&self) -> Result<LocalModelCatalogSnapshot, LocalModelManagerError> {
        LocalModelManagerService::catalog_snapshot()
    }

    fn preferred_import_backend(&self) -> Result<crate::LocalModelBackend, LocalModelManagerError> {
        LocalModelManagerService::preferred_import_backend()
    }

    fn install_catalog_model(
        &self,
        model_id: String,
        file: Option<String>,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async move {
            LocalModelManagerService::install_catalog_model(self, &model_id, file.as_deref()).await
        })
    }

    fn import_hugging_face(
        &self,
        input: crate::HuggingFaceLocalModelImport,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(LocalModelManagerService::import_hugging_face(self, input))
    }

    fn import_local_file(
        &self,
        input: crate::LocalFileModelImport,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(LocalModelManagerService::import_local_file(self, input))
    }

    fn cancel_installation(
        &self,
        installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async move {
            LocalModelManagerService::cancel_installation(self, &installation_id).await
        })
    }

    fn remove(
        &self,
        installation_id: String,
    ) -> LocalModelManagementFuture<
        '_,
        Result<crate::RemovedLocalModelInstallation, LocalModelManagerError>,
    > {
        Box::pin(async move { LocalModelManagerService::remove(self, &installation_id).await })
    }

    fn activate(
        &self,
        installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>
    {
        Box::pin(async move { LocalModelManagerService::activate(self, &installation_id).await })
    }

    fn retry_active_installation(
        &self,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelRuntimeStatus, LocalModelManagerError>>
    {
        Box::pin(LocalModelManagerService::retry_active_installation(self))
    }

    fn reconstruct_persisted_instances(
        &self,
    ) -> LocalModelManagementFuture<
        '_,
        Result<LocalModelReconstructionReport, LocalModelManagerError>,
    > {
        Box::pin(LocalModelManagerService::reconstruct_persisted_instances(
            self,
        ))
    }

    fn subscribe_events(
        &self,
        after: Option<String>,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelManagerEventStream, LocalModelManagerError>>
    {
        Box::pin(
            async move { LocalModelManagerService::subscribe_events(self, after.as_deref()).await },
        )
    }

    fn begin_shutdown(&self) -> LocalModelManagementFuture<'_, ()> {
        Box::pin(LocalModelManagerService::begin_shutdown(self))
    }

    fn shutdown(&self) -> LocalModelManagementFuture<'_, Result<(), LocalModelManagerError>> {
        Box::pin(LocalModelManagerService::shutdown(self))
    }
}

impl fmt::Debug for LocalModelManagerService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelManagerService")
            .field(
                "accepting_work",
                &(self.inner.lifecycle.load(Ordering::Acquire) == LIFECYCLE_RUNNING),
            )
            .finish_non_exhaustive()
    }
}

fn validate_config(config: &LocalModelManagerConfig) -> Result<(), LocalModelManagerError> {
    if config.context_window_tokens == 0 {
        return Err(LocalModelManagerError::InvalidConfiguration(
            "context_window_tokens must be greater than zero",
        ));
    }
    if config.timeout_seconds == 0 {
        return Err(LocalModelManagerError::InvalidConfiguration(
            "timeout_seconds must be greater than zero",
        ));
    }
    if config.startup_timeout_seconds == 0 {
        return Err(LocalModelManagerError::InvalidConfiguration(
            "startup_timeout_seconds must be greater than zero",
        ));
    }
    Ok(())
}
