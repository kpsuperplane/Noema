//! Local-model installation, process, routing, and shutdown control plane.

use std::{
    collections::HashMap,
    fmt,
    path::PathBuf,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicU8, Ordering},
    },
};

use thiserror::Error;
use tokio::{
    sync::{Mutex, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use noema_home::{NoemaPaths, SystemErrorLogger};

use crate::{
    LocalModelActivationPersistenceHandle, LocalModelEventRecord,
    LocalModelInstallationPersistenceHandle, LocalModelInstallationRecord, ProviderInstanceKey,
    ProviderPersistenceError, ProviderRegistration, ProviderRegistry, ProviderRegistryError,
    ProviderRegistryHandle,
};

use super::{
    LocalHardwareProfile, LocalModelBuild, LocalModelCatalogEntry, LocalModelInstallError,
    LocalModelInstaller, LocalModelRouteHandle, LocalModelRuntimeStatus,
};
use process::{DefaultLocalModelProcessFactory, LocalModelProcess, LocalModelProcessFactory};

mod events;
mod lifecycle;
mod process;
mod workers;

#[cfg(test)]
mod tests;

const LIFECYCLE_RUNNING: u8 = 0;
const LIFECYCLE_SHUTTING_DOWN: u8 = 1;
const LIFECYCLE_STOPPED: u8 = 2;

/// Runtime construction settings shared by every managed local instance.
#[derive(Clone)]
pub struct LocalModelManagerConfig {
    /// Packaged llama.cpp resource root supplied by the desktop shell.
    pub runtime_root: Option<PathBuf>,
    /// Context window exposed to prompt planning.
    pub context_window_tokens: u32,
    /// Generation request timeout.
    pub timeout_seconds: u64,
    /// Time allowed for a process to load its model.
    pub startup_timeout_seconds: u64,
    /// Optional developer diagnostic logger for malformed model output.
    pub system_errors: Option<SystemErrorLogger>,
}

impl fmt::Debug for LocalModelManagerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelManagerConfig")
            .field(
                "runtime_root",
                &self.runtime_root.as_ref().map(|_| "[REDACTED PATH]"),
            )
            .field("context_window_tokens", &self.context_window_tokens)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("startup_timeout_seconds", &self.startup_timeout_seconds)
            .field("system_errors_configured", &self.system_errors.is_some())
            .finish()
    }
}

/// Runtime state for one exact managed installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedLocalModelStatus {
    /// Immutable provider instance identity.
    pub key: ProviderInstanceKey,
    /// Concrete installation identity.
    pub installation_id: String,
    /// Provider-facing model profile.
    pub model_id: String,
    /// Whether this exact instance is currently published for new work.
    pub is_active: bool,
    /// Current supervised process state.
    pub runtime: LocalModelRuntimeStatus,
}

/// Bundled catalog entries together with the machine profiles used for selection.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelCatalogSnapshot {
    /// Catalog projections in maintainer-defined order.
    pub entries: Vec<LocalModelCatalogSnapshotEntry>,
}

/// One catalog entry with manager-owned compatibility and recommendation results.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelCatalogSnapshotEntry {
    /// Validated bundled model and all downloadable builds.
    pub model: LocalModelCatalogEntry,
    /// Best compatible build for this machine, when one fits.
    pub selected_build: Option<LocalModelBuild>,
    /// Hardware values that selected the compatible build.
    pub compatible_hardware: Option<LocalHardwareProfile>,
    /// Explanation derived from the matched model, build, and hardware.
    pub compatibility_explanation: Option<String>,
    /// Whether this model is the top compatible recommendation.
    pub is_recommended: bool,
}

/// One manager-owned local-model subscription payload.
#[derive(Clone, Debug, PartialEq)]
pub enum LocalModelManagerEvent {
    /// Durable installation/event state suitable for reconnect backfill.
    Durable {
        /// Cursor-bearing durable event.
        event: LocalModelEventRecord,
        /// Current installation projection, absent after removal.
        installation: Option<Box<LocalModelInstallationRecord>>,
    },
    /// Ephemeral state change for the currently published process.
    RuntimeChanged {
        /// New active-process status.
        status: LocalModelRuntimeStatus,
    },
}

