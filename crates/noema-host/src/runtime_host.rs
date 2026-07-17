//! Contract-shaped application host shared by client shells.

use std::sync::Arc;

use noema_capabilities_mcp::McpControlPlaneHandle;
#[cfg(feature = "composition")]
use noema_home::{SystemErrorEvent, SystemErrorLogger};
use noema_memory::{MemoryRepositoryHandle, MemoryServiceAccessHandle};
use noema_providers::{LocalModelManager, ProviderAccountOperationsHandle, ProviderRegistryHandle};
use noema_runtime::{RuntimeEventRegistry, RuntimeHandle};
use noema_store::NoemaStore;
use thiserror::Error;

#[cfg(feature = "composition")]
use crate::HostConfig;
use crate::{OnboardingService, WebConfig};

/// Narrow diagnostic contract supplied to API adapters.
pub trait ArtifactDiagnosticOperations: Send + Sync {
    /// Record a failed authorized artifact download without exposing roots or loggers.
    fn record_download_failure(&self, operation: &'static str);
}

/// Clonable artifact diagnostic handle.
pub type ArtifactDiagnosticHandle = Arc<dyn ArtifactDiagnosticOperations>;

/// Assembled service handles consumed by transport-neutral API adapters.
#[derive(Clone)]
pub struct HostServices {
    runtime: RuntimeHandle,
    store: NoemaStore,
    artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    artifact_diagnostics: ArtifactDiagnosticHandle,
    provider_account_operations: ProviderAccountOperationsHandle,
    mcp_operations: McpControlPlaneHandle,
    local_model_manager: LocalModelManager,
    provider_registry: ProviderRegistryHandle,
    memory_repository: MemoryRepositoryHandle,
    memory_service_access: MemoryServiceAccessHandle,
    onboarding: OnboardingService,
    memory_startup_error: Option<String>,
    runtime_events: RuntimeEventRegistry,
}

impl HostServices {
    #[cfg(feature = "composition")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        runtime: RuntimeHandle,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        artifact_diagnostics: ArtifactDiagnosticHandle,
        provider_account_operations: ProviderAccountOperationsHandle,
        mcp_operations: McpControlPlaneHandle,
        local_model_manager: LocalModelManager,
        provider_registry: ProviderRegistryHandle,
        memory_repository: MemoryRepositoryHandle,
        memory_service_access: MemoryServiceAccessHandle,
        onboarding: OnboardingService,
        memory_startup_error: Option<String>,
        runtime_events: RuntimeEventRegistry,
    ) -> Self {
        Self {
            runtime,
            store,
            artifact_operations,
            artifact_diagnostics,
            provider_account_operations,
            mcp_operations,
            local_model_manager,
            provider_registry,
            memory_repository,
            memory_service_access,
            onboarding,
            memory_startup_error,
            runtime_events,
        }
    }

    /// Governed runtime command handle.
    #[must_use]
    pub const fn runtime(&self) -> &RuntimeHandle {
        &self.runtime
    }

    /// Canonical structured store.
    #[must_use]
    pub const fn store(&self) -> &NoemaStore {
        &self.store
    }

    /// Governed artifact operations.
    #[must_use]
    pub const fn artifact_operations(&self) -> &noema_artifacts::ArtifactOperationsHandle {
        &self.artifact_operations
    }

    /// Narrow artifact diagnostics.
    #[must_use]
    pub const fn artifact_diagnostics(&self) -> &ArtifactDiagnosticHandle {
        &self.artifact_diagnostics
    }

    /// Provider account and authentication operations.
    #[must_use]
    pub const fn provider_account_operations(&self) -> &ProviderAccountOperationsHandle {
        &self.provider_account_operations
    }

    /// MCP control-plane operations.
    #[must_use]
    pub const fn mcp_operations(&self) -> &McpControlPlaneHandle {
        &self.mcp_operations
    }

    /// Local-model management operations.
    #[must_use]
    pub const fn local_model_manager(&self) -> &LocalModelManager {
        &self.local_model_manager
    }

    /// Exact provider-instance registry.
    #[must_use]
    pub const fn provider_registry(&self) -> &ProviderRegistryHandle {
        &self.provider_registry
    }

    /// Memory settings and article repository.
    #[must_use]
    pub const fn memory_repository(&self) -> &MemoryRepositoryHandle {
        &self.memory_repository
    }

    /// Request-scoped memory service access.
    #[must_use]
    pub const fn memory_service_access(&self) -> &MemoryServiceAccessHandle {
        &self.memory_service_access
    }

    /// Cross-subsystem onboarding operations.
    #[must_use]
    pub const fn onboarding(&self) -> &OnboardingService {
        &self.onboarding
    }

    /// Managed memory startup detail, when startup degraded.
    #[must_use]
    pub fn memory_startup_error(&self) -> Option<&str> {
        self.memory_startup_error.as_deref()
    }

    /// Transport-neutral runtime event registry.
    #[must_use]
    pub const fn runtime_events(&self) -> &RuntimeEventRegistry {
        &self.runtime_events
    }
}

/// One fully assembled Noema application host.
pub struct NoemaHost {
    services: HostServices,
    web_config: WebConfig,
    #[cfg(feature = "composition")]
    lifecycle: crate::composition::HostLifecycle,
}

impl NoemaHost {
    /// Return the service handle bundle used to construct API state.
    #[must_use]
    pub const fn services(&self) -> &HostServices {
        &self.services
    }

    /// Return the resolved local web listener configuration.
    #[must_use]
    pub const fn web_config(&self) -> &WebConfig {
        &self.web_config
    }

    #[cfg(feature = "composition")]
    pub(crate) fn assembled(
        services: HostServices,
        web_config: WebConfig,
        lifecycle: crate::composition::HostLifecycle,
    ) -> Self {
        Self {
            services,
            web_config,
            lifecycle,
        }
    }

