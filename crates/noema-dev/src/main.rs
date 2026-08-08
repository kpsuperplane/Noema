//! Local development supervisor for Noema web mode.

mod workflow;

use std::{
    env,
    ffi::OsString,
    future::Future,
    io,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
};

use thiserror::Error;
use tokio::{
    process::{Child, Command},
    time::{Duration, timeout},
};

const WEB_ASSET_WATCH_SCRIPT: &str = "dev:assets";
const DEV_RUST_TARGET_DIR: &str = "target/noema-dev";
const WEB_SERVER_WATCH_COMMAND: &str = "run -p noema-server --bin noema_web --features dev-no-auth";
const WEB_SERVER_WATCH_IGNORE_GLOBS: [&str; 2] =
    ["apps/web/**", "crates/noema-server/target/web-assets/**"];

#[derive(Debug, Error)]
enum DevError {
    #[error(transparent)]
    Workflow(#[from] workflow::WorkflowError),

    #[error("failed to start {label}: {source}")]
    SpawnProcess {
        label: &'static str,
        source: io::Error,
    },

    #[error("failed to wait for {label}: {source}")]
    WaitProcess {
        label: &'static str,
        source: io::Error,
    },

    #[error("{label} exited with status {status}")]
    ProcessExited {
        label: &'static str,
        status: ExitStatus,
    },

    #[error("failed to install dev shutdown signal handler: {source}")]
    ShutdownSignal { source: io::Error },

    #[error("unknown Noema development mode: {mode:?}")]
    UnknownMode { mode: OsString },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run_mode().await {
        eprintln!("Noema Cargo workflow failed: {error}");
        std::process::exit(1);
    }
}

async fn run_mode() -> Result<(), DevError> {
    let mut args = env::args_os().skip(1);
    match args.next() {
        None => run_development().await,
        Some(mode) if mode == "dev" => run_development().await,
        Some(mode) if mode == "validate" => workflow::run_validation(args.collect())
            .await
            .map_err(Into::into),
        Some(mode) => Err(DevError::UnknownMode { mode }),
    }
}

async fn run_development() -> Result<(), DevError> {
    let repo_root = repo_root();
    workflow::prepare_development(&repo_root).await?;
    let web_dir = repo_root.join("apps/web");

    let mut web = spawn_web_watcher(&web_dir)?;
    let mut server = spawn_web_server_watcher(&repo_root)?;
    let mut bridge = spawn_bridge_watcher(&repo_root)?;

    eprintln!("Noema dev supervisor started");
    eprintln!("web assets: bun run {WEB_ASSET_WATCH_SCRIPT}");
    eprintln!("web server: cargo watch -x {WEB_SERVER_WATCH_COMMAND}");
    eprintln!(
        "web server target: {}",
        dev_rust_target_dir(&repo_root).display()
    );
    if bridge.is_some() {
        eprintln!("foundation bridge: cargo watch -s swift build");
    }

    supervise_dev_processes(&mut web, &mut server, bridge.as_mut(), shutdown_signal()).await
}

async fn supervise_dev_processes<S>(
    web: &mut Child,
    server: &mut Child,
    bridge: Option<&mut Child>,
    shutdown_signal: S,
) -> Result<(), DevError>
where
    S: Future<Output = Result<&'static str, DevError>>,
{
    let mut bridge = bridge;
    let result = tokio::select! {
        result = wait_for_child("web asset watcher", web) => result,
        result = wait_for_child("web server watcher", server) => result,
        result = async {
            match bridge.as_deref_mut() {
                Some(child) => wait_for_child("foundation bridge watcher", child).await,
                None => std::future::pending().await,
            }
        } => result,
        result = shutdown_signal => {
            match result {
                Ok(signal) => {
                    eprintln!("received {signal}; stopping Noema dev supervisor");
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
    };

    stop_child(web).await;
    stop_child(server).await;
    if let Some(bridge) = bridge {
        stop_child(bridge).await;
    }
    result
}

#[cfg(unix)]
async fn shutdown_signal() -> Result<&'static str, DevError> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt =
        signal(SignalKind::interrupt()).map_err(|source| DevError::ShutdownSignal { source })?;
    let mut terminate =
        signal(SignalKind::terminate()).map_err(|source| DevError::ShutdownSignal { source })?;
    let mut hangup =
        signal(SignalKind::hangup()).map_err(|source| DevError::ShutdownSignal { source })?;

    tokio::select! {
        _ = interrupt.recv() => Ok("SIGINT"),
        _ = terminate.recv() => Ok("SIGTERM"),
        _ = hangup.recv() => Ok("SIGHUP"),
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<&'static str, DevError> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|source| DevError::ShutdownSignal { source })?;
    Ok("Ctrl-C")
}

fn spawn_web_watcher(web_dir: &Path) -> Result<Child, DevError> {
    let mut command = Command::new("bun");
    command.arg("run").arg(WEB_ASSET_WATCH_SCRIPT);

    spawn_dev_process("web asset watcher", &mut command, web_dir)
}

fn spawn_web_server_watcher(repo_root: &Path) -> Result<Child, DevError> {
    let mut command = Command::new(cargo_exe());
    configure_web_server_watcher(&mut command, repo_root);

    spawn_dev_process("web server watcher", &mut command, repo_root)
}

fn configure_web_server_watcher(command: &mut Command, repo_root: &Path) {
    command
        .arg("watch")
        .arg("--delay")
        .arg("1.5")
        .arg("-E")
        .arg("CARGO_PROFILE_DEV_INCREMENTAL=true")
        .arg("-E")
        .arg("CARGO_PROFILE_DEV_DEBUG=0")
        .arg("-w")
        .arg("crates")
        .arg("-w")
        .arg("Cargo.toml")
        .arg("-w")
        .arg("Cargo.lock");

    for glob in WEB_SERVER_WATCH_IGNORE_GLOBS {
        command.arg("--ignore").arg(glob);
    }

    command.arg("-x").arg(WEB_SERVER_WATCH_COMMAND);
    command.env("CARGO_TARGET_DIR", dev_rust_target_dir(repo_root));
    command.env("NOEMA_WEB__HOST", "0.0.0.0");
}

fn dev_rust_target_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(DEV_RUST_TARGET_DIR)
}

fn spawn_bridge_watcher(repo_root: &Path) -> Result<Option<Child>, DevError> {
    if !cfg!(target_os = "macos") {
        return Ok(None);
    }

    let package_dir = repo_root.join("crates/noema-providers/apple-foundation-bridge");
    if !package_dir.exists() {
        return Ok(None);
    }

    let mut command = Command::new(cargo_exe());
    command.arg("watch").arg("-C").arg(&package_dir).args([
        "-w",
        "Package.swift",
        "-w",
        "Sources",
        "-s",
        "swift build",
    ]);

    spawn_dev_process("foundation bridge watcher", &mut command, &package_dir).map(Some)
}

fn spawn_dev_process(
    label: &'static str,
    command: &mut Command,
    current_dir: &Path,
) -> Result<Child, DevError> {
    #[cfg(unix)]
    command.process_group(0);

    strip_cargo_run_env(command);

    command
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| DevError::SpawnProcess { label, source })
}

fn strip_cargo_run_env(command: &mut Command) {
    for (key, _) in env::vars_os() {
        if let Some(key) = key.to_str()
            && (matches!(
                key,
                "CARGO_MANIFEST_DIR"
                    | "CARGO_MANIFEST_PATH"
                    | "CARGO_CRATE_NAME"
                    | "CARGO_BIN_NAME"
                    | "CARGO_PRIMARY_PACKAGE"
            ) || key.starts_with("CARGO_PKG_"))
        {
            command.env_remove(key);
        }
    }
}

async fn wait_for_child(label: &'static str, child: &mut Child) -> Result<(), DevError> {
    let status = child
        .wait()
        .await
        .map_err(|source| DevError::WaitProcess { label, source })?;
    Err(DevError::ProcessExited { label, status })
}

async fn stop_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }

    #[cfg(unix)]
    signal_process_group(child, "-TERM").await;
    if wait_for_child_exit(child, Duration::from_secs(2)).await {
        return;
    }

    #[cfg(unix)]
    signal_process_group(child, "-KILL").await;
    if wait_for_child_exit(child, Duration::from_secs(1)).await {
        return;
    }

    let _ = child.start_kill();
    let _ = child.wait().await;
}

async fn wait_for_child_exit(child: &mut Child, duration: Duration) -> bool {
    matches!(timeout(duration, child.wait()).await, Ok(Ok(_)))
}

#[cfg(unix)]
async fn signal_process_group(child: &Child, signal: &str) {
    let Some(pid) = child.id() else {
        return;
    };

    let process_group = format!("-{pid}");
    let _ = Command::new("kill")
        .arg(signal)
        .arg("--")
        .arg(process_group)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("noema-dev lives under crates/noema-dev")
        .to_path_buf()
}

fn cargo_exe() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_watcher_ignores_web_sources_and_generated_assets() {
        assert_eq!(
            WEB_SERVER_WATCH_IGNORE_GLOBS,
            ["apps/web/**", "crates/noema-server/target/web-assets/**",]
        );
    }

    #[test]
    fn server_watcher_uses_fast_development_profile() {
        let mut command = Command::new("cargo");
        configure_web_server_watcher(&mut command, Path::new("/workspace"));

        let arguments = command.as_std().get_args().collect::<Vec<_>>();
        assert!(arguments.windows(2).any(|pair| pair == ["--delay", "1.5"]));
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-E", "CARGO_PROFILE_DEV_INCREMENTAL=true"])
        );
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-E", "CARGO_PROFILE_DEV_DEBUG=0"])
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervisor_stops_watchers_when_shutdown_signal_arrives() {
        let mut web = spawn_test_watcher();
        let mut server = spawn_test_watcher();
        let mut bridge = spawn_test_watcher();

        let result = supervise_dev_processes(&mut web, &mut server, Some(&mut bridge), async {
            Ok("SIGINT")
        })
        .await;

        assert!(result.is_ok());
        assert_child_exited(&mut web).await;
        assert_child_exited(&mut server).await;
        assert_child_exited(&mut bridge).await;
    }

    #[cfg(unix)]
    fn spawn_test_watcher() -> Child {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("trap 'exit 0' TERM; while :; do sleep 1; done")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        command.spawn().expect("spawn test watcher")
    }

    #[cfg(unix)]
    async fn assert_child_exited(child: &mut Child) {
        tokio::time::timeout(Duration::from_secs(2), child.wait())
            .await
            .expect("child should exit before timeout")
            .expect("wait for child");
    }
}
