//! Standalone local web server entrypoint for Noema.

use std::path::{Path, PathBuf};

use noema_host::start_from_process_env_with_local_model_runtime_root;
use noema_server::run_daemon_web;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("failed to run Noema web server: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "dev-no-auth")]
    generate_graphql_schema()?;

    let host =
        start_from_process_env_with_local_model_runtime_root(local_model_runtime_root()).await?;
    run_daemon_web(host).await?;
    Ok(())
}

#[cfg(feature = "dev-no-auth")]
fn generate_graphql_schema() -> Result<(), std::io::Error> {
    let output_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../graphql/schema.graphql");
    let schema = noema_api::graphql::schema_sdl();
    if write_if_changed(&output_path, schema.as_bytes())? {
        eprintln!("wrote {}", output_path.display());
    }
    Ok(())
}

#[cfg(feature = "dev-no-auth")]
fn write_if_changed(path: &Path, contents: &[u8]) -> Result<bool, std::io::Error> {
    if std::fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(false);
    }
    std::fs::write(path, contents)?;
    Ok(true)
}

fn local_model_runtime_root() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../noema-desktop/binaries/runtime"))
    } else {
        None
    }
}

#[cfg(all(test, feature = "dev-no-auth"))]
mod tests {
    use super::*;

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
