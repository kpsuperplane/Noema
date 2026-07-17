//! Managed Mnemosyne sidecar lifecycle.

use std::{env, path::Path, process::ExitStatus, process::Stdio};

use noema_home::{SystemErrorEvent, SystemErrorLogger};
use thiserror::Error;
use tokio::time::{Duration, sleep};

use super::{MnemosyneConnection, allocate_loopback_port};
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
        data_dir: &Path,
        runtime_dir: &Path,
        settings: &MemoryServiceSettingsRecord,
        system_errors: SystemErrorLogger,
        command: MnemosyneSidecarCommand,
        port: u16,
        model_proxy: Option<MemoryModelProxy>,
    ) -> Result<Self, MnemosyneLifecycleError> {
        match settings.mode {
            MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
                model_proxy: None,
            }),
            MemoryServiceMode::Managed => {
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
                    let error_message =
                        format!("Mnemosyne managed process exited with status {status}");
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
        }
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
            &MemoryServiceSettingsRecord {
                settings_id: "default".to_string(),
                mode: MemoryServiceMode::Managed,
                base_url: None,
                port: None,
                provider_account_id: None,
                provider_kind: None,
                provider_instance_key: None,
                model_profile: None,
                reasoning_effort: None,
            },
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
        let env_file = paths.data_dir().join("model-env.txt");
        let env_text = tokio::fs::read_to_string(env_file).await.expect("env file");
        assert!(env_text.contains(&format!("NOEMA_MEMORY_OPENAI_BASE_URL={openai_base_url}")));
        assert!(env_text.contains(&format!("NOEMA_MEMORY_OPENAI_API_KEY={api_key}")));
        assert!(env_text.contains("NOEMA_MEMORY_MODEL=memory-model"));
        assert!(env_text.contains("NOEMA_MNEMOSYNE_PORT=0"));

        lifecycle.shutdown().await;
    }

    #[tokio::test]
    async fn managed_directory_failure_stops_model_proxy() {
        let home = TempDir::new().expect("temp noema home");
        let data_dir = home.path().join("data-file");
        fs::write(&data_dir, "not a directory").expect("write conflicting data path");
        let runtime_dir = home.path().join("runtime");
        let (proxy, api_key) = start_test_proxy().await;
        let openai_base_url = proxy.openai_base_url().to_string();

        let result = super::MnemosyneLifecycle::start_with_command_for_test(
            &data_dir,
            &runtime_dir,
            "sleep 5".to_string(),
            proxy,
        )
        .await;

        assert!(matches!(result, Err(super::MnemosyneLifecycleError::Io(_))));
        assert_proxy_stopped(&openai_base_url, &api_key).await;
    }

    #[tokio::test]
    async fn managed_spawn_failure_stops_model_proxy() {
        let home = TempDir::new().expect("temp noema home");
        let paths = crate::paths::MemoryServicePaths::from_noema_root(home.path());
        let (proxy, api_key) = start_test_proxy().await;
        let openai_base_url = proxy.openai_base_url().to_string();

        let result = super::MnemosyneLifecycle::start_with_command_and_port(
            &paths.data_dir(),
            &paths.runtime_dir(),
            &managed_settings(),
            noema_home::SystemErrorLogger::new(paths.runtime_dir().join("errors.jsonl")),
            super::MnemosyneSidecarCommand::MissingExecutable,
            0,
            Some(proxy),
        )
        .await;

        assert!(matches!(
            result,
            Err(super::MnemosyneLifecycleError::Start(message))
                if message == "Mnemosyne managed process could not start"
        ));
        assert_proxy_stopped(&openai_base_url, &api_key).await;
    }

    #[tokio::test]
    async fn managed_early_exit_reaps_child_and_stops_model_proxy() {
        let home = TempDir::new().expect("temp noema home");
        let paths = crate::paths::MemoryServicePaths::from_noema_root(home.path());
        let (proxy, api_key) = start_test_proxy().await;
        let openai_base_url = proxy.openai_base_url().to_string();

        let result = super::MnemosyneLifecycle::start_with_command_for_test(
            &paths.data_dir(),
            &paths.runtime_dir(),
            "exit 7".to_string(),
            proxy,
        )
        .await;

        assert!(matches!(
            result,
            Err(super::MnemosyneLifecycleError::Start(message))
                if message.contains("exited with status")
        ));
        assert_proxy_stopped(&openai_base_url, &api_key).await;
    }

    #[tokio::test]
    async fn failed_start_cleanup_kills_and_waits_for_running_child() {
        let (proxy, api_key) = start_test_proxy().await;
        let openai_base_url = proxy.openai_base_url().to_string();
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .spawn()
            .expect("spawn sleeping child");

        super::shutdown_managed_resources(Some(&mut child), Some(proxy)).await;

        let status = child
            .try_wait()
            .expect("inspect cleaned-up child")
            .expect("child should already be reaped");
        assert!(!status.success());
        assert_proxy_stopped(&openai_base_url, &api_key).await;
    }

    async fn start_test_proxy() -> (crate::model_proxy::MemoryModelProxy, String) {
        static NEXT_API_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let api_key = format!(
            "proxy-secret-{}",
            NEXT_API_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let proxy = crate::model_proxy::MemoryModelProxy::start(
            crate::model_proxy::MemoryModelProxyConfig {
                route_resolver: test_route_resolver(
                    noema_providers::ProviderSelectionSnapshot::explicit(
                        "codex",
                        "provider_account:codex:memory-test",
                        "memory-model",
                        None,
                        Some("mnemosyne_test".to_string()),
                    ),
                    std::sync::Arc::new(StaticProvider),
                ),
                api_key: api_key.clone(),
                model_profile: "memory-model".to_string(),
                system_errors: None,
            },
        )
        .await
        .expect("start proxy");
        (proxy, api_key)
    }

    fn managed_settings() -> crate::MemoryServiceSettingsRecord {
        crate::MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: crate::MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: None,
            provider_kind: None,
            provider_instance_key: None,
            model_profile: None,
            reasoning_effort: None,
        }
    }

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

    fn test_route_resolver(
        mut selection: noema_providers::ProviderSelectionSnapshot,
        provider: noema_providers::ProviderHandle,
    ) -> noema_providers::ProviderRouteResolverHandle {
        let key = noema_providers::ProviderInstanceKey::new("codex:mnemosyne-lifecycle:test")
            .expect("provider key");
        selection.provider_instance_key = Some(key.clone());
        let registry = std::sync::Arc::new(noema_providers::ProviderRegistry::new());
        registry.register(key, provider).expect("register provider");
        let loader_selection = selection.clone();
        std::sync::Arc::new(noema_providers::RegistryProviderRouteResolver::new(
            noema_providers::provider_selection_loader(move || {
                let selection = loader_selection.clone();
                Box::pin(async move { Ok(selection) })
            }),
            registry,
        ))
    }

    #[derive(Debug)]
    struct StaticProvider;

    impl noema_providers::ProviderOperations for StaticProvider {
        fn generate_streaming<'a>(
            &'a self,
            _request: noema_providers::GenerateRequest,
            _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            noema_providers::GenerateResponse,
                            noema_providers::ProviderError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async {
                Ok(noema_providers::GenerateResponse::final_text(
                    "ok",
                    "test",
                    "memory-model",
                ))
            })
        }
    }
}
