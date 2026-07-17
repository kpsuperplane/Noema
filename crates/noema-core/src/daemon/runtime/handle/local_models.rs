use noema_home::{NoemaPaths, SystemErrorLogger};
use noema_providers::{
    DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS, DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
    DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS, LocalModelsProviderConfig, ProviderError, ProviderHandle,
    ProviderKind, erase_model_provider,
};

use super::CodexRuntimeHandle;
use noema_store::NoemaStore;

use crate::{LocalModelsProvider, daemon::protocol::DaemonError};

impl CodexRuntimeHandle {
    /// Set the packaged llama.cpp resource root used for dynamic registrations.
    pub(crate) async fn set_local_models_runtime_root(
        &self,
        runtime_root: Option<std::path::PathBuf>,
    ) {
        *self.local_models_runtime_root.write().await = runtime_root;
    }

    /// Attach the supervisor owned by an already-registered local provider.
    pub(crate) async fn attach_local_models_runtime(&self, runtime: crate::LlamaServerSupervisor) {
        self.local_models_runtime.write().await.replace(runtime);
    }

    /// Register or replace the active installed local model without restarting Noema.
    pub(crate) async fn register_installed_local_model(
        &self,
        installation: &noema_providers::LocalModelInstallationRecord,
        paths: &NoemaPaths,
    ) -> Result<(), DaemonError> {
        let (provider, runtime) = self
            .prepare_installed_local_model(installation, paths)
            .await?;
        runtime.retry().await.map_err(local_model_runtime_error)?;

        let publication = self.provider_routes.begin_publication().await;
        let mut runtime_slot = self.local_models_runtime.write().await;
        publication
            .register(ProviderKind::LocalModels.as_str(), provider)
            .map_err(DaemonError::from)?;
        runtime_slot.replace(runtime);
        Ok(())
    }

    /// Atomically activate and publish one installed local model.
    pub(crate) async fn activate_installed_local_model(
        &self,
        installation: &noema_providers::LocalModelInstallationRecord,
        paths: &NoemaPaths,
        store: &NoemaStore,
    ) -> Result<(), DaemonError> {
        let (provider, runtime) = self
            .prepare_installed_local_model(installation, paths)
            .await?;
        runtime.retry().await.map_err(local_model_runtime_error)?;

        let publication = self.provider_routes.begin_publication().await;
        let mut runtime_slot = self.local_models_runtime.write().await;
        store
            .activate_local_model_as_system_default(&installation.installation_id)
            .await?;
        publication
            .register(ProviderKind::LocalModels.as_str(), provider)
            .map_err(DaemonError::from)?;
        runtime_slot.replace(runtime);
        Ok(())
    }

    async fn prepare_installed_local_model(
        &self,
        installation: &noema_providers::LocalModelInstallationRecord,
        paths: &NoemaPaths,
    ) -> Result<(ProviderHandle, crate::LlamaServerSupervisor), DaemonError> {
        if installation.status != noema_providers::LocalModelInstallationStatus::Installed {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: ProviderKind::LocalModels.as_str().to_string(),
                message: format!("local model `{}` is not installed", installation.model_id),
            }));
        }
        let sha256 = installation.sha256.as_deref().ok_or_else(|| {
            DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: ProviderKind::LocalModels.as_str().to_string(),
                message: format!(
                    "installed local model `{}` has no verified digest",
                    installation.model_id
                ),
            })
        })?;
        let model_path = paths.local_model_blob_path(sha256)?;
        let runtime_root = self.local_models_runtime_root.read().await.clone();
        let provider = LocalModelsProvider::new(LocalModelsProviderConfig {
            default_model: installation.model_id.clone(),
            model_path: Some(model_path),
            preferred_backend: Some(installation.backend),
            runtime_root,
            context_window_tokens: DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
            system_errors: Some(SystemErrorLogger::from_paths(paths)),
        })?;
        let runtime = provider.runtime().clone();
        let provider = erase_model_provider(provider);
        Ok((provider, runtime))
    }

    /// Return the current local inference process status, when a model is registered.
    pub(crate) async fn local_model_runtime_status(
        &self,
    ) -> Option<crate::LocalModelRuntimeStatus> {
        self.local_models_runtime
            .read()
            .await
            .as_ref()
            .map(crate::LlamaServerSupervisor::status)
    }

    /// Subscribe to local inference process transitions, when a model is registered.
    pub(crate) async fn subscribe_local_model_runtime_status(
        &self,
    ) -> Option<tokio::sync::watch::Receiver<crate::LocalModelRuntimeStatus>> {
        self.local_models_runtime
            .read()
            .await
            .as_ref()
            .map(crate::LlamaServerSupervisor::subscribe_status)
    }
}

fn local_model_runtime_error(error: crate::LlamaServerError) -> DaemonError {
    DaemonError::Provider(ProviderError::ProviderUnavailable {
        provider: ProviderKind::LocalModels.as_str().to_string(),
        message: error.to_string(),
    })
}
