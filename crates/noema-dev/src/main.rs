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
const DEV_ASSET_DIR_ENV: &str = "NOEMA_DEV_ASSET_DIR";
const ROOT_DEV_ASSET_DIR: &str = "/run/noema-dev/web-assets";
const WEB_SERVER_WATCH_IGNORE_GLOBS: [&str; 2] =
    ["apps/web/**", "crates/noema-server/target/web-assets/**"];
const WATCHER_RESTART_DELAY: Duration = Duration::from_millis(250);

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

    #[error("failed to locate the Noema development executable: {source}")]
    LocateExecutable { source: io::Error },

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
        Some(mode) if mode == "serve" => {
            workflow::run_development_server().await.map_err(Into::into)
        }
        Some(mode) if mode == "validate" => workflow::run_validation(args.collect())
            .await
            .map_err(Into::into),
        Some(mode) => Err(DevError::UnknownMode { mode }),
    }
}

async fn run_development() -> Result<(), DevError> {
    let repo_root = repo_root();
    let web_dir = repo_root.join("apps/web");

    let mut web = spawn_web_watcher(&web_dir)?;
    let mut server = spawn_web_server_watcher(&repo_root)?;
    let mut bridge = spawn_bridge_watcher(&repo_root)?;

    eprintln!("Noema dev supervisor started");
    eprintln!("web assets: bun run {WEB_ASSET_WATCH_SCRIPT}");
    eprintln!("web server: cargo watch -- noema-dev serve");
    if bridge.is_some() {
        eprintln!("foundation bridge: cargo watch -s swift build");
    }

    supervise_dev_processes(
        &repo_root,
        &web_dir,
        &mut web,
        &mut server,
        &mut bridge,
        shutdown_signal(),
    )
    .await
}

async fn supervise_dev_processes<S>(
    repo_root: &Path,
    web_dir: &Path,
    web: &mut Child,
    server: &mut Child,
    bridge: &mut Option<Child>,
    shutdown_signal: S,
) -> Result<(), DevError>
where
    S: Future<Output = Result<&'static str, DevError>>,
{
    tokio::pin!(shutdown_signal);
    loop {
        let result = tokio::select! {
            result = wait_for_child("web asset watcher", web) => result,
            result = wait_for_child("web server watcher", server) => result,
            result = async {
                match bridge.as_mut() {
                    Some(child) => wait_for_child("foundation bridge watcher", child).await,
                    None => std::future::pending().await,
                }
            } => result,
            result = &mut shutdown_signal => {
                match result {
                    Ok(signal) => {
                        eprintln!("received {signal}; stopping Noema dev supervisor");
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            }
        };

        match result {
            Ok(()) => {
                stop_dev_processes(web, server, bridge).await;
                return Ok(());
            }
            Err(error) => {
                let Some(label) = watcher_exit_label(&error) else {
                    stop_dev_processes(web, server, bridge).await;
                    return Err(error);
                };

                eprintln!("{error}; retrying {label}");
                tokio::time::sleep(WATCHER_RESTART_DELAY).await;

                let restart = match label {
                    "web asset watcher" => spawn_web_watcher(web_dir).map(|child| *web = child),
                    "web server watcher" => {
                        spawn_web_server_watcher(repo_root).map(|child| *server = child)
                    }
                    "foundation bridge watcher" => {
                        spawn_bridge_watcher(repo_root).map(|child| *bridge = child)
                    }
                    _ => unreachable!("unknown development watcher label: {label}"),
                };
                if let Err(error) = restart {
                    stop_dev_processes(web, server, bridge).await;
                    return Err(error);
                }
            }
        }
    }
}

fn watcher_exit_label(error: &DevError) -> Option<&'static str> {
    match error {
        DevError::ProcessExited { label, .. } => Some(label),
        _ => None,
    }
}

async fn stop_dev_processes(web: &mut Child, server: &mut Child, bridge: &mut Option<Child>) {
    stop_child(web).await;
    stop_child(server).await;
    if let Some(bridge) = bridge.as_mut() {
        stop_child(bridge).await;
    }
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
    if running_as_root() {
        command.env(DEV_ASSET_DIR_ENV, ROOT_DEV_ASSET_DIR);
    }

    spawn_dev_process("web asset watcher", &mut command, web_dir)
}

fn spawn_web_server_watcher(repo_root: &Path) -> Result<Child, DevError> {
    let executable = env::current_exe().map_err(|source| DevError::LocateExecutable { source })?;
    let mut command = Command::new(cargo_exe());
    configure_web_server_watcher(&mut command, &executable);

    spawn_dev_process("web server watcher", &mut command, repo_root)
}

fn configure_web_server_watcher(command: &mut Command, executable: &Path) {
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

    command.arg("--").arg(executable).arg("serve");
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
    // Let nested Cargo apply the repository's native compiler wrappers itself;
    // inherited resolved wrappers make sccache fail its rustc probe.
    command.env_remove("CC").env_remove("CXX");
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

#[cfg(target_os = "linux")]
fn running_as_root() -> bool {
    use std::os::unix::fs::MetadataExt;

    std::fs::metadata("/proc/self").is_ok_and(|metadata| metadata.uid() == 0)
}

#[cfg(not(target_os = "linux"))]
fn running_as_root() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_cargo_does_not_inherit_native_compiler_wrappers() {
        let mut command = Command::new("cargo");
        strip_cargo_run_env(&mut command);

        for key in ["CC", "CXX"] {
            assert!(
                command
                    .as_std()
                    .get_envs()
                    .any(|(candidate, value)| candidate == key && value.is_none()),
                "{key} should be removed"
            );
        }
    }

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
        configure_web_server_watcher(&mut command, Path::new("/workspace/noema-dev"));

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

    #[test]
    fn server_watcher_runs_the_budgeted_server_command() {
        let mut command = Command::new("cargo");
        configure_web_server_watcher(&mut command, Path::new("/workspace/noema-dev"));

        let arguments = command.as_std().get_args().collect::<Vec<_>>();
        assert!(
            arguments
                .windows(3)
                .any(|pair| pair == ["--", "/workspace/noema-dev", "serve"])
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervisor_stops_watchers_when_shutdown_signal_arrives() {
        let mut web = spawn_test_watcher();
        let mut server = spawn_test_watcher();
        let mut bridge = Some(spawn_test_watcher());

        let result = supervise_dev_processes(
            Path::new("/workspace"),
            Path::new("/workspace/apps/web"),
            &mut web,
            &mut server,
            &mut bridge,
            async { Ok("SIGINT") },
        )
        .await;

        assert!(result.is_ok());
        assert_child_exited(&mut web).await;
        assert_child_exited(&mut server).await;
        assert_child_exited(bridge.as_mut().expect("bridge watcher")).await;
    }

    #[cfg(unix)]
    #[test]
    fn only_watcher_exit_requests_a_restart() {
        use std::os::unix::process::ExitStatusExt;

        let exited = DevError::ProcessExited {
            label: "web asset watcher",
            status: ExitStatus::from_raw(1),
        };
        assert_eq!(watcher_exit_label(&exited), Some("web asset watcher"));

        let unknown = DevError::UnknownMode {
            mode: OsString::from("unknown"),
        };
        assert_eq!(watcher_exit_label(&unknown), None);
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
