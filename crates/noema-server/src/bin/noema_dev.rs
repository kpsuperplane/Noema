//! Local development supervisor for Noema web mode.

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

const WEB_ASSET_WATCH_SCRIPT: &str = "dev:assets";
const DEV_RUST_TARGET_DIR: &str = "target/noema-dev";
const MNEMOSYNE_INSTALL_STAMP_FILE: &str = ".noema-install.stamp";

#[derive(Debug, Error)]
enum DevError {
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

    #[error("failed to install dev Mnemosyne sidecar: {source}")]
    InstallMnemosyne { source: io::Error },

    #[error("dev Mnemosyne sidecar installer exited with status {status}")]
    MnemosyneInstallerExited { status: ExitStatus },

    #[error("Python 3.10 or newer is required for the Mnemosyne sidecar")]
    MissingMnemosynePython,

    #[error("failed to install dev shutdown signal handler: {source}")]
    ShutdownSignal { source: io::Error },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("failed to run Noema dev supervisor: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), DevError> {
    let repo_root = repo_root();
    let web_dir = repo_root.join("crates/noema-core/web");

    generate_graphql_schema(&repo_root)?;
    let mnemosyne_sidecar_command = ensure_dev_mnemosyne_sidecar(&repo_root).await?;

    let mut web = spawn_web_watcher(&web_dir)?;
    let mut server = spawn_web_server_watcher(&repo_root, mnemosyne_sidecar_command.as_deref())?;
    let mut bridge = spawn_bridge_watcher(&repo_root)?;

    eprintln!("Noema dev supervisor started");
    eprintln!("web assets: bun run {WEB_ASSET_WATCH_SCRIPT}");
    eprintln!("web server: cargo watch -x {}", web_server_watch_command());
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
        result = wait_for_optional_child("foundation bridge watcher", bridge.as_deref_mut()) => {
            result
        }
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
    stop_optional_child(bridge).await;
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

fn spawn_web_server_watcher(
    repo_root: &Path,
    mnemosyne_sidecar_command: Option<&str>,
) -> Result<Child, DevError> {
    let mut command = Command::new(cargo_exe());
    command
        .arg("watch")
        .arg("-w")
        .arg("crates")
        .arg("-w")
        .arg("Cargo.toml")
        .arg("-w")
        .arg("Cargo.lock");

    for glob in web_server_watch_ignore_globs() {
        command.arg("--ignore").arg(glob);
    }

    command.arg("-x").arg(web_server_watch_command());
    command.env("CARGO_TARGET_DIR", dev_rust_target_dir(repo_root));
    command.env("NOEMA_WEB__HOST", "0.0.0.0");
    if let Some(mnemosyne_sidecar_command) = mnemosyne_sidecar_command {
        command.env(
            noema_core::mnemosyne::NOEMA_MNEMOSYNE_SIDECAR_COMMAND_ENV,
            mnemosyne_sidecar_command,
        );
    }

    spawn_dev_process("web server watcher", &mut command, repo_root)
}

fn web_server_watch_command() -> &'static str {
    "run -p noema-server --bin noema_web --features dev-no-auth"
}

fn dev_rust_target_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(DEV_RUST_TARGET_DIR)
}

fn web_server_watch_ignore_globs() -> [&'static str; 2] {
    [
        "crates/noema-core/web/**",
        "crates/noema-server/target/web-assets/**",
    ]
}

fn spawn_bridge_watcher(repo_root: &Path) -> Result<Option<Child>, DevError> {
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
    repo_root.join("crates/noema-providers/apple-foundation-bridge")
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

fn generate_graphql_schema(repo_root: &Path) -> Result<(), DevError> {
    let output_path = graphql_schema_output_path(repo_root);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| DevError::GenerateSchema { source })?;
    }

    let schema = noema_core::graphql::build_schema(noema_core::graphql::GraphqlState::for_tests());
    std::fs::write(&output_path, schema.sdl())
        .map_err(|source| DevError::GenerateSchema { source })?;
    eprintln!("wrote {}", output_path.display());
    Ok(())
}

