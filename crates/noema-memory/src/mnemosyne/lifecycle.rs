//! Managed Mnemosyne sidecar lifecycle.

use std::{env, path::Path, process::ExitStatus, process::Stdio};

use noema_home::{SystemErrorEvent, SystemErrorLogger};
use thiserror::Error;
use tokio::time::{Duration, sleep};

use super::{MnemosyneConnection, endpoint::allocate_loopback_port};
use crate::{MemoryServiceMode, MemoryServiceSettingsRecord, model_proxy::MemoryModelProxy};

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
        data_dir: &Path,
        runtime_dir: &Path,
        settings: &MemoryServiceSettingsRecord,
        system_errors: SystemErrorLogger,
        model_proxy: Option<MemoryModelProxy>,
    ) -> Result<Self, MnemosyneLifecycleError> {
        match settings.mode {
            MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
                model_proxy: None,
            }),
            MemoryServiceMode::Managed => {
                let port = match allocate_loopback_port() {
                    Ok(port) => port,
                    Err(error) => {
                        shutdown_managed_resources(None, model_proxy).await;
                        return Err(error.into());
                    }
                };
                let command = MnemosyneSidecarCommand::from_env();
                Self::start_with_command_and_port(
                    data_dir,
                    runtime_dir,
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
        data_dir: &Path,
        runtime_dir: &Path,
        system_errors: SystemErrorLogger,
        command: MnemosyneSidecarCommand,
        port: u16,
        model_proxy: Option<MemoryModelProxy>,
    ) -> Result<Self, MnemosyneLifecycleError> {
        let Some(model_proxy) = model_proxy else {
            return Err(MnemosyneLifecycleError::Start(
                "memory model proxy is unavailable".to_string(),
            ));
        };
        if let Err(error) = create_managed_directories(data_dir, runtime_dir).await {
            shutdown_managed_resources(None, Some(model_proxy)).await;
            return Err(error.into());
        }

        let connection = MnemosyneConnection::new(format!("http://127.0.0.1:{port}"), None);
        let mut process = command.into_process_command(port);
        process
            .env("NOEMA_MNEMOSYNE_DATA_DIR", data_dir)
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
                    SystemErrorEvent::new(
                        "mnemosyne_start_failed",
                        "Mnemosyne managed process could not start",
                    )
                    .with_error_chain([error.to_string()]),
                );
                shutdown_managed_resources(None, Some(model_proxy)).await;
                return Err(MnemosyneLifecycleError::Start(
                    "Mnemosyne managed process could not start".to_string(),
                ));
            }
        };

        let early_exit = match wait_for_early_child_exit(&mut child).await {
            Ok(status) => status,
            Err(error) => {
                shutdown_managed_resources(Some(&mut child), Some(model_proxy)).await;
                return Err(error.into());
            }
        };
        if let Some(status) = early_exit {
            let error_message = format!("Mnemosyne managed process exited with status {status}");
            system_errors.try_append(
                SystemErrorEvent::new(
                    "mnemosyne_start_failed",
                    "Mnemosyne managed process exited during startup",
                )
                .with_error_chain([error_message.clone()]),
            );
            shutdown_managed_resources(Some(&mut child), Some(model_proxy)).await;
            return Err(MnemosyneLifecycleError::Start(error_message));
        }

        Ok(Self {
            child: Some(child),
            connection: Some(connection),
            model_proxy: Some(model_proxy),
        })
    }

    /// Stop the managed Mnemosyne child process, when one was started.
    pub async fn shutdown(mut self) {
        shutdown_managed_resources(self.child.as_mut(), self.model_proxy.take()).await;
    }

    #[cfg(test)]
    pub(crate) async fn start_with_command_for_test(
        data_dir: &Path,
        runtime_dir: &Path,
        command: String,
        model_proxy: MemoryModelProxy,
    ) -> Result<Self, MnemosyneLifecycleError> {
        Self::start_with_command_and_port(
            data_dir,
            runtime_dir,
            SystemErrorLogger::new(runtime_dir.join("errors.jsonl")),
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
    #[cfg(test)]
    MissingExecutable,
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
                    .current_dir(
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mnemosyne-sidecar"),
                    );
                command
            }
            Self::Shell(script) => {
                let mut command = tokio::process::Command::new("sh");
                command.arg("-c").arg(script);
                command
            }
            #[cfg(test)]
            Self::MissingExecutable => {
                tokio::process::Command::new("/noema/tests/missing-mnemosyne-sidecar")
            }
        }
    }
}

async fn create_managed_directories(
    data_dir: &Path,
    runtime_dir: &Path,
) -> Result<(), std::io::Error> {
    tokio::fs::create_dir_all(data_dir).await?;
    tokio::fs::create_dir_all(runtime_dir).await?;
    Ok(())
}

