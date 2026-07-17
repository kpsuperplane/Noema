//! Runtime-independent local-model management contract.

use std::{fmt, future::Future, path::PathBuf, pin::Pin, sync::Arc};

use noema_home::SystemErrorLogger;
use thiserror::Error;

use super::{
    LocalModelBackend, LocalModelCatalogSnapshot, LocalModelEventRecord,
    LocalModelInstallationRecord, RemovedLocalModelInstallation,
};
use crate::{
    ProviderInstanceKey, ProviderPersistenceError, ProviderRegistryError, ProviderRegistryHandle,
};

/// Input for importing a public GGUF from an immutable Hugging Face revision.
#[derive(Clone, Debug, PartialEq)]
pub struct HuggingFaceLocalModelImport {
    /// Product-facing model name.
    pub name: String,
    /// Provider-facing model profile.
    pub model_id: String,
    /// Public repository in `owner/repository` form.
    pub repo: String,
    /// Immutable 40-character commit.
    pub revision: String,
    /// GGUF path inside the repository.
    pub file: String,
    /// Expected lowercase SHA-256 digest.
    pub sha256: String,
    /// Optional user-supplied license label.
    pub license: Option<String>,
    /// Backend to use for this advanced import.
    pub backend: LocalModelBackend,
}

/// Input for importing an existing GGUF from the local filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalFileModelImport {
    /// Product-facing model name.
    pub name: String,
    /// Provider-facing model profile.
    pub model_id: String,
    /// Existing local GGUF path.
    pub path: PathBuf,
    /// Optional user-supplied license label.
    pub license: Option<String>,
    /// Backend to use for this advanced import.
    pub backend: LocalModelBackend,
}

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

/// Observable state of the local inference runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalModelRuntimeStatus {
    /// No server process is running.
    Stopped,
    /// One backend candidate is loading the installed model.
    Starting {
        /// Candidate currently being started.
        backend: LocalModelBackend,
    },
    /// A backend candidate is healthy and ready for generation.
    Ready {
        /// Active backend.
        backend: LocalModelBackend,
        /// Loopback API base URL.
        endpoint: String,
        /// Installed model id loaded by the process.
        model_id: String,
    },
    /// A candidate failed and the supervisor is trying the next backend.
    Retrying {
        /// Candidate that failed.
        failed_backend: LocalModelBackend,
        /// Next candidate to try.
        next_backend: LocalModelBackend,
        /// Concise launch or health failure.
        message: String,
    },
    /// Every configured backend candidate failed.
    Failed {
        /// Combined candidate failures.
        message: String,
    },
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

/// One structurally valid instance that could not be started during reconstruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DegradedLocalModelInstance {
    /// Immutable provider instance identity.
    pub key: ProviderInstanceKey,
    /// Concrete installation identity.
    pub installation_id: String,
    /// Stable non-secret runtime failure summary.
    pub message: String,
}

