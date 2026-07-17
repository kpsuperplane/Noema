//! Injectable managed-process construction.

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use noema_home::NoemaPaths;

use crate::{
    LocalModelInstallationRecord, LocalModelInstallationStatus, LocalModelsProviderConfig,
    ProviderHandle, erase_model_provider,
};

use super::{LocalModelManagerConfig, LocalModelManagerError};
use crate::{
    LocalModelRuntimeStatus,
    local_models::{
        LocalModelInstallError, LocalModelsProvider,
        download_support::{blob_relative_path, hash_file},
    },
};

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
            let model_path = verified_model_blob_path(&self.paths, &installation).await?;
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

async fn verified_model_blob_path(
    paths: &NoemaPaths,
    installation: &LocalModelInstallationRecord,
) -> Result<PathBuf, LocalModelManagerError> {
    if installation.status != LocalModelInstallationStatus::Installed {
        return Err(LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "installation status is not installed",
        });
    }
    let sha256 = installation.sha256.as_deref().ok_or_else(|| {
        LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "verified digest is missing",
        }
    })?;
    let model_path = paths.local_model_blob_path(sha256).map_err(|_| {
        LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "verified digest is invalid",
        }
    })?;
    let expected_relative_path = blob_relative_path(sha256);
    if installation.blob_relative_path.as_deref() != Some(expected_relative_path.as_str()) {
        return Err(LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "verified blob path does not match its digest",
        });
    }
    let actual_sha256 = hash_file(&model_path, &CancellationToken::new())
        .await
        .map_err(|error| LocalModelManagerError::Runtime {
            operation: "verify_model_blob",
            message: if matches!(
                error,
                LocalModelInstallError::Io(ref source)
                    if source.kind() == std::io::ErrorKind::NotFound
            ) {
                "installed model blob is unavailable"
            } else {
                "installed model blob could not be verified"
            }
            .to_string(),
        })?;
    if actual_sha256 != sha256 {
        return Err(LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "installed model blob digest does not match its verified digest",
        });
    }
    Ok(model_path)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use ring::digest::{SHA256, digest};

    use super::*;
    use crate::{
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelBackend, LocalModelSourceKind,
        local_model_provider_instance_key, local_models::download_support::hex_digest,
    };

    const MODEL_BYTES: &[u8] = b"verified model bytes";

    #[tokio::test]
    async fn verified_model_blob_path_accepts_canonical_untampered_blob() {
        let temp = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
        let installation = installed_record(MODEL_BYTES);
        let blob = paths
            .local_model_blob_path(installation.sha256.as_deref().expect("digest"))
            .expect("blob path");
        fs::create_dir_all(blob.parent().expect("blob parent")).expect("create blob parent");
        fs::write(&blob, MODEL_BYTES).expect("write blob");

        let verified = verified_model_blob_path(&paths, &installation)
            .await
            .expect("verify blob");

        assert_eq!(verified, blob);
    }

    #[tokio::test]
    async fn verified_model_blob_path_rejects_tampered_blob() {
        let temp = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
        let installation = installed_record(MODEL_BYTES);
        let blob = paths
            .local_model_blob_path(installation.sha256.as_deref().expect("digest"))
            .expect("blob path");
        fs::create_dir_all(blob.parent().expect("blob parent")).expect("create blob parent");
        fs::write(&blob, b"tampered model bytes").expect("write tampered blob");

        let error = verified_model_blob_path(&paths, &installation)
            .await
            .expect_err("reject tampered blob");

        assert!(matches!(
            error,
            LocalModelManagerError::InstallationNotReady {
                reason: "installed model blob digest does not match its verified digest",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn verified_model_blob_path_rejects_noncanonical_durable_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
        let mut installation = installed_record(MODEL_BYTES);
        installation.blob_relative_path = Some("models/blobs/other.gguf".to_string());

        let error = verified_model_blob_path(&paths, &installation)
            .await
            .expect_err("reject noncanonical path");

        assert!(matches!(
            error,
            LocalModelManagerError::InstallationNotReady {
                reason: "verified blob path does not match its digest",
                ..
            }
        ));
    }

    fn installed_record(bytes: &[u8]) -> LocalModelInstallationRecord {
        let sha256 = hex_digest(digest(&SHA256, bytes).as_ref());
        let installation_id = "installation:test";
        let model_id = "model:test";
        LocalModelInstallationRecord {
            installation_id: installation_id.to_string(),
            provider_instance_key: local_model_provider_instance_key(
                LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
                installation_id,
                model_id,
            )
            .expect("provider key"),
            model_id: model_id.to_string(),
            display_name: "Test model".to_string(),
            source_kind: LocalModelSourceKind::LocalFile,
            source_repo: None,
            source_revision: None,
            source_file: Some("test.gguf".to_string()),
            sha256: Some(sha256.clone()),
            download_gb: 0.0,
            expected_bytes: Some(bytes.len() as u64),
            downloaded_bytes: bytes.len() as u64,
            license: None,
            backend: LocalModelBackend::Cpu,
            status: LocalModelInstallationStatus::Installed,
            blob_relative_path: Some(blob_relative_path(&sha256)),
            is_active: false,
            runtime_retired_at: None,
            retirement_claimed_at: None,
            error_code: None,
            error_message: None,
            installed_at: Some("2026-01-01T00:00:00Z".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }
}
