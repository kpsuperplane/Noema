//! Managed Supermemory child-process lifecycle.

use std::{io::ErrorKind, process::ExitStatus};

use thiserror::Error;
use tokio::time::{Duration, sleep};

use super::{
    SupermemoryBinaryResolver, SupermemoryConnection, SupermemoryServerBinary,
    allocate_loopback_port,
};
use crate::supermemory::SupermemoryModelProxy;

/// Supermemory process lifecycle owned by the runtime host.
pub struct SupermemoryLifecycle {
    child: Option<tokio::process::Child>,
    connection: Option<SupermemoryConnection>,
    model_proxy: Option<SupermemoryModelProxy>,
}

impl SupermemoryLifecycle {
    /// Start the configured Supermemory lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryLifecycleError`] when managed directory setup or
    /// child-process startup fails.
    pub async fn start(
        paths: &crate::NoemaPaths,
        settings: &crate::MemoryServiceSettingsRecord,
        store: crate::NoemaStore,
        system_errors: crate::SystemErrorLogger,
        model_proxy: Option<SupermemoryModelProxy>,
    ) -> Result<Self, SupermemoryLifecycleError> {
        match settings.mode {
            crate::MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
                model_proxy: None,
            }),
            crate::MemoryServiceMode::Managed => {
                let binary = match SupermemoryBinaryResolver::default_for_paths(paths).resolve() {
                    Ok(binary) => binary,
                    Err(error) => {
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
                    model_proxy,
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
        _store: crate::NoemaStore,
        system_errors: crate::SystemErrorLogger,
        binary: SupermemoryServerBinary,
        port: u16,
        model_proxy: Option<SupermemoryModelProxy>,
    ) -> Result<Self, SupermemoryLifecycleError> {
        match settings.mode {
            crate::MemoryServiceMode::External => Ok(Self {
                child: None,
                connection: None,
                model_proxy: None,
            }),
            crate::MemoryServiceMode::Managed => {
                tokio::fs::create_dir_all(paths.supermemory_data_dir()).await?;
                tokio::fs::create_dir_all(paths.supermemory_secrets_dir()).await?;

                let connection =
                    SupermemoryConnection::new(format!("http://127.0.0.1:{port}"), None);
                let mut command = tokio::process::Command::new(&binary.path);
                command
                    .env("SUPERMEMORY_DATA_DIR", paths.supermemory_data_dir())
                    .env("SUPERMEMORY_PORT", port.to_string())
                    .env("PORT", port.to_string())
                    .kill_on_drop(true);
                if let Some(proxy) = &model_proxy {
                    command
                        .env("OPENAI_BASE_URL", proxy.openai_base_url())
                        .env("OPENAI_API_KEY", proxy.api_key())
                        .env("OPENAI_MODEL", proxy.model_profile())
                        .env("OPENAI_FAST_MODEL", proxy.model_profile())
                        .env("OPENAI_TEXT_MODEL", proxy.model_profile());
                }
                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(error) => {
                        let (_error_code, error_message) = start_error_details(&error);
                        system_errors.try_append(
                            crate::SystemErrorEvent::new(
                                "supermemory_start_failed",
                                "Supermemory managed process could not start",
                            )
                            .with_error_chain([error.to_string()]),
                        );
                        return Err(SupermemoryLifecycleError::Start(error_message));
                    }
                };
                if let Some(status) = wait_for_early_child_exit(&mut child).await? {
                    let error_message = recent_supermemory_error_message(paths)
                        .await
                        .unwrap_or_else(|| {
                            format!("Supermemory managed process exited with status {status}")
                        });
                    system_errors.try_append(
                        crate::SystemErrorEvent::new(
                            "supermemory_start_failed",
                            "Supermemory managed process exited during startup",
                        )
                        .with_error_chain([error_message.clone()]),
                    );
                    return Err(SupermemoryLifecycleError::Start(error_message));
                }

                Ok(Self {
                    child: Some(child),
                    connection: Some(connection),
                    model_proxy,
                })
            }
        }
    }

    /// Stop the managed Supermemory child process, when one was started.
    pub async fn shutdown(mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill().await;
        }
        if let Some(model_proxy) = self.model_proxy {
            model_proxy.shutdown().await;
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

async fn recent_supermemory_error_message(paths: &crate::NoemaPaths) -> Option<String> {
    let text = tokio::fs::read_to_string(paths.supermemory_data_dir().join("error.log"))
        .await
        .ok()?;
    text.lines()
        .rev()
        .find_map(sanitize_supermemory_error_log_line)
}

fn sanitize_supermemory_error_log_line(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let line = line
        .split_once("] ")
        .map_or(line, |(_prefix, message)| message)
        .trim();
    let line = line
        .strip_prefix("fatal during startup:")
        .unwrap_or(line)
        .trim();
    let message = line
        .split_once(". ")
        .map_or(line, |(first_sentence, _rest)| first_sentence)
        .trim();
    if message.is_empty() {
        None
    } else if message.ends_with('.') {
        Some(message.to_string())
    } else {
        Some(format!("{message}."))
    }
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

/// Errors returned by Supermemory lifecycle startup.
#[derive(Debug, Error)]
pub enum SupermemoryLifecycleError {
    /// Managed directory setup failed.
    #[error("supermemory directory setup failed: {0}")]
    Io(#[from] std::io::Error),
    /// Managed child process startup failed.
    #[error("supermemory startup failed: {0}")]
    Start(String),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    #[tokio::test]
    async fn managed_start_failure_returns_start_error() {
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
            None,
        )
        .await
        {
            Ok(_) => panic!("missing executable should fail startup"),
            Err(error) => error,
        };

        assert!(matches!(error, super::SupermemoryLifecycleError::Start(_)));
        assert!(
            error
                .to_string()
                .contains("supermemory-server executable was not found on PATH")
        );
    }

    #[tokio::test]
    async fn managed_child_early_exit_returns_error_log_message() {
        let home = TempDir::new().expect("temp noema home");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
            .await
            .expect("open store");
        let settings = store.memory_service_settings().await.expect("settings");
        let error_logger = store.system_error_logger();
        let binary = home.path().join("supermemory-server");
        write_exiting_supermemory_server(&binary);

        let error = match super::SupermemoryLifecycle::start_with_resolved_binary_and_port(
            &paths,
            &settings,
            store.clone(),
            error_logger,
            crate::supermemory::SupermemoryServerBinary {
                path: binary,
                source: crate::supermemory::SupermemoryBinarySource::Bundled,
            },
            6769,
            None,
        )
        .await
        {
            Ok(_) => panic!("early child exit should fail startup"),
            Err(error) => error,
        };

        assert!(matches!(error, super::SupermemoryLifecycleError::Start(_)));
        assert!(
            error
                .to_string()
                .contains("No model provider API key configured.")
        );
    }

    #[tokio::test]
    async fn managed_child_receives_private_model_proxy_env() {
        let home = TempDir::new().expect("temp noema home");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
            .await
            .expect("open store");
        let settings = store.memory_service_settings().await.expect("settings");
        let error_logger = store.system_error_logger();
        let binary = home.path().join("supermemory-server");
        write_sleeping_supermemory_server(&binary);
        let proxy = crate::supermemory::SupermemoryModelProxy::start(
            crate::supermemory::SupermemoryModelProxyConfig {
                provider: std::sync::Arc::new(StaticProvider),
                api_key: "proxy-secret".to_string(),
                model_profile: "memory-model".to_string(),
                reasoning_effort: None,
                system_errors: None,
            },
        )
        .await
        .expect("start proxy");
        let openai_base_url = proxy.openai_base_url().to_string();

        let lifecycle = super::SupermemoryLifecycle::start_with_resolved_binary_and_port(
            &paths,
            &settings,
            store.clone(),
            error_logger,
            crate::supermemory::SupermemoryServerBinary {
                path: binary,
                source: crate::supermemory::SupermemoryBinarySource::Bundled,
            },
            6770,
            Some(proxy),
        )
        .await
        .expect("start lifecycle");

        let env_file = paths.supermemory_data_dir().join("model-env.txt");
        let env_text = tokio::fs::read_to_string(env_file).await.expect("env file");
        assert!(env_text.contains(&format!("OPENAI_BASE_URL={openai_base_url}")));
        assert!(env_text.contains("OPENAI_API_KEY=proxy-secret"));
        assert!(env_text.contains("OPENAI_MODEL=memory-model"));
        assert!(env_text.contains("OPENAI_FAST_MODEL=memory-model"));
        assert!(env_text.contains("OPENAI_TEXT_MODEL=memory-model"));

        lifecycle.shutdown().await;
    }

    fn write_exiting_supermemory_server(path: &std::path::Path) {
        fs::write(
            path,
            r#"#!/bin/sh
mkdir -p "$SUPERMEMORY_DATA_DIR"
echo "[2026-07-08T03:46:12.343Z] fatal during startup: No model provider API key configured." > "$SUPERMEMORY_DATA_DIR/error.log"
exit 1
"#,
        )
        .expect("write fake server");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
    }

    fn write_sleeping_supermemory_server(path: &std::path::Path) {
        fs::write(
            path,
            r#"#!/bin/sh
mkdir -p "$SUPERMEMORY_DATA_DIR"
{
  echo "OPENAI_BASE_URL=$OPENAI_BASE_URL"
  echo "OPENAI_API_KEY=$OPENAI_API_KEY"
  echo "OPENAI_MODEL=$OPENAI_MODEL"
  echo "OPENAI_FAST_MODEL=$OPENAI_FAST_MODEL"
  echo "OPENAI_TEXT_MODEL=$OPENAI_TEXT_MODEL"
} > "$SUPERMEMORY_DATA_DIR/model-env.txt"
sleep 5
"#,
        )
        .expect("write fake server");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
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