/// Result of reconstructing every persistently referenced local instance.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalModelReconstructionReport {
    /// Exact instances that are ready and registered.
    pub ready: Vec<ProviderInstanceKey>,
    /// Structurally valid instances that remain temporarily unavailable.
    pub degraded: Vec<DegradedLocalModelInstance>,
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
pub type LocalModelManagerEventStream = Pin<
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
    /// A durable reference has no installation owner.
    #[error("referenced local-model instance has no installation: {provider_instance_key}")]
    ReferencedInstallationMissing {
        /// Missing exact instance identity.
        provider_instance_key: ProviderInstanceKey,
    },
    /// Persisted installation identity does not match its immutable provenance.
    #[error(
        "local-model installation `{installation_id}` has mismatched provider instance identity"
    )]
    InstallationIdentityMismatch {
        /// Installation with malformed identity.
        installation_id: String,
    },
    /// A durable future reference points at an already claimed installation.
    #[error("referenced local-model instance is claimed for retirement: {provider_instance_key}")]
    ReferencedInstallationClaimed {
        /// Claimed exact instance identity.
        provider_instance_key: ProviderInstanceKey,
    },
    /// A durable future reference points at a runtime-retired installation.
    #[error("referenced local-model instance is runtime-retired: {provider_instance_key}")]
    ReferencedInstallationRuntimeRetired {
        /// Retired exact instance identity.
        provider_instance_key: ProviderInstanceKey,
    },
    /// More than one installation owns the same immutable key.
    #[error("duplicate local-model provider instance identity: {provider_instance_key}")]
    DuplicateInstanceIdentity {
        /// Duplicated exact instance identity.
        provider_instance_key: ProviderInstanceKey,
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

/// Boxed asynchronous local-model management operation.
pub type LocalModelManagementFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Object-safe local-model management operations implemented by a host service or test fake.
pub trait LocalModelManagement: Send + Sync {
    /// Returns the shared exact-instance registry used by runtime consumers.
    fn registry(&self) -> ProviderRegistryHandle;
    /// Returns the active process status without waiting for a change.
    fn runtime_status(&self) -> LocalModelRuntimeStatus;
    /// Lists durable installations in repository presentation order.
    fn installations(
        &self,
    ) -> LocalModelManagementFuture<
        '_,
        Result<Vec<LocalModelInstallationRecord>, LocalModelManagerError>,
    >;
    /// Returns durable installation events after an optional exclusive cursor.
    fn events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> LocalModelManagementFuture<'_, Result<Vec<LocalModelEventRecord>, LocalModelManagerError>>;
    /// Returns status for every retained exact local-model process.
    fn managed_instances(&self) -> LocalModelManagementFuture<'_, Vec<ManagedLocalModelStatus>>;
    /// Returns the bundled catalog and machine compatibility projections.
    ///
    /// # Errors
    ///
    /// Returns an implementation-defined discovery error when the catalog cannot be projected.
    fn catalog_snapshot(&self) -> Result<LocalModelCatalogSnapshot, LocalModelManagerError>;
    /// Returns the platform-preferred backend for an imported GGUF.
    ///
    /// # Errors
    ///
    /// Returns an implementation-defined discovery error when hardware cannot be inspected.
    fn preferred_import_backend(&self) -> Result<LocalModelBackend, LocalModelManagerError>;
    /// Queues and activates one compatible catalog artifact.
    fn install_catalog_model(
        &self,
        model_id: String,
        file: Option<String>,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>;
    /// Queues one pinned public Hugging Face GGUF import.
    fn import_hugging_face(
        &self,
        input: HuggingFaceLocalModelImport,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>;
    /// Queues one existing local GGUF import.
    fn import_local_file(
        &self,
        input: LocalFileModelImport,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>;
    /// Cancels an installation worker and durably marks it cancelled.
    fn cancel_installation(
        &self,
        installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>;
    /// Removes one inactive installation and its unreferenced artifact.
    fn remove(
        &self,
        installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<RemovedLocalModelInstallation, LocalModelManagerError>>;
    /// Activates one verified installation as the system default.
    fn activate(
        &self,
        installation_id: String,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelInstallationRecord, LocalModelManagerError>>;
    /// Retries the active installation after a transient runtime failure.
    fn retry_active_installation(
        &self,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelRuntimeStatus, LocalModelManagerError>>;
    /// Reconstructs exact local-model instances referenced by durable future work.
    fn reconstruct_persisted_instances(
        &self,
    ) -> LocalModelManagementFuture<
        '_,
        Result<LocalModelReconstructionReport, LocalModelManagerError>,
    >;
    /// Subscribes to merged durable and active-runtime events.
    fn subscribe_events(
        &self,
        after: Option<String>,
    ) -> LocalModelManagementFuture<'_, Result<LocalModelManagerEventStream, LocalModelManagerError>>;
    /// Rejects new work and drains manager-owned installation workers.
    fn begin_shutdown(&self) -> LocalModelManagementFuture<'_, ()>;
    /// Drains exact leases and stops every retained process.
    fn shutdown(&self) -> LocalModelManagementFuture<'_, Result<(), LocalModelManagerError>>;
}

/// Opaque, clonable local-model control-plane handle.
#[derive(Clone)]
pub struct LocalModelManager {
    operations: Arc<dyn LocalModelManagement>,
}

impl LocalModelManager {
    /// Creates a handle over an externally supplied management implementation.
    #[must_use]
    pub fn from_operations(operations: Arc<dyn LocalModelManagement>) -> Self {
        Self { operations }
    }

    /// Returns the shared exact-instance registry used by runtime consumers.
    #[must_use]
    pub fn registry(&self) -> ProviderRegistryHandle {
        self.operations.registry()
    }

    /// Returns the active process status without waiting for a change.
    #[must_use]
    pub fn runtime_status(&self) -> LocalModelRuntimeStatus {
        self.operations.runtime_status()
    }

    /// Lists durable installations in repository presentation order.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn installations(
        &self,
    ) -> Result<Vec<LocalModelInstallationRecord>, LocalModelManagerError> {
        self.operations.installations().await
    }

    /// Returns durable installation events after an optional exclusive cursor.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> Result<Vec<LocalModelEventRecord>, LocalModelManagerError> {
        self.operations.events(after_cursor, limit).await
    }

    /// Returns status for every retained exact local-model process.
    pub async fn managed_instances(&self) -> Vec<ManagedLocalModelStatus> {
        self.operations.managed_instances().await
    }

    /// Returns the bundled catalog and machine compatibility projections.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub fn catalog_snapshot(&self) -> Result<LocalModelCatalogSnapshot, LocalModelManagerError> {
        self.operations.catalog_snapshot()
    }

    /// Returns the platform-preferred backend for an imported GGUF.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub fn preferred_import_backend(&self) -> Result<LocalModelBackend, LocalModelManagerError> {
        self.operations.preferred_import_backend()
    }

    /// Queues and activates one compatible catalog artifact.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn install_catalog_model(
        &self,
        model_id: &str,
        file: Option<&str>,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.operations
            .install_catalog_model(model_id.to_string(), file.map(str::to_string))
            .await
    }

    /// Queues one pinned public Hugging Face GGUF import.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn import_hugging_face(
        &self,
        input: HuggingFaceLocalModelImport,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.operations.import_hugging_face(input).await
    }

    /// Queues one existing local GGUF import.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn import_local_file(
        &self,
        input: LocalFileModelImport,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.operations.import_local_file(input).await
    }

    /// Cancels an installation worker and durably marks it cancelled.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn cancel_installation(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.operations
            .cancel_installation(installation_id.to_string())
            .await
    }

    /// Removes one inactive installation and its unreferenced artifact.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn remove(
        &self,
        installation_id: &str,
    ) -> Result<RemovedLocalModelInstallation, LocalModelManagerError> {
        self.operations.remove(installation_id.to_string()).await
    }

    /// Activates one verified installation as the system default.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn activate(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelManagerError> {
        self.operations.activate(installation_id.to_string()).await
    }

    /// Retries the active installation after a transient runtime failure.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn retry_active_installation(
        &self,
    ) -> Result<LocalModelRuntimeStatus, LocalModelManagerError> {
        self.operations.retry_active_installation().await
    }

    /// Reconstructs exact local-model instances referenced by durable future work.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn reconstruct_persisted_instances(
        &self,
    ) -> Result<LocalModelReconstructionReport, LocalModelManagerError> {
        self.operations.reconstruct_persisted_instances().await
    }

    /// Subscribes to merged durable and active-runtime events.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn subscribe_events(
        &self,
        after: Option<&str>,
    ) -> Result<LocalModelManagerEventStream, LocalModelManagerError> {
        self.operations
            .subscribe_events(after.map(str::to_string))
            .await
    }

    /// Rejects new work and drains manager-owned installation workers.
    pub async fn begin_shutdown(&self) {
        self.operations.begin_shutdown().await;
    }

    /// Drains exact leases and stops every retained process.
    ///
    /// # Errors
    ///
    /// Returns errors reported by the configured management implementation.
    pub async fn shutdown(&self) -> Result<(), LocalModelManagerError> {
        self.operations.shutdown().await
    }
}

impl fmt::Debug for LocalModelManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelManager")
            .finish_non_exhaustive()
    }
}
