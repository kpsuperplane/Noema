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
    let output_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/src/generated/schema.graphql");
    std::fs::write(&output_path, noema_api::graphql::schema_sdl())?;
    eprintln!("wrote {}", output_path.display());
    Ok(())
}

fn local_model_runtime_root() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../noema-desktop/binaries/runtime"))
    } else {
        None
    }
}
