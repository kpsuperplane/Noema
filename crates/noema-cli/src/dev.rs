//! Development workflow helpers.

use std::{
    env,
    future::Future,
    io,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
};
use thiserror::Error;
use tokio::{
    process::{Child, Command},
    time::{Duration, sleep},
};

#[derive(Debug, Clone)]
pub(crate) struct DevDaemonOptions {
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) config: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub(crate) enum DevDaemonError {
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

    #[error("failed to generate GraphQL schema: {source}")]
    GenerateSchema { source: io::Error },

    #[error("failed to install dev-daemon shutdown signal handler: {source}")]
    ShutdownSignal { source: io::Error },
}

/// `package.json` script that watches and rebuilds web assets without invoking
/// cargo (type generation is handled separately to avoid a concurrent build).
const WEB_ASSET_WATCH_SCRIPT: &str = "dev:assets";

pub(crate) async fn run_dev_daemon(options: DevDaemonOptions) -> Result<(), DevDaemonError> {
    let repo_root = repo_root();
    let web_dir = repo_root.join("crates/noema-core/web");

    // Generate the GraphQL schema in-process before starting the watchers.
    // Doing it here (rather than via a separate `cargo run` of the noema-core
    // export bin) keeps every cargo invocation in this flow on the same
    // noema-cli package graph, so shared dependencies are compiled once.
    generate_graphql_schema(&repo_root)?;

    let mut web = spawn_web_watcher(&web_dir)?;
    let mut daemon = spawn_daemon_watcher(&repo_root, &options)?;
    let mut bridge = spawn_bridge_watcher(&repo_root)?;

    eprintln!("noema dev daemon started");
    eprintln!("web assets: bun run dev");
    eprintln!("daemon: cargo watch -x {}", daemon_start_command(options));
    if bridge.is_some() {
        eprintln!("foundation bridge: cargo watch -s swift build");
    }

    supervise_dev_processes(&mut web, &mut daemon, bridge.as_mut(), shutdown_signal()).await
}

async fn supervise_dev_processes<S>(
    web: &mut Child,
    daemon: &mut Child,
    bridge: Option<&mut Child>,
    shutdown_signal: S,
) -> Result<(), DevDaemonError>
where
    S: Future<Output = Result<&'static str, DevDaemonError>>,
{
    let mut bridge = bridge;
    let result = tokio::select! {
        result = wait_for_child("web asset watcher", web) => result,
        result = wait_for_child("daemon watcher", daemon) => result,
        result = wait_for_optional_child("foundation bridge watcher", bridge.as_deref_mut()) => {
            result
        }
        result = shutdown_signal => {
            match result {
                Ok(signal) => {
                    eprintln!("received {signal}; stopping noema dev daemon");
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
    };

    stop_child(web).await;
    stop_child(daemon).await;
    stop_optional_child(bridge).await;
    result
}

#[cfg(unix)]
async fn shutdown_signal() -> Result<&'static str, DevDaemonError> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|source| DevDaemonError::ShutdownSignal { source })?;
    let mut terminate = signal(SignalKind::terminate())
        .map_err(|source| DevDaemonError::ShutdownSignal { source })?;
    let mut hangup =
        signal(SignalKind::hangup()).map_err(|source| DevDaemonError::ShutdownSignal { source })?;

    tokio::select! {
        _ = interrupt.recv() => Ok("SIGINT"),
        _ = terminate.recv() => Ok("SIGTERM"),
        _ = hangup.recv() => Ok("SIGHUP"),
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<&'static str, DevDaemonError> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|source| DevDaemonError::ShutdownSignal { source })?;
    Ok("Ctrl-C")
}

fn spawn_web_watcher(web_dir: &Path) -> Result<Child, DevDaemonError> {
    let mut command = Command::new("bun");
    command.arg("run").arg(WEB_ASSET_WATCH_SCRIPT);

    spawn_dev_process("web asset watcher", &mut command, web_dir)
}

fn spawn_bridge_watcher(repo_root: &Path) -> Result<Option<Child>, DevDaemonError> {
    if !foundation_bridge_watcher_enabled() {
        return Ok(None);
    }

    let package_dir = foundation_bridge_package_dir(repo_root);
    if !package_dir.exists() {
        return Ok(None);
    }

    let mut command = Command::new(cargo_exe());
    command.args(foundation_bridge_watch_args(&package_dir));

    spawn_dev_process("foundation bridge watcher", &mut command, &package_dir).map(Some)
}

fn foundation_bridge_watcher_enabled() -> bool {
    cfg!(target_os = "macos")
}

fn foundation_bridge_package_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("crates/noema-core/apple-foundation-bridge")
}

fn foundation_bridge_watch_args(package_dir: &Path) -> Vec<String> {
    vec![
        "watch".to_string(),
        "-C".to_string(),
        package_dir.to_string_lossy().to_string(),
        "-w".to_string(),
        "Package.swift".to_string(),
        "-w".to_string(),
        "Sources".to_string(),
        "-s".to_string(),
        "swift build".to_string(),
    ]
}

fn graphql_schema_output_path(repo_root: &Path) -> PathBuf {
    repo_root.join("crates/noema-core/web/src/generated/schema.graphql")
}

fn generate_graphql_schema(repo_root: &Path) -> Result<(), DevDaemonError> {
    let output_path = graphql_schema_output_path(repo_root);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|source| DevDaemonError::GenerateSchema { source })?;
    }
    let schema = noema_core::graphql::build_schema(noema_core::graphql::GraphqlState::for_tests());
    std::fs::write(&output_path, schema.sdl())
        .map_err(|source| DevDaemonError::GenerateSchema { source })?;
    eprintln!("wrote {}", output_path.display());
    Ok(())
}

