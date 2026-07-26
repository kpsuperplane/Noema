mod action_gateway;
mod action_resolution;
mod action_reviewer;
pub(in crate::daemon) mod actor;
mod background_task;
pub(crate) mod context_compaction;
mod context_window;
mod continuation_context;
mod conversation_state;
pub(in crate::daemon) mod handle;
mod local_tool_results;
pub(in crate::daemon) mod local_tools;
mod mcp_auth_resolution;
pub(crate) mod model_context;
mod model_context_ledger;
pub(crate) mod model_tools;
mod progress;
pub(crate) mod progress_audit;
mod prompt_context;
mod runtime_debug;
mod task_continuation;
mod task_transcript;
mod tasks;
mod tool_lifecycle;
pub(in crate::daemon) mod transcript_persistence;
pub(in crate::daemon) mod turn;
pub(crate) mod turn_timing;
mod web_tools;
mod work_notification;

pub(crate) use background_task::BackgroundTaskGenerateRequest;
pub use handle::RuntimeHandle;

/// Host-provided dependencies required to start the governed runtime.
pub struct RuntimeSpawnConfig {
    /// Route resolver for the primary conversation agent.
    pub primary_provider: noema_providers::ProviderRouteResolverHandle,
    /// Route resolver for default background generation.
    pub default_provider: noema_providers::ProviderRouteResolverHandle,
    /// Route resolver for task progress audits.
    pub progress_audit_provider: noema_providers::ProviderRouteResolverHandle,
    /// Route resolver for governed-action review. It has no implicit fallback.
    pub action_reviewer_provider: noema_providers::ProviderRouteResolverHandle,
    /// Route resolver for web-page summarization.
    pub web_summary_provider: noema_providers::ProviderRouteResolverHandle,
    /// Exact provider instance registry used by task routing.
    pub provider_registry: noema_providers::ProviderRegistryHandle,
    /// Durable runtime store.
    pub store: noema_store::NoemaStore,
    /// Governed artifact operations.
    pub artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    /// Redacted system diagnostic sink.
    pub system_errors: noema_home::SystemErrorLogger,
    /// Native Markdown memory tree shared by runtime and API.
    pub native_memory: Option<noema_memory::NativeMemory>,
    /// Runtime event registry shared with API adapters.
    pub runtime_events: super::RuntimeEventRegistry,
    /// Host-owned concrete web backend resolver.
    pub web_backends: crate::WebBackendResolverHandle,
    /// Capability catalog source.
    pub capability_bindings: noema_capabilities::CapabilityBindingSourceHandle,
    /// Capability invokers available to the runtime.
    pub capability_invokers: std::sync::Arc<[noema_capabilities::CapabilityInvokerRegistration]>,
}
