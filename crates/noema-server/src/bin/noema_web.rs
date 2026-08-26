//! Standalone local web server entrypoint for Noema.

use std::path::{Path, PathBuf};

use noema_host::{
    run_private_worker_if_requested, start_from_process_env_with_local_model_runtime_root,
};
use noema_server::run_daemon_web;

fn main() {
    if let Some(status) = run_private_worker_if_requested() {
        std::process::exit(status);
    }
    if let Err(error) = run_application() {
        eprintln!("failed to run Noema web server: {error}");
        std::process::exit(1);
    }
}

fn run_application() -> Result<(), Box<dyn std::error::Error>> {
    require_unprivileged_release_user()?;
    application_runtime()?.block_on(run())
}

fn require_unprivileged_release_user() -> Result<(), std::io::Error> {
    reject_root(!cfg!(debug_assertions) && current_user_is_root())
}

#[cfg(unix)]
fn current_user_is_root() -> bool {
    rustix::process::geteuid().is_root()
}

#[cfg(not(unix))]
fn current_user_is_root() -> bool {
    false
}

fn reject_root(is_root: bool) -> Result<(), std::io::Error> {
    if is_root {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "release Noema server must not run as root",
        ))
    } else {
        Ok(())
    }
}

fn application_runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .build()
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    generate_graphql_schema()?;

    let host =
        start_from_process_env_with_local_model_runtime_root(local_model_runtime_root()).await?;
    run_daemon_web(host).await?;
    Ok(())
}

#[cfg(debug_assertions)]
fn generate_graphql_schema() -> Result<(), std::io::Error> {
    let output_path = std::env::var_os("NOEMA_DEV_SCHEMA_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../graphql/schema.graphql")
        });
    let schema = noema_api::graphql::schema_sdl();
    if write_if_changed(&output_path, schema.as_bytes())? {
        eprintln!("wrote {}", output_path.display());
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn write_if_changed(path: &Path, contents: &[u8]) -> Result<bool, std::io::Error> {
    if std::fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(false);
    }
    std::fs::write(path, contents)?;
    Ok(true)
}

fn local_model_runtime_root() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        Some(
            std::env::var_os("NOEMA_DEV_LOCAL_MODEL_RUNTIME_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../noema-desktop/binaries/runtime")
                }),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_runtime_supports_cold_iana_timezone_parsing() {
        let runtime = application_runtime().expect("application runtime");
        let next = runtime.block_on(async {
            tokio::task::spawn_blocking(|| {
                noema_tasks::next_recurrence_at_or_after(
                    "0 7 * * *",
                    "America/Los_Angeles",
                    1_786_472_647,
                )
            })
            .await
            .expect("timezone parser task")
            .expect("valid recurrence")
        });

        assert!(next >= 1_786_472_647);
    }

    #[test]
    fn release_service_identity_rejects_root() {
        assert!(reject_root(true).is_err());
        assert!(reject_root(false).is_ok());
    }

    #[test]
    fn unchanged_schema_is_not_rewritten() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("schema.graphql");
        std::fs::write(&path, b"type Query").expect("seed schema");

        assert!(!write_if_changed(&path, b"type Query").expect("compare schema"));
    }

    #[test]
    fn changed_schema_is_rewritten() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("schema.graphql");
        std::fs::write(&path, b"type Query").expect("seed schema");

        assert!(write_if_changed(&path, b"type Mutation").expect("write schema"));
        assert_eq!(std::fs::read(&path).expect("read schema"), b"type Mutation");
    }
}
