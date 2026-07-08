//! Managed Mnemosyne sidecar lifecycle.

use std::{env, process::ExitStatus, process::Stdio};

use thiserror::Error;
use tokio::time::{Duration, sleep};

use super::{MnemosyneConnection, allocate_loopback_port};
use crate::MemoryModelProxy;

/// Environment variable that overrides the managed Mnemosyne sidecar launch command.
pub const NOEMA_MNEMOSYNE_SIDECAR_COMMAND_ENV: &str = "NOEMA_MNEMOSYNE_SIDECAR_COMMAND";

/// Mnemosyne process lifecycle owned by the runtime host.
pub struct MnemosyneLifecycle {
    child: Option<tokio::process::Child>,
    connection: Option<MnemosyneConnection>,
    model_proxy: Option<MemoryModelProxy>,
}

impl MnemosyneLifecycle {
    /// Start the configured Mnemosyne lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`MnemosyneLifecycleError`] when managed directory setup or
    /// child-process startup fails.
    pub async fn start(
        paths: &crate::NoemaPaths,
        settings: &crate::MemoryServiceSettingsRecord,
        system_errors: crate::SystemErrorLogger,
        model_proxy: Option<MemoryModelProxy>,
    ) -> Result<Self, MnemosyneLifecycleError> {
        match settings.mode {
            crate::MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
                model_proxy: None,
            }),
            crate::MemoryServiceMode::Managed => {
                let port = allocate_loopback_port()?;
                let command = MnemosyneSidecarCommand::from_env();
                Self::start_with_command_and_port(
                    paths,
                    settings,
                    system_errors,
                    command,
                    port,
                    model_proxy,
                )
                .await
            }
        }
    }

    /// Runtime-only connection used by Noema to reach the managed sidecar.
    #[must_use]
    pub fn connection(&self) -> Option<&MnemosyneConnection> {
        self.connection.as_ref()
    }

    async fn start_with_command_and_port(
        paths: &crate::NoemaPaths,
        settings: &crate::MemoryServiceSettingsRecord,
        system_errors: crate::SystemErrorLogger,
        command: MnemosyneSidecarCommand,
        port: u16,
        model_proxy: Option<MemoryModelProxy>,
    ) -> Result<Self, MnemosyneLifecycleError> {
        match settings.mode {
            crate::MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
                model_proxy: None,
            }),
            crate::MemoryServiceMode::Managed => {
                let Some(model_proxy) = model_proxy else {
                    return Err(MnemosyneLifecycleError::Start(
                        "memory model proxy is unavailable".to_string(),
                    ));
                };
                tokio::fs::create_dir_all(paths.mnemosyne_data_dir()).await?;
                tokio::fs::create_dir_all(paths.mnemosyne_runtime_dir()).await?;

                let connection = MnemosyneConnection::new(format!("http://127.0.0.1:{port}"), None);
                let mut process = command.into_process_command(port);
                process
                    .env("NOEMA_MNEMOSYNE_DATA_DIR", paths.mnemosyne_data_dir())
                    .env("NOEMA_MNEMOSYNE_PORT", port.to_string())
                    .env(
                        "NOEMA_MEMORY_OPENAI_BASE_URL",
                        model_proxy.openai_base_url(),
                    )
                    .env("NOEMA_MEMORY_OPENAI_API_KEY", model_proxy.api_key())
                    .env("NOEMA_MEMORY_MODEL", model_proxy.model_profile())
                    .stdin(Stdio::null())
                    .kill_on_drop(true);

                let mut child = match process.spawn() {
                    Ok(child) => child,
                    Err(error) => {
                        system_errors.try_append(
                            crate::SystemErrorEvent::new(
                                "mnemosyne_start_failed",
                                "Mnemosyne managed process could not start",
                            )
                            .with_error_chain([error.to_string()]),
                        );
                        return Err(MnemosyneLifecycleError::Start(
                            "Mnemosyne managed process could not start".to_string(),
                        ));
                    }
                };

                if let Some(status) = wait_for_early_child_exit(&mut child).await? {
                    let error_message =
                        format!("Mnemosyne managed process exited with status {status}");
                    system_errors.try_append(
                        crate::SystemErrorEvent::new(
                            "mnemosyne_start_failed",
                            "Mnemosyne managed process exited during startup",
                        )
                        .with_error_chain([error_message.clone()]),
                    );
                    return Err(MnemosyneLifecycleError::Start(error_message));
                }

                Ok(Self {
                    child: Some(child),
                    connection: Some(connection),
                    model_proxy: Some(model_proxy),
                })
            }
        }
    }

    /// Stop the managed Mnemosyne child process, when one was started.
    pub async fn shutdown(mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill().await;
        }
        if let Some(model_proxy) = self.model_proxy {
            model_proxy.shutdown().await;
        }
    }

    #[cfg(test)]
    pub(crate) async fn start_with_command_for_test(
        paths: &crate::NoemaPaths,
        command: String,
        model_proxy: MemoryModelProxy,
    ) -> Result<Self, MnemosyneLifecycleError> {
        Self::start_with_command_and_port(
            paths,
            &crate::MemoryServiceSettingsRecord {
                settings_id: "default".to_string(),
                mode: crate::MemoryServiceMode::Managed,
                base_url: None,
                port: None,
                provider_account_id: None,
                provider_kind: None,
                model_profile: None,
                reasoning_effort: None,
            },
            crate::SystemErrorLogger::new(paths.root().join("errors.jsonl")),
            MnemosyneSidecarCommand::Shell(command),
            0,
            Some(model_proxy),
        )
        .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MnemosyneSidecarCommand {
    Default,
    Shell(String),
}

impl MnemosyneSidecarCommand {
    fn from_env() -> Self {
        env::var(NOEMA_MNEMOSYNE_SIDECAR_COMMAND_ENV)
            .ok()
            .filter(|command| !command.trim().is_empty())
            .map_or(Self::Default, Self::Shell)
    }

    fn into_process_command(self, port: u16) -> tokio::process::Command {
        match self {
            Self::Default => {
                let mut command = tokio::process::Command::new("python3");
                command
                    .arg("-m")
                    .arg("uvicorn")
                    .arg("--factory")
                    .arg("noema_mnemosyne_sidecar.app:create_app")
                    .arg("--host")
                    .arg("127.0.0.1")
                    .arg("--port")
                    .arg(port.to_string())
                    .current_dir(env!("NOEMA_MNEMOSYNE_SIDECAR_DIR"));
                command
            }
            Self::Shell(script) => {
                let mut command = tokio::process::Command::new("sh");
                command.arg("-c").arg(script);
                command
            }
        }
    }
}

async fn wait_for_early_child_exit(
    child: &mut tokio::process::Child,
) -> Result<Option<ExitStatus>, std::io::Error> {
    for _ in 0..20 {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        sleep(Duration::from_millis(50)).await;
    }
    Ok(None)
}

/// Errors returned by Mnemosyne lifecycle startup.
#[derive(Debug, Error)]
pub enum MnemosyneLifecycleError {
    /// Managed directory setup failed.
    #[error("mnemosyne directory setup failed: {0}")]
    Io(#[from] std::io::Error),
    /// Managed child process startup failed.
    #[error("mnemosyne startup failed: {0}")]
    Start(String),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    #[tokio::test]
    async fn managed_mnemosyne_lifecycle_exports_private_sidecar_env() {
        let home = TempDir::new().expect("temp noema home");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let proxy = crate::MemoryModelProxy::start(crate::MemoryModelProxyConfig {
            provider: std::sync::Arc::new(StaticProvider),
            api_key: "proxy-secret".to_string(),
            model_profile: "memory-model".to_string(),
            reasoning_effort: None,
            system_errors: None,
        })
        .await
        .expect("start proxy");
        let openai_base_url = proxy.openai_base_url().to_string();
        let command = write_sleeping_mnemosyne_sidecar(home.path());

        let lifecycle = super::MnemosyneLifecycle::start_with_command_for_test(
            &paths,
            command.to_string_lossy().to_string(),
            proxy,
        )
        .await
        .expect("start lifecycle");

        assert!(
            lifecycle
                .connection()
                .expect("connection")
                .base_url
                .starts_with("http://127.0.0.1:")
        );
        let env_file = paths.mnemosyne_data_dir().join("model-env.txt");
        let env_text = tokio::fs::read_to_string(env_file).await.expect("env file");
        assert!(env_text.contains(&format!("NOEMA_MEMORY_OPENAI_BASE_URL={openai_base_url}")));
        assert!(env_text.contains("NOEMA_MEMORY_OPENAI_API_KEY=proxy-secret"));
        assert!(env_text.contains("NOEMA_MEMORY_MODEL=memory-model"));
        assert!(env_text.contains("NOEMA_MNEMOSYNE_PORT=0"));

        lifecycle.shutdown().await;
    }

    fn write_sleeping_mnemosyne_sidecar(root: &std::path::Path) -> std::path::PathBuf {
        let path = root.join("mnemosyne-sidecar");
        fs::write(
            &path,
            r#"#!/bin/sh
mkdir -p "$NOEMA_MNEMOSYNE_DATA_DIR"
{
  echo "NOEMA_MEMORY_OPENAI_BASE_URL=$NOEMA_MEMORY_OPENAI_BASE_URL"
  echo "NOEMA_MEMORY_OPENAI_API_KEY=$NOEMA_MEMORY_OPENAI_API_KEY"
  echo "NOEMA_MEMORY_MODEL=$NOEMA_MEMORY_MODEL"
  echo "NOEMA_MNEMOSYNE_PORT=$NOEMA_MNEMOSYNE_PORT"
} > "$NOEMA_MNEMOSYNE_DATA_DIR/model-env.txt"
sleep 5
"#,
        )
        .expect("write fake sidecar");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("chmod");
        }
        path
    }

    #[derive(Debug)]
    struct StaticProvider;

    impl crate::daemon::RuntimeModelProvider for StaticProvider {
        fn generate_streaming<'a>(
            &'a self,
            _request: crate::provider::GenerateRequest,
            _on_event: &'a mut (dyn FnMut(crate::provider::GenerateStreamEvent) + Send),
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            crate::provider::GenerateResponse,
                            crate::provider::ProviderError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async {
                Ok(crate::provider::GenerateResponse::final_text(
                    "ok",
                    "test",
                    "memory-model",
                ))
            })
        }
    }
}