fn spawn_daemon_watcher(
    repo_root: &Path,
    options: &DevDaemonOptions,
) -> Result<Child, DevDaemonError> {
    let mut command = Command::new(cargo_exe());
    command
        .arg("watch")
        .arg("-w")
        .arg("crates")
        .arg("-w")
        .arg("Cargo.toml")
        .arg("-w")
        .arg("Cargo.lock");

    // Ignore non-Rust paths that the type generator and vite rewrite constantly.
    // Without this, those writes retrigger the watcher mid-build, interrupting
    // compilation and thrashing cargo's fingerprints into redundant rebuilds.
    for glob in daemon_watch_ignore_globs() {
        command.arg("--ignore").arg(glob);
    }

    command.arg("-x").arg(daemon_start_command(options.clone()));

    spawn_dev_process("daemon watcher", &mut command, repo_root)
}

fn daemon_watch_ignore_globs() -> [&'static str; 2] {
    [
        // Frontend sources and generated TypeScript: never Rust build inputs.
        "crates/noema-core/web/**",
        // Vite asset output: served from disk in debug builds, so changes here
        // must not retrigger a daemon rebuild.
        "crates/noema-core/src/daemon/web/assets/**",
    ]
}

fn spawn_dev_process(
    label: &'static str,
    command: &mut Command,
    current_dir: &Path,
) -> Result<Child, DevDaemonError> {
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
        .map_err(|source| DevDaemonError::SpawnProcess { label, source })
}

/// Remove the `CARGO_*` variables that `cargo run` injects into this process's
/// environment before spawning a child that itself runs cargo.
///
/// `cargo dev-daemon` runs us via `cargo run`, which exports `CARGO_MANIFEST_DIR`
/// and friends. If those leak into the watched `cargo run -- start`, cargo sees a
/// changed build-script environment and rebuilds every dependency with a
/// `build.rs` (ring, rustls, and similar crates) on each invocation.
fn strip_cargo_run_env(command: &mut Command) {
    for (key, _) in env::vars_os() {
        if let Some(key) = key.to_str()
            && is_cargo_run_injected_env(key)
        {
            command.env_remove(key);
        }
    }
}

fn is_cargo_run_injected_env(key: &str) -> bool {
    matches!(
        key,
        "CARGO_MANIFEST_DIR"
            | "CARGO_MANIFEST_PATH"
            | "CARGO_CRATE_NAME"
            | "CARGO_BIN_NAME"
            | "CARGO_PRIMARY_PACKAGE"
    ) || key.starts_with("CARGO_PKG_")
}

async fn wait_for_child(label: &'static str, child: &mut Child) -> Result<(), DevDaemonError> {
    let status = child
        .wait()
        .await
        .map_err(|source| DevDaemonError::WaitProcess { label, source })?;
    Err(DevDaemonError::ProcessExited { label, status })
}

async fn wait_for_optional_child(
    label: &'static str,
    child: Option<&mut Child>,
) -> Result<(), DevDaemonError> {
    match child {
        Some(child) => wait_for_child(label, child).await,
        None => std::future::pending().await,
    }
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

async fn stop_optional_child(child: Option<&mut Child>) {
    if let Some(child) = child {
        stop_child(child).await;
    }
}

async fn wait_for_child_exit(child: &mut Child, duration: Duration) -> bool {
    let mut elapsed = Duration::ZERO;
    let tick = Duration::from_millis(50);

    while elapsed < duration {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return true;
        }

        sleep(tick).await;
        elapsed += tick;
    }

    matches!(child.try_wait(), Ok(Some(_)))
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
        .expect("noema-cli lives under crates/noema-cli")
        .to_path_buf()
}