async fn ensure_dev_mnemosyne_sidecar(repo_root: &Path) -> Result<Option<String>, DevError> {
    if env::var_os(noema_core::mnemosyne::NOEMA_MNEMOSYNE_SIDECAR_COMMAND_ENV).is_some() {
        eprintln!(
            "using {} override for managed Mnemosyne",
            noema_core::mnemosyne::NOEMA_MNEMOSYNE_SIDECAR_COMMAND_ENV
        );
        return Ok(None);
    }

    let python = mnemosyne_venv_python(repo_root);
    if !executable_exists(&python) {
        let base_python = mnemosyne_base_python().await?;
        eprintln!(
            "creating dev Mnemosyne sidecar environment at {}",
            mnemosyne_sidecar_venv_dir(repo_root).display()
        );
        let status = mnemosyne_venv_create_command(repo_root, &base_python)
            .status()
            .await
            .map_err(|source| DevError::InstallMnemosyne { source })?;
        if !status.success() {
            return Err(DevError::MnemosyneInstallerExited { status });
        }
    }

    if mnemosyne_install_is_current(repo_root)? {
        eprintln!(
            "reusing dev Mnemosyne sidecar package in {}",
            mnemosyne_sidecar_venv_dir(repo_root).display()
        );
    } else {
        eprintln!(
            "installing dev Mnemosyne sidecar package into {}",
            mnemosyne_sidecar_venv_dir(repo_root).display()
        );
        let status = mnemosyne_install_command(repo_root)
            .status()
            .await
            .map_err(|source| DevError::InstallMnemosyne { source })?;
        if !status.success() {
            return Err(DevError::MnemosyneInstallerExited { status });
        }
        mark_mnemosyne_install_current(repo_root)?;
    }
    let command = mnemosyne_sidecar_uvicorn_command(&python);
    eprintln!("dev Mnemosyne sidecar: {}", python.display());
    Ok(Some(command))
}

fn mnemosyne_sidecar_uvicorn_command(python: &Path) -> String {
    format!(
        "{} -m uvicorn --factory noema_mnemosyne_sidecar.app:create_app --host 127.0.0.1 --port \"$NOEMA_MNEMOSYNE_PORT\"",
        shell_quote(python)
    )
}

fn mnemosyne_install_stamp_path(repo_root: &Path) -> PathBuf {
    mnemosyne_sidecar_venv_dir(repo_root).join(MNEMOSYNE_INSTALL_STAMP_FILE)
}

fn mnemosyne_install_is_current(repo_root: &Path) -> Result<bool, DevError> {
    let Ok(stamp) = std::fs::metadata(mnemosyne_install_stamp_path(repo_root)) else {
        return Ok(false);
    };
    let manifest =
        std::fs::metadata(mnemosyne_sidecar_source_dir(repo_root).join("pyproject.toml"))
            .map_err(|source| DevError::InstallMnemosyne { source })?;
    let stamp_modified = stamp
        .modified()
        .map_err(|source| DevError::InstallMnemosyne { source })?;
    let manifest_modified = manifest
        .modified()
        .map_err(|source| DevError::InstallMnemosyne { source })?;
    Ok(stamp_modified >= manifest_modified)
}

fn mark_mnemosyne_install_current(repo_root: &Path) -> Result<(), DevError> {
    std::fs::write(mnemosyne_install_stamp_path(repo_root), b"editable")
        .map_err(|source| DevError::InstallMnemosyne { source })
}

