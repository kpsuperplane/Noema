//! CLI command implementations.

use crate::{
    Args, CliError,
    dev::{DevDaemonOptions, run_dev_daemon},
};
use noema_core::{DaemonClient, DaemonError, default_socket_path, is_connection_refused};
use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    time,
};

pub(crate) mod chat;
pub(crate) mod config;
pub(crate) mod memory;
pub(crate) mod start;

pub(crate) use chat::{run_chat, run_one_shot};
pub(crate) use config::run_config;
pub(crate) use memory::run_memory_graphql;
pub(crate) use start::run_start;

pub(crate) async fn run_dev_daemon_command(args: &Args) -> Result<(), CliError> {
    run_dev_daemon(dev_daemon_options(args))
        .await
        .map_err(Into::into)
}

fn dev_daemon_options(args: &Args) -> DevDaemonOptions {
    DevDaemonOptions {
        provider: args.provider.clone(),
        model: args.model.clone(),
        base_url: args.base_url.clone(),
        config: args.config.clone(),
    }
}

pub(crate) struct ConnectedDaemon {
    pub(crate) client: DaemonClient,
    temporary_child: Option<Child>,
}

impl ConnectedDaemon {
    pub(crate) async fn connect_or_start(args: &Args) -> Result<Self, CliError> {
        let socket_path = default_socket_path()?;
        match DaemonClient::connect(&socket_path).await {
            Ok(mut client) => {
                client.hello().await?;
                Ok(Self {
                    client,
                    temporary_child: None,
                })
            }
            Err(error) if is_connection_refused(&error) => {
                eprintln!(
                    "noema daemon is not running; starting a temporary daemon for this chat. Run `noema start` to keep it warm."
                );
                Self::start_temporary(args, socket_path).await
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn start_temporary(args: &Args, socket_path: PathBuf) -> Result<Self, CliError> {
        let mut child = spawn_temporary_daemon(args)?;
        let client = wait_for_daemon(&socket_path, &mut child).await?;

        Ok(Self {
            client,
            temporary_child: Some(child),
        })
    }

    pub(crate) async fn shutdown_if_temporary(&mut self) -> Result<(), CliError> {
        let Some(mut child) = self.temporary_child.take() else {
            return Ok(());
        };

        let _ = self.client.shutdown().await;
        match time::timeout(Duration::from_secs(5), child.wait()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(CliError::SpawnDaemon(error)),
            Err(_) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                Ok(())
            }
        }
    }
}

fn spawn_temporary_daemon(args: &Args) -> Result<Child, CliError> {
    let exe = env::current_exe().map_err(CliError::CurrentExe)?;
    let mut command = Command::new(exe);

    if let Some(config) = args.config.as_ref() {
        command.arg("--config").arg(config);
    }
    if let Some(model) = args.model.as_ref() {
        command.arg("--model").arg(model);
    }
    if let Some(base_url) = args.base_url.as_ref() {
        command.arg("--base-url").arg(base_url);
    }

    command
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.kill_on_drop(true);
    command.spawn().map_err(CliError::SpawnDaemon)
}

async fn wait_for_daemon(socket_path: &Path, child: &mut Child) -> Result<DaemonClient, CliError> {
    let deadline = time::Instant::now() + Duration::from_secs(10);

    loop {
        if let Some(status) = child.try_wait().map_err(CliError::SpawnDaemon)? {
            return Err(CliError::Daemon(DaemonError::Protocol(format!(
                "temporary daemon exited before accepting connections: {status}"
            ))));
        }

        match DaemonClient::connect(socket_path).await {
            Ok(mut client) => {
                client.hello().await?;
                return Ok(client);
            }
            Err(error) if is_connection_refused(&error) && time::Instant::now() < deadline => {
                time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}
