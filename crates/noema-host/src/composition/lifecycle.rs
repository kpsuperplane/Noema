//! Shared startup rollback and normal host shutdown.

use noema_capabilities_mcp::LocalMcpService;
use noema_home::{SystemErrorEvent, SystemErrorLogger};
use noema_memory::MnemosyneLifecycle;
use noema_providers::{LocalModelManager, ProviderAccountService};
use noema_runtime::{RuntimeHandle, TaskRuntimeHandle};

/// Resources that require asynchronous cleanup after startup.
pub(super) struct StartupResources {
    provider_accounts: Option<ProviderAccountService>,
    local_models: Option<LocalModelManager>,
    mnemosyne: Option<MnemosyneLifecycle>,
    mcp: Option<LocalMcpService>,
    runtime: Option<RuntimeHandle>,
    task_runtime: Option<TaskRuntimeHandle>,
    system_errors: SystemErrorLogger,
}

impl StartupResources {
    pub(super) fn new(system_errors: SystemErrorLogger) -> Self {
        Self {
            provider_accounts: None,
            local_models: None,
            mnemosyne: None,
            mcp: None,
            runtime: None,
            task_runtime: None,
            system_errors,
        }
    }

    pub(super) fn retain_provider_accounts(&mut self, service: ProviderAccountService) {
        self.provider_accounts = Some(service);
    }

    pub(super) fn retain_local_models(&mut self, manager: LocalModelManager) {
        self.local_models = Some(manager);
    }

    pub(super) fn retain_mnemosyne(&mut self, lifecycle: MnemosyneLifecycle) {
        self.mnemosyne = Some(lifecycle);
    }

    pub(super) fn retain_mcp(&mut self, service: LocalMcpService) {
        self.mcp = Some(service);
    }

    pub(super) fn retain_runtime(&mut self, runtime: RuntimeHandle) {
        self.runtime = Some(runtime);
    }

    pub(super) fn retain_task_runtime(&mut self, runtime: TaskRuntimeHandle) {
        self.task_runtime = Some(runtime);
    }

    pub(super) async fn shutdown(mut self) {
        if let Some(mcp) = &self.mcp {
            mcp.begin_shutdown();
        }
        if let Some(local_models) = &self.local_models {
            local_models.begin_shutdown().await;
        }
        if let Some(task_runtime) = self.task_runtime.take() {
            task_runtime.shutdown().await;
        }
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown().await;
        }
        if let Some(mnemosyne) = self.mnemosyne.take() {
            mnemosyne.shutdown().await;
        }
        if let Some(local_models) = self.local_models.take()
            && let Err(error) = local_models.shutdown().await
        {
            self.system_errors.try_append(
                SystemErrorEvent::new(
                    "local_model_shutdown_failed",
                    "A local model process did not stop cleanly",
                )
                .with_error_chain([error.to_string()]),
            );
        }
        if let Some(mcp) = self.mcp.take()
            && !mcp.shutdown().await
        {
            self.system_errors.try_append(SystemErrorEvent::new(
                "mcp_shutdown_drain_timeout",
                "MCP work did not terminate before the shutdown deadline",
            ));
        }
        if let Some(provider_accounts) = self.provider_accounts.take() {
            provider_accounts.shutdown().await;
        }
    }
}

/// Concrete lifecycle resources retained solely for ordered shutdown.
pub(crate) struct HostLifecycle {
    resources: StartupResources,
}

impl HostLifecycle {
    pub(super) fn new(resources: StartupResources) -> Self {
        Self { resources }
    }

    pub(crate) async fn shutdown(self) {
        self.resources.shutdown().await;
    }
}