async fn mnemosyne_base_python() -> Result<PathBuf, DevError> {
    for candidate in mnemosyne_python_candidates() {
        if !executable_exists(&candidate) {
            continue;
        }
        let status = Command::new(&candidate)
            .arg("-c")
            .arg("import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|source| DevError::InstallMnemosyne { source })?;
        if status.success() {
            return Ok(candidate);
        }
    }
    Err(DevError::MissingMnemosynePython)
}

fn mnemosyne_python_candidates() -> Vec<PathBuf> {
    [
        "python3.13",
        "python3.12",
        "python3.11",
        "python3.10",
        "/opt/homebrew/bin/python3.13",
        "/opt/homebrew/bin/python3.12",
        "/opt/homebrew/bin/python3.11",
        "/opt/homebrew/bin/python3.10",
        "/usr/local/bin/python3.13",
        "/usr/local/bin/python3.12",
        "/usr/local/bin/python3.11",
        "/usr/local/bin/python3.10",
        "python3",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn mnemosyne_venv_create_command(repo_root: &Path, base_python: &Path) -> Command {
    let mut command = Command::new(base_python);
    command
        .arg("-m")
        .arg("venv")
        .arg(mnemosyne_sidecar_venv_dir(repo_root))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn mnemosyne_install_command(repo_root: &Path) -> Command {
    let mut command = Command::new(mnemosyne_venv_python(repo_root));
    command
        .args(mnemosyne_install_args(repo_root))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn mnemosyne_install_args(repo_root: &Path) -> Vec<String> {
    vec![
        "-m".to_string(),
        "pip".to_string(),
        "install".to_string(),
        "--editable".to_string(),
        mnemosyne_sidecar_source_dir(repo_root)
            .display()
            .to_string(),
    ]
}

fn mnemosyne_sidecar_source_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("crates/noema-core/mnemosyne-sidecar")
}

fn mnemosyne_sidecar_venv_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("crates/noema-core/target/mnemosyne-sidecar-venv")
}

fn mnemosyne_venv_python(repo_root: &Path) -> PathBuf {
    if cfg!(windows) {
        mnemosyne_sidecar_venv_dir(repo_root).join("Scripts/python.exe")
    } else {
        mnemosyne_sidecar_venv_dir(repo_root).join("bin/python")
    }
}

fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn executable_exists(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
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

async fn wait_for_child(label: &'static str, child: &mut Child) -> Result<(), DevError> {
    let status = child
        .wait()
        .await
        .map_err(|source| DevError::WaitProcess { label, source })?;
    Err(DevError::ProcessExited { label, status })
}

async fn wait_for_optional_child(
    label: &'static str,
    child: Option<&mut Child>,
) -> Result<(), DevError> {
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
        .expect("noema-core lives under crates/noema-core")
        .to_path_buf()
}

fn cargo_exe() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_watch_runs_noema_web() {
        assert_eq!(
            web_server_watch_command(),
            "run -p noema-server --bin noema_web --features dev-no-auth"
        );
    }

    #[test]
    fn web_asset_watcher_skips_schema_generation() {
        assert_eq!(WEB_ASSET_WATCH_SCRIPT, "dev:assets");
    }

    #[test]
    fn server_watcher_ignores_web_sources_and_generated_assets() {
        assert_eq!(
            web_server_watch_ignore_globs(),
            [
                "crates/noema-core/web/**",
                "crates/noema-server/target/web-assets/**",
            ]
        );
    }

    #[test]
    fn server_watcher_uses_an_isolated_rust_target_dir() {
        assert_eq!(
            dev_rust_target_dir(Path::new("/workspace")),
            PathBuf::from("/workspace/target/noema-dev")
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
    fn mnemosyne_dev_paths_target_generated_venv() {
        assert_eq!(
            mnemosyne_sidecar_source_dir(Path::new("/workspace")),
            PathBuf::from("/workspace/crates/noema-core/mnemosyne-sidecar")
        );
        assert_eq!(
            mnemosyne_sidecar_venv_dir(Path::new("/workspace")),
            PathBuf::from("/workspace/crates/noema-core/target/mnemosyne-sidecar-venv")
        );
        assert_eq!(
            mnemosyne_venv_python(Path::new("/workspace")),
            PathBuf::from("/workspace/crates/noema-core/target/mnemosyne-sidecar-venv/bin/python")
        );
        assert_eq!(
            mnemosyne_install_stamp_path(Path::new("/workspace")),
            PathBuf::from(
                "/workspace/crates/noema-core/target/mnemosyne-sidecar-venv/.noema-install.stamp"
            )
        );
    }

    #[test]
    fn mnemosyne_dev_sidecar_command_uses_asgi_factory() {
        assert_eq!(
            mnemosyne_sidecar_uvicorn_command(Path::new("/workspace/.venv/bin/python")),
            "'/workspace/.venv/bin/python' -m uvicorn --factory noema_mnemosyne_sidecar.app:create_app --host 127.0.0.1 --port \"$NOEMA_MNEMOSYNE_PORT\""
        );
    }

    #[test]
    fn mnemosyne_dev_install_uses_editable_source_package() {
        assert_eq!(
            mnemosyne_install_args(Path::new("/workspace")),
            vec![
                "-m",
                "pip",
                "install",
                "--editable",
                "/workspace/crates/noema-core/mnemosyne-sidecar"
            ]
        );
    }

    #[test]
    fn foundation_bridge_watcher_uses_swift_package_sources() {
        assert_eq!(
            foundation_bridge_package_dir(Path::new("/workspace")),
            PathBuf::from("/workspace/crates/noema-providers/apple-foundation-bridge")
        );
        assert_eq!(
            foundation_bridge_watch_args(Path::new(
                "/workspace/crates/noema-providers/apple-foundation-bridge"
            )),
            vec![
                "watch",
                "-C",
                "/workspace/crates/noema-providers/apple-foundation-bridge",
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
    fn strips_cargo_run_injected_env_only() {
        assert!(is_cargo_run_injected_env("CARGO_MANIFEST_DIR"));
        assert!(is_cargo_run_injected_env("CARGO_PKG_VERSION"));
        assert!(is_cargo_run_injected_env("CARGO_BIN_NAME"));
        assert!(!is_cargo_run_injected_env("CARGO"));
        assert!(!is_cargo_run_injected_env("CARGO_HOME"));
        assert!(!is_cargo_run_injected_env("NOEMA_HOME"));
        assert!(!is_cargo_run_injected_env("PATH"));
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
