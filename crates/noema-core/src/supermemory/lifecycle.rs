//! Managed Supermemory child-process lifecycle.

use thiserror::Error;

/// Supermemory process lifecycle owned by the runtime host.
pub struct SupermemoryLifecycle {
    child: Option<tokio::process::Child>,
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
            crate::MemoryServiceMode::External => Ok(Self { child: None }),
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

                let mut command = tokio::process::Command::new("supermemory-server");
                command
                    .env("SUPERMEMORY_DATA_DIR", paths.supermemory_data_dir())
                    .env(
                        "SUPERMEMORY_PORT",
                        settings.port.unwrap_or(6767).to_string(),
                    )
                    .kill_on_drop(true);
                let child = command.spawn().map_err(|error| {
                    system_errors.try_append(
                        crate::SystemErrorEvent::new(
                            "supermemory_start_failed",
                            "Supermemory managed process could not start",
                        )
                        .with_error_chain([error.to_string()]),
                    );
                    SupermemoryLifecycleError::Start(error.to_string())
                })?;

                Ok(Self { child: Some(child) })
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
