//! Narrow composition seams for downstream contract tests.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_DIAGNOSTIC_ID: AtomicU64 = AtomicU64::new(1);

/// Runtime dependencies supplied by downstream contract tests.
pub struct RuntimeContractTestSpawnConfig {
    /// Route resolver for the primary conversation agent.
    pub primary_provider: noema_providers::ProviderRouteResolverHandle,
    /// Route resolver for default background generation.
    pub default_provider: noema_providers::ProviderRouteResolverHandle,
    /// Route resolver for task progress audits.
    pub progress_audit_provider: noema_providers::ProviderRouteResolverHandle,
    /// Route resolver for web-page summarization.
    pub web_summary_provider: noema_providers::ProviderRouteResolverHandle,
    /// Exact provider instance registry used by task routing.
    pub provider_registry: noema_providers::ProviderRegistryHandle,
    /// Durable runtime store.
    pub store: noema_store::NoemaStore,
    /// Governed artifact operations.
    pub artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    /// Optional memory service operations.
    pub memory_operations: Option<noema_memory::MemoryOperationsHandle>,
    /// Runtime event registry shared with API adapters.
    pub runtime_events: crate::RuntimeEventRegistry,
    /// Host-owned concrete web backend resolver.
    pub web_backends: crate::WebBackendResolverHandle,
    /// Capability catalog source.
    pub capability_bindings: noema_capabilities::CapabilityBindingSourceHandle,
    /// Capability invokers available to the runtime.
    pub capability_invokers: Arc<[noema_capabilities::CapabilityInvokerRegistration]>,
}

/// Start a runtime with an isolated test-owned diagnostic sink.
///
/// # Errors
///
/// Returns the same startup errors as [`crate::RuntimeHandle::spawn`].
pub async fn spawn_runtime(
    config: RuntimeContractTestSpawnConfig,
) -> Result<crate::RuntimeHandle, crate::RuntimeError> {
    crate::RuntimeHandle::spawn(crate::RuntimeSpawnConfig {
        primary_provider: config.primary_provider,
        default_provider: config.default_provider,
        progress_audit_provider: config.progress_audit_provider,
        web_summary_provider: config.web_summary_provider,
        provider_registry: config.provider_registry,
        store: config.store,
        artifact_operations: config.artifact_operations,
        system_errors: isolated_system_error_logger(),
        memory_operations: config.memory_operations,
        runtime_events: config.runtime_events,
        web_backends: config.web_backends,
        capability_bindings: config.capability_bindings,
        capability_invokers: config.capability_invokers,
    })
    .await
}

fn isolated_system_error_logger() -> noema_home::SystemErrorLogger {
    let id = NEXT_DIAGNOSTIC_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(format!(
            "noema-runtime-contract-{}-{id}",
            std::process::id()
        ))
        .join("errors.log");
    noema_home::SystemErrorLogger::new(path)
}
