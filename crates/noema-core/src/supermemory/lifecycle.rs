//! Managed Supermemory child-process lifecycle.

use std::io::ErrorKind;

use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::{
    SupermemoryBinaryResolver, SupermemoryConnection, SupermemoryServerBinary,
    allocate_loopback_port,
};

/// Supermemory process lifecycle owned by the runtime host.
pub struct SupermemoryLifecycle {
    child: Option<tokio::process::Child>,
    connection: Option<SupermemoryConnection>,
}

impl SupermemoryLifecycle {
    /// Start the configured Supermemory lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryLifecycleError`] when managed directory setup,
    /// status persistence, or child-process startup fails.
    pub async fn start(
        paths: &crate::NoemaPaths,
        settings: &crate::MemoryServiceSettingsRecord,
        store: crate::NoemaStore,
        system_errors: crate::SystemErrorLogger,
    ) -> Result<Self, SupermemoryLifecycleError> {
        match settings.mode {
            crate::MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
            }),
            crate::MemoryServiceMode::Managed => {
                let binary = match SupermemoryBinaryResolver::default_for_paths(paths).resolve() {
                    Ok(binary) => binary,
                    Err(error) => {
                        persist_lifecycle_failure(
                            &store,
                            error.sanitized_code(),
                            error.sanitized_message(),
                        )
                        .await?;
                        system_errors.try_append(
                            crate::SystemErrorEvent::new(
                                "supermemory_start_failed",
                                "Supermemory managed process could not start",
                            )
                            .with_error_chain([error.to_string()]),
                        );
                        return Err(SupermemoryLifecycleError::Start(error.to_string()));
                    }
                };
                let port = allocate_loopback_port()?;
                Self::start_with_resolved_binary_and_port(
                    paths,
                    settings,
                    store,
                    system_errors,
                    binary,
                    port,
                )
                .await
            }
        }
    }

    /// Runtime-only connection used by Noema to reach the managed sidecar.
    #[must_use]
    pub fn connection(&self) -> Option<&SupermemoryConnection> {
        self.connection.as_ref()
    }

    async fn start_with_resolved_binary_and_port(
        paths: &crate::NoemaPaths,
        settings: &crate::MemoryServiceSettingsRecord,
        store: crate::NoemaStore,
        system_errors: crate::SystemErrorLogger,
        binary: SupermemoryServerBinary,
        port: u16,
    ) -> Result<Self, SupermemoryLifecycleError> {
        match settings.mode {
            crate::MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
            }),
            crate::MemoryServiceMode::Managed => {
                tokio::fs::create_dir_all(paths.supermemory_data_dir()).await?;
                tokio::fs::create_dir_all(paths.supermemory_secrets_dir()).await?;
                store
                    .save_memory_service_status(crate::MemoryServiceStatusRecord {
                        status_id: "default".to_string(),
                        status: crate::MemoryServiceStatus::Starting,
                        checked_at: None,
                        last_error_code: None,
                        last_error_message: None,
                    })
                    .await?;

                let connection =
                    SupermemoryConnection::new(format!("http://127.0.0.1:{port}"), None);
                let mut command = tokio::process::Command::new(&binary.path);
                command
                    .env("SUPERMEMORY_DATA_DIR", paths.supermemory_data_dir())
                    .env("SUPERMEMORY_PORT", port.to_string())
                    .env("PORT", port.to_string())
                    .kill_on_drop(true);
                let child = match command.spawn() {
                    Ok(child) => child,
                    Err(error) => {
                        let (error_code, error_message) = start_error_details(&error);
                        store
                            .save_memory_service_status(crate::MemoryServiceStatusRecord {
                                status_id: "default".to_string(),
                                status: crate::MemoryServiceStatus::Unavailable,
                                checked_at: now_rfc3339().ok(),
                                last_error_code: Some(error_code),
                                last_error_message: Some(error_message),
                            })
                            .await?;
                        system_errors.try_append(
                            crate::SystemErrorEvent::new(
                                "supermemory_start_failed",
                                "Supermemory managed process could not start",
                            )
                            .with_error_chain([error.to_string()]),
                        );
                        return Err(SupermemoryLifecycleError::Start(error.to_string()));
                    }
                };

                Ok(Self {
                    child: Some(child),
                    connection: Some(connection),
                })
            }
        }
    }

    /// Stop the managed Supermemory child process, when one was started.
    pub async fn shutdown(mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill().await;
        }
    }
}

async fn persist_lifecycle_failure(
    store: &crate::NoemaStore,
    code: &str,
    message: &str,
) -> Result<(), crate::StoreError> {
    store
        .save_memory_service_status(crate::MemoryServiceStatusRecord {
            status_id: "default".to_string(),
            status: crate::MemoryServiceStatus::Unavailable,
            checked_at: now_rfc3339().ok(),
            last_error_code: Some(code.to_string()),
            last_error_message: Some(message.to_string()),
        })
        .await?;
    Ok(())
}

fn start_error_details(error: &std::io::Error) -> (String, String) {
    if error.kind() == ErrorKind::NotFound {
        return (
            "supermemory_server_missing".to_string(),
            "supermemory-server executable was not found on PATH".to_string(),
        );
    }
    (
        "supermemory_start_failed".to_string(),
        "Supermemory managed process could not start".to_string(),
    )
}

fn now_rfc3339() -> Result<String, time::error::Format> {
    OffsetDateTime::now_utc().format(&Rfc3339)
}

/// Errors returned by Supermemory lifecycle startup.
#[derive(Debug, Error)]
pub enum SupermemoryLifecycleError {
    /// Managed directory setup failed.
    #[error("supermemory directory setup failed: {0}")]
    Io(#[from] std::io::Error),
    /// Memory service status persistence failed.
    #[error("supermemory status persistence failed: {0}")]
    Store(#[from] crate::StoreError),
    /// Managed child process startup failed.
    #[error("supermemory startup failed: {0}")]
    Start(String),
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    #[tokio::test]
    async fn managed_start_failure_persists_unavailable_status() {
        let home = TempDir::new().expect("temp noema home");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
            .await
            .expect("open store");
        let settings = store.memory_service_settings().await.expect("settings");
        let error_logger = store.system_error_logger();

        let error = match super::SupermemoryLifecycle::start_with_resolved_binary_and_port(
            &paths,
            &settings,
            store.clone(),
            error_logger,
            crate::supermemory::SupermemoryServerBinary {
                path: "/definitely/missing/supermemory-server".into(),
                source: crate::supermemory::SupermemoryBinarySource::Environment,
            },
            6768,
        )
        .await
        {
            Ok(_) => panic!("missing executable should fail startup"),
            Err(error) => error,
        };

        assert!(matches!(error, super::SupermemoryLifecycleError::Start(_)));
        let status = store.memory_service_status().await.expect("status");
        assert_eq!(status.status, crate::MemoryServiceStatus::Unavailable);
        assert_eq!(
            status.last_error_code.as_deref(),
            Some("supermemory_server_missing")
        );
        assert_eq!(
            status.last_error_message.as_deref(),
            Some("supermemory-server executable was not found on PATH")
        );
    }
}