    /// Shut down admission, runtime work, and concrete child services in dependency order.
    #[cfg(feature = "composition")]
    pub async fn shutdown(self) {
        self.lifecycle.shutdown().await;
    }
}

/// Start an application host from already resolved host configuration.
///
/// # Errors
///
/// Returns [`RuntimeHostError`] when home, store, provider, or runtime startup fails.
#[cfg(feature = "composition")]
pub async fn start_from_loaded_config(config: HostConfig) -> Result<NoemaHost, RuntimeHostError> {
    crate::composition::start_from_loaded_config(config).await
}

/// Initialize the Noema home, load process configuration, and start one host.
///
/// # Errors
///
/// Returns [`RuntimeHostError`] when configuration or application startup fails.
#[cfg(feature = "composition")]
pub async fn start_from_process_env() -> Result<NoemaHost, RuntimeHostError> {
    start_from_process_env_with_local_model_runtime_root(None).await
}

/// Initialize process configuration and start one host with an optional
/// shell-packaged llama.cpp resource root.
///
/// # Errors
///
/// Returns [`RuntimeHostError`] when configuration or application startup fails.
#[cfg(feature = "composition")]
pub async fn start_from_process_env_with_local_model_runtime_root(
    local_model_runtime_root: Option<std::path::PathBuf>,
) -> Result<NoemaHost, RuntimeHostError> {
    crate::composition::start_from_process_env_with_local_model_runtime_root(
        local_model_runtime_root,
    )
    .await
}

#[cfg(feature = "composition")]
pub(crate) struct SystemErrorArtifactDiagnostics {
    logger: SystemErrorLogger,
}

#[cfg(feature = "composition")]
impl SystemErrorArtifactDiagnostics {
    pub(crate) fn new(logger: SystemErrorLogger) -> Self {
        Self { logger }
    }
}

#[cfg(feature = "composition")]
impl ArtifactDiagnosticOperations for SystemErrorArtifactDiagnostics {
    fn record_download_failure(&self, operation: &'static str) {
        self.logger.try_append(
            SystemErrorEvent::new(
                "artifact_download_failed",
                "An artifact download could not be completed",
            )
            .with_context(serde_json::json!({ "operation": operation })),
        );
    }
}

/// Runtime host startup error.
#[derive(Debug, Error)]
pub enum RuntimeHostError {
    /// Configuration loading or validation failed.
    #[cfg(feature = "composition")]
    #[error("configuration failed: {0}")]
    Config(#[from] crate::ConfigError),
    /// Noema home path resolution failed.
    #[error("data folder path resolution failed: {0}")]
    Path(#[from] noema_home::NoemaPathError),
    /// Noema home initialization failed.
    #[error("data folder initialization failed: {0}")]
    Home(#[from] noema_home::NoemaHomeError),
    /// Store setup failed.
    #[error("store setup failed: {0}")]
    Store(#[source] noema_store::StoreError),
    /// A configured default could not be resolved without changing user intent.
    #[error("configured default provider failed: {0}")]
    ConfiguredDefault(#[source] noema_store::StoreError),
    /// Provider adapter construction failed.
    #[error("provider setup failed: {0}")]
    Provider(#[from] noema_providers::ProviderError),
    /// Provider selection identity was invalid.
    #[error("provider selection setup failed: {0}")]
    ProviderSelection(#[from] noema_providers::ProviderSelectionError),
    /// Provider registry setup failed.
    #[error("provider registry setup failed: {0}")]
    Registry(#[from] noema_providers::ProviderRegistryError),
    /// Local-model lifecycle setup failed.
    #[error("local-model setup failed: {0}")]
    LocalModel(#[from] noema_providers::LocalModelManagerError),
    /// Artifact service setup failed.
    #[error("artifact service setup failed: {0}")]
    Artifact(#[from] noema_artifacts::ArtifactOperationError),
    /// MCP service setup failed.
    #[cfg(feature = "composition")]
    #[error("MCP service setup failed: {0}")]
    Mcp(#[from] noema_capabilities_mcp::LocalMcpServiceConstructionError),
    /// Governed execution runtime setup failed.
    #[error("runtime setup failed: {0}")]
    Runtime(#[from] noema_runtime::RuntimeError),
    /// Host composition invariant failed.
    #[error("host composition failed: {0}")]
    Composition(String),
}

impl RuntimeHostError {
    /// Plain-language user-facing message.
    #[must_use]
    pub fn user_message(&self) -> &'static str {
        match self {
            #[cfg(feature = "composition")]
            Self::Config(_) => "Noema could not load its configuration.",
            Self::ConfiguredDefault(_) | Self::ProviderSelection(_) => {
                "Noema could not load its configuration."
            }
            Self::Path(_) | Self::Home(_) => "Noema could not open its data folder.",
            Self::Store(_) => "Noema could not start its local memory store.",
            Self::Provider(_)
            | Self::Registry(_)
            | Self::LocalModel(_)
            | Self::Artifact(_)
            | Self::Runtime(_)
            | Self::Composition(_) => "Noema could not start the local assistant service.",
            #[cfg(feature = "composition")]
            Self::Mcp(_) => "Noema could not start the local assistant service.",
        }
    }

    /// Detailed diagnostic string.
    #[must_use]
    pub fn technical_details(&self) -> String {
        self.to_string()
    }
}

impl From<noema_store::StoreError> for RuntimeHostError {
    fn from(source: noema_store::StoreError) -> Self {
        if matches!(
            &source,
            noema_store::StoreError::ConfiguredDefaultUnresolvable { .. }
        ) {
            Self::ConfiguredDefault(source)
        } else {
            Self::Store(source)
        }
    }
}