/// Cursor-bearing manager event ready for transport adaptation.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelManagerEventRecord {
    /// Reconnect cursor. Runtime events retain the durable cursor prefix.
    pub cursor: String,
    /// Provider-owned event payload.
    pub payload: LocalModelManagerEvent,
}

/// Boxed manager event stream.
pub type LocalModelManagerEventStream = std::pin::Pin<
    Box<
        dyn futures_util::Stream<
                Item = Result<LocalModelManagerEventRecord, LocalModelManagerError>,
            > + Send,
    >,
>;

/// Local-model manager failures.
#[derive(Debug, Error)]
pub enum LocalModelManagerError {
    /// Installation or artifact management failed.
    #[error("local-model installation operation failed: {message}")]
    Installation {
        /// Concise installer failure without exposing the internal installer API.
        message: String,
    },
    /// Durable provider state could not be read or committed.
    #[error("local-model persistence operation failed: {0}")]
    Persistence(#[from] ProviderPersistenceError),
    /// Exact provider registration or retirement failed.
    #[error("local-model provider registry operation failed: {0}")]
    Registry(#[from] ProviderRegistryError),
    /// The requested installation cannot back a process.
    #[error("local-model installation `{installation_id}` is not runtime-ready: {reason}")]
    InstallationNotReady {
        /// Installation that failed validation.
        installation_id: String,
        /// Stable, non-secret validation detail.
        reason: &'static str,
    },
    /// A process could not be started, health-checked, or stopped.
    #[error("local-model runtime operation `{operation}` failed: {message}")]
    Runtime {
        /// Stable operation identifier.
        operation: &'static str,
        /// Concise process failure.
        message: String,
    },
    /// Active installations cannot be removed.
    #[error("active local-model installation cannot be removed: {installation_id}")]
    ActiveInstallation {
        /// Active installation identity.
        installation_id: String,
    },
    /// New work was requested after shutdown began.
    #[error("local-model manager is shutting down")]
    ShuttingDown,
    /// Runtime construction settings are invalid.
    #[error("invalid local-model manager configuration: {0}")]
    InvalidConfiguration(&'static str),
    /// Bundled catalog or local hardware discovery failed.
    #[error("local-model discovery operation `{operation}` failed: {message}")]
    Discovery {
        /// Stable discovery operation.
        operation: &'static str,
        /// Concise failure detail.
        message: String,
    },
    /// A reconnect cursor did not contain a valid durable cursor prefix.
    #[error("invalid local-model event cursor")]
    InvalidEventCursor,
}

impl From<LocalModelInstallError> for LocalModelManagerError {
    fn from(error: LocalModelInstallError) -> Self {
        Self::Installation {
            message: error.to_string(),
        }
    }
}

/// Clonable local-model control-plane handle.
#[derive(Clone)]
pub struct LocalModelManager {
    inner: Arc<ManagerInner>,
}

struct ManagerInner {
    installer: LocalModelInstaller,
    installations: LocalModelInstallationPersistenceHandle,
    activation: LocalModelActivationPersistenceHandle,
    registry: ProviderRegistryHandle,
    routes: LocalModelRouteHandle,
    process_factory: Arc<dyn LocalModelProcessFactory>,
    system_errors: Option<SystemErrorLogger>,
    control: Mutex<()>,
    instances: StdMutex<HashMap<ProviderInstanceKey, ManagedInstance>>,
    workers: Mutex<HashMap<String, InstallationWorker>>,
    active_status_forwarder: Mutex<Option<ActiveStatusForwarder>>,
    runtime_status_tx: watch::Sender<LocalModelRuntimeStatus>,
    lifecycle_tx: watch::Sender<u8>,
    lifecycle: AtomicU8,
}

#[derive(Clone)]
struct ManagedInstance {
    installation: LocalModelInstallationRecord,
    registration: ProviderRegistration,
    process: Arc<dyn LocalModelProcess>,
}

struct InstallationWorker {
    cancellation: CancellationToken,
    completion: watch::Receiver<bool>,
}

struct ActiveStatusForwarder {
    cancellation: CancellationToken,
    task: JoinHandle<()>,
}

impl LocalModelManager {
    /// Creates a manager without starting the persisted active process.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError`] when configuration is invalid or the
    /// installer HTTP client cannot be built.
    pub fn new(
        installations: LocalModelInstallationPersistenceHandle,
        activation: LocalModelActivationPersistenceHandle,
        paths: NoemaPaths,
        config: LocalModelManagerConfig,
    ) -> Result<Self, LocalModelManagerError> {
        validate_config(&config)?;
        let system_errors = config.system_errors.clone();
        let process_factory = Arc::new(DefaultLocalModelProcessFactory::new(paths.clone(), config));
        Self::new_with_factory(
            installations,
            activation,
            paths,
            process_factory,
            system_errors,
        )
    }

    fn new_with_factory(
        installations: LocalModelInstallationPersistenceHandle,
        activation: LocalModelActivationPersistenceHandle,
        paths: NoemaPaths,
        process_factory: Arc<dyn LocalModelProcessFactory>,
        system_errors: Option<SystemErrorLogger>,
    ) -> Result<Self, LocalModelManagerError> {
        let installer = LocalModelInstaller::new(Arc::clone(&installations), paths)?;
        let registry = Arc::new(ProviderRegistry::new());
        let routes = LocalModelRouteHandle::new(Arc::clone(&registry));
        let (runtime_status_tx, _) = watch::channel(LocalModelRuntimeStatus::Stopped);
        let (lifecycle_tx, _) = watch::channel(LIFECYCLE_RUNNING);
        Ok(Self {
            inner: Arc::new(ManagerInner {
                installer,
                installations,
                activation,
                registry,
                routes,
                process_factory,
                system_errors,
                control: Mutex::new(()),
                instances: StdMutex::new(HashMap::new()),
                workers: Mutex::new(HashMap::new()),
                active_status_forwarder: Mutex::new(None),
                runtime_status_tx,
                lifecycle_tx,
                lifecycle: AtomicU8::new(LIFECYCLE_RUNNING),
            }),
        })
    }

    /// Returns the exact-instance route handle shared with runtime consumers.
    #[must_use]
    pub fn route_handle(&self) -> LocalModelRouteHandle {
        self.inner.routes.clone()
    }

    /// Returns the active process status without waiting for a change.
    #[must_use]
    pub fn runtime_status(&self) -> LocalModelRuntimeStatus {
        self.inner.runtime_status_tx.borrow().clone()
    }

    /// Subscribes to active-process status changes.
    #[must_use]
    pub fn subscribe_runtime_status(&self) -> watch::Receiver<LocalModelRuntimeStatus> {
        self.inner.runtime_status_tx.subscribe()
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

    /// Returns status for every retained exact local-model process.
    #[must_use]
    pub async fn managed_instances(&self) -> Vec<ManagedLocalModelStatus> {
        let active_key = self.inner.routes.active_key().await;
        let instances = self.inner.instances.lock().expect("instances lock");
        let mut statuses = instances
            .iter()
            .map(|(key, instance)| ManagedLocalModelStatus {
                key: key.clone(),
                installation_id: instance.installation.installation_id.clone(),
                model_id: instance.installation.model_id.clone(),
                is_active: active_key.as_ref() == Some(key),
                runtime: instance.process.status(),
            })
            .collect::<Vec<_>>();
        statuses.sort_by(|left, right| left.installation_id.cmp(&right.installation_id));
        statuses
    }

    fn ensure_accepting_work(&self) -> Result<(), LocalModelManagerError> {
        if self.inner.lifecycle.load(Ordering::Acquire) == LIFECYCLE_RUNNING {
            Ok(())
        } else {
            Err(LocalModelManagerError::ShuttingDown)
        }
    }
}

impl fmt::Debug for LocalModelManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelManager")
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
