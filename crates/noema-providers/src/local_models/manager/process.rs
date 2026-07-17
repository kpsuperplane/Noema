//! Injectable managed-process construction.

use std::{future::Future, pin::Pin, sync::Arc};

use tokio::sync::watch;

use noema_home::NoemaPaths;

use crate::{
    LocalModelInstallationRecord, LocalModelInstallationStatus, LocalModelsProviderConfig,
    ProviderHandle, erase_model_provider,
};

use super::{LocalModelManagerConfig, LocalModelManagerError};
use crate::{LocalModelRuntimeStatus, local_models::LocalModelsProvider};

pub(super) type LocalModelProcessFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, LocalModelManagerError>> + Send + 'a>>;

pub(super) trait LocalModelProcessFactory: Send + Sync {
    fn start(
        &self,
        installation: LocalModelInstallationRecord,
    ) -> LocalModelProcessFuture<'_, Arc<dyn LocalModelProcess>>;
}

pub(super) trait LocalModelProcess: Send + Sync {
    fn provider(&self) -> ProviderHandle;

    fn status(&self) -> LocalModelRuntimeStatus;

    fn subscribe_status(&self) -> watch::Receiver<LocalModelRuntimeStatus>;

    fn shutdown(&self) -> LocalModelProcessFuture<'_, ()>;
}

pub(super) struct DefaultLocalModelProcessFactory {
    paths: NoemaPaths,
    config: LocalModelManagerConfig,
}

impl DefaultLocalModelProcessFactory {
    pub(super) fn new(paths: NoemaPaths, config: LocalModelManagerConfig) -> Self {
        Self { paths, config }
    }
}

impl LocalModelProcessFactory for DefaultLocalModelProcessFactory {
    fn start(
        &self,
        installation: LocalModelInstallationRecord,
    ) -> LocalModelProcessFuture<'_, Arc<dyn LocalModelProcess>> {
        Box::pin(async move {
            if installation.status != LocalModelInstallationStatus::Installed {
                return Err(LocalModelManagerError::InstallationNotReady {
                    installation_id: installation.installation_id,
                    reason: "installation status is not installed",
                });
            }
            let sha256 = installation.sha256.as_deref().ok_or_else(|| {
                LocalModelManagerError::InstallationNotReady {
                    installation_id: installation.installation_id.clone(),
                    reason: "verified digest is missing",
                }
            })?;
            let model_path = self.paths.local_model_blob_path(sha256).map_err(|_| {
                LocalModelManagerError::InstallationNotReady {
                    installation_id: installation.installation_id.clone(),
                    reason: "verified digest is invalid",
                }
            })?;
            if !tokio::fs::try_exists(&model_path).await.map_err(|_| {
                LocalModelManagerError::Runtime {
                    operation: "inspect_model_blob",
                    message: "installed model blob is unavailable".to_string(),
                }
            })? {
                return Err(LocalModelManagerError::Runtime {
                    operation: "inspect_model_blob",
                    message: "installed model blob is unavailable".to_string(),
                });
            }
            let provider = LocalModelsProvider::new(LocalModelsProviderConfig {
                default_model: installation.model_id,
                model_path: Some(model_path),
                preferred_backend: Some(installation.backend),
                runtime_root: self.config.runtime_root.clone(),
                context_window_tokens: self.config.context_window_tokens,
                timeout_seconds: self.config.timeout_seconds,
                startup_timeout_seconds: self.config.startup_timeout_seconds,
                system_errors: self.config.system_errors.clone(),
            })
            .map_err(|error| LocalModelManagerError::Runtime {
                operation: "construct_local_provider",
                message: error.to_string(),
            })?;
            let runtime = provider.runtime().clone();
            runtime
                .ensure_ready()
                .await
                .map_err(|error| LocalModelManagerError::Runtime {
                    operation: "start_local_process",
                    message: error.to_string(),
                })?;
            Ok(Arc::new(LlamaManagedProcess {
                provider: erase_model_provider(provider),
                runtime,
            }) as Arc<dyn LocalModelProcess>)
        })
    }
}

struct LlamaManagedProcess {
    provider: ProviderHandle,
    runtime: crate::local_models::LlamaServerSupervisor,
}

impl LocalModelProcess for LlamaManagedProcess {
    fn provider(&self) -> ProviderHandle {
        Arc::clone(&self.provider)
    }

    fn status(&self) -> LocalModelRuntimeStatus {
        self.runtime.status()
    }

    fn subscribe_status(&self) -> watch::Receiver<LocalModelRuntimeStatus> {
        self.runtime.subscribe_status()
    }

    fn shutdown(&self) -> LocalModelProcessFuture<'_, ()> {
        Box::pin(async move {
            self.runtime.shutdown().await;
            Ok(())
        })
    }
}