fn cargo_exe() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn daemon_start_command(options: DevDaemonOptions) -> String {
    let mut args = vec![
        "run".to_string(),
        "-p".to_string(),
        "noema-cli".to_string(),
        "--".to_string(),
    ];

    if let Some(provider) = options.provider {
        args.extend(["--provider".to_string(), shell_quote(&provider)]);
    }
    if let Some(model) = options.model {
        args.extend(["--model".to_string(), shell_quote(&model)]);
    }
    if let Some(base_url) = options.base_url {
        args.extend(["--base-url".to_string(), shell_quote(&base_url)]);
    }
    if let Some(config) = options.config {
        args.extend([
            "--config".to_string(),
            shell_quote(&config.to_string_lossy()),
        ]);
    }

    args.push("start".to_string());
    args.join(" ")
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/._-:=@".contains(character))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_asset_watcher_skips_type_generation() {
        // The web watcher must run vite only; type generation happens once up
        // front so it never races the daemon build on the shared target dir.
        assert_eq!(WEB_ASSET_WATCH_SCRIPT, "dev:assets");
    }

    #[test]
    fn strips_cargo_run_injected_env_only() {
        assert!(is_cargo_run_injected_env("CARGO_MANIFEST_DIR"));
        assert!(is_cargo_run_injected_env("CARGO_PKG_VERSION"));
        assert!(is_cargo_run_injected_env("CARGO_BIN_NAME"));
        // Must keep cargo's own location and the user's real environment.
        assert!(!is_cargo_run_injected_env("CARGO"));
        assert!(!is_cargo_run_injected_env("CARGO_HOME"));
        assert!(!is_cargo_run_injected_env("NOEMA_HOME"));
        assert!(!is_cargo_run_injected_env("PATH"));
    }

    #[test]
    fn daemon_watcher_ignores_generated_and_asset_paths() {
        assert_eq!(
            daemon_watch_ignore_globs(),
            [
                "crates/noema-core/web/**",
                "crates/noema-core/src/daemon/web/assets/**",
            ]
        );
    }

    #[test]
    fn graphql_schema_output_path_targets_web_generated_dir() {
        assert_eq!(
            graphql_schema_output_path(Path::new("/workspace")),
            PathBuf::from("/workspace/crates/noema-core/web/src/generated/schema.graphql")
        );
    }

    #[test]
    fn foundation_bridge_watcher_uses_swift_package_sources() {
        assert_eq!(
            foundation_bridge_package_dir(Path::new("/workspace")),
            PathBuf::from("/workspace/crates/noema-core/apple-foundation-bridge")
        );
        assert_eq!(
            foundation_bridge_watch_args(Path::new(
                "/workspace/crates/noema-core/apple-foundation-bridge"
            )),
            vec![
                "watch",
                "-C",
                "/workspace/crates/noema-core/apple-foundation-bridge",
                "-w",
                "Package.swift",
                "-w",
                "Sources",
                "-s",
                "swift build",
            ]
        );
        assert_eq!(
            foundation_bridge_watcher_enabled(),
            cfg!(target_os = "macos")
        );
    }

    #[test]
    fn daemon_start_command_defaults_to_start() {
        let command = daemon_start_command(DevDaemonOptions {
            provider: None,
            model: None,
            base_url: None,
            config: None,
        });

        assert_eq!(command, "run -p noema-cli -- start");
    }

    #[test]
    fn daemon_start_command_forwards_global_options() {
        let command = daemon_start_command(DevDaemonOptions {
            provider: Some("codex".to_string()),
            model: Some("gpt test".to_string()),
            base_url: Some("http://localhost:1234".to_string()),
            config: Some(PathBuf::from("/tmp/noema config.yaml")),
        });

        assert_eq!(
            command,
            "run -p noema-cli -- --provider codex --model 'gpt test' --base-url http://localhost:1234 --config '/tmp/noema config.yaml' start"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervisor_stops_both_watchers_when_shutdown_signal_arrives() {
        let mut web = spawn_test_watcher();
        let mut daemon = spawn_test_watcher();

        let result =
            supervise_dev_processes(&mut web, &mut daemon, None, async { Ok("SIGINT") }).await;

        assert!(result.is_ok());
        assert_child_exited(&mut web).await;
        assert_child_exited(&mut daemon).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervisor_stops_bridge_watcher_when_shutdown_signal_arrives() {
        let mut web = spawn_test_watcher();
        let mut daemon = spawn_test_watcher();
        let mut bridge = spawn_test_watcher();

        let result = supervise_dev_processes(&mut web, &mut daemon, Some(&mut bridge), async {
            Ok("SIGINT")
        })
        .await;

        assert!(result.is_ok());
        assert_child_exited(&mut web).await;
        assert_child_exited(&mut daemon).await;
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