async fn shutdown_managed_resources(
    child: Option<&mut tokio::process::Child>,
    model_proxy: Option<MemoryModelProxy>,
) {
    if let Some(child) = child {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    if let Some(model_proxy) = model_proxy {
        model_proxy.shutdown().await;
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
        let paths = crate::paths::MemoryServicePaths::from_noema_root(home.path());
        let (proxy, api_key) = start_test_proxy().await;
        let openai_base_url = proxy.openai_base_url().to_string();
        let command = write_sleeping_mnemosyne_sidecar(home.path());

        let lifecycle = super::MnemosyneLifecycle::start_with_command_for_test(
            &paths.data_dir(),
            &paths.runtime_dir(),
            format!("sh {}", command.display()),
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
        let env_file = paths.data_dir().join("model-env.txt");
        let mut env_text = None;
        for _ in 0..20 {
            env_text = tokio::fs::read_to_string(&env_file).await.ok();
            if env_text.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let env_text = env_text.expect("sidecar env file");
        assert!(env_text.contains(&format!("NOEMA_MEMORY_OPENAI_BASE_URL={openai_base_url}")));
        assert!(env_text.contains(&format!("NOEMA_MEMORY_OPENAI_API_KEY={api_key}")));
        assert!(env_text.contains("NOEMA_MEMORY_MODEL=memory-model"));
        assert!(env_text.contains("NOEMA_MNEMOSYNE_PORT=0"));

        lifecycle.shutdown().await;
    }

    #[tokio::test]
    async fn managed_startup_failures_stop_model_proxy_at_every_boundary() {
        #[derive(Clone, Copy, Debug)]
        enum Failure {
            Directory,
            Spawn,
            EarlyExit,
        }

        for failure in [Failure::Directory, Failure::Spawn, Failure::EarlyExit] {
            let home = TempDir::new().expect("temp noema home");
            let paths = crate::paths::MemoryServicePaths::from_noema_root(home.path());
            let data_dir = if matches!(failure, Failure::Directory) {
                let path = home.path().join("data-file");
                fs::write(&path, "not a directory").expect("write conflicting data path");
                path
            } else {
                paths.data_dir()
            };
            let (proxy, api_key) = start_test_proxy().await;
            let openai_base_url = proxy.openai_base_url().to_string();
            let result = match failure {
                Failure::Spawn => {
                    super::MnemosyneLifecycle::start_with_command_and_port(
                        &data_dir,
                        &paths.runtime_dir(),
                        noema_home::SystemErrorLogger::new(
                            paths.runtime_dir().join("errors.jsonl"),
                        ),
                        super::MnemosyneSidecarCommand::MissingExecutable,
                        0,
                        Some(proxy),
                    )
                    .await
                }
                Failure::Directory | Failure::EarlyExit => {
                    super::MnemosyneLifecycle::start_with_command_for_test(
                        &data_dir,
                        &paths.runtime_dir(),
                        if matches!(failure, Failure::Directory) {
                            "sleep 5"
                        } else {
                            "exit 7"
                        }
                        .to_string(),
                        proxy,
                    )
                    .await
                }
            };
            assert!(
                match (failure, result) {
                    (Failure::Directory, Err(super::MnemosyneLifecycleError::Io(_))) => true,
                    (Failure::Spawn, Err(super::MnemosyneLifecycleError::Start(message))) =>
                        message == "Mnemosyne managed process could not start",
                    (Failure::EarlyExit, Err(super::MnemosyneLifecycleError::Start(message))) =>
                        message.contains("exited with status"),
                    _ => false,
                },
                "unexpected {failure:?} result"
            );
            assert_proxy_stopped(&openai_base_url, &api_key).await;
        }

        let (proxy, api_key) = start_test_proxy().await;
        let openai_base_url = proxy.openai_base_url().to_string();
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .spawn()
            .expect("spawn cleanup child");
        super::shutdown_managed_resources(Some(&mut child), Some(proxy)).await;
        let status = child
            .try_wait()
            .expect("inspect cleanup child")
            .expect("failed-start cleanup must reap its running child");
        assert!(
            !status.success(),
            "failed-start cleanup must kill the child"
        );
        assert_proxy_stopped(&openai_base_url, &api_key).await;
    }

    use crate::model_proxy::test_support::start_test_proxy;

    async fn assert_proxy_stopped(openai_base_url: &str, api_key: &str) {
        let response = reqwest::Client::new()
            .post(format!("{openai_base_url}/chat/completions"))
            .bearer_auth(api_key)
            .json(&serde_json::json!({
                "model": "memory-model",
                "messages": [{"role": "user", "content": "probe shutdown"}]
            }))
            .send()
            .await;
        if let Ok(response) = response {
            assert_eq!(
                response.status(),
                reqwest::StatusCode::UNAUTHORIZED,
                "the original memory model proxy still accepted its private key"
            );
        }
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
}
