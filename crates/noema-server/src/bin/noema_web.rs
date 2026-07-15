//! Standalone local web server entrypoint for Noema.

use std::path::{Path, PathBuf};

use noema_core::{Config, ConfigOverrides, NoemaHomeInitOptions, NoemaPaths, init_noema_home};
use noema_server::run_daemon_web;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("failed to run Noema web server: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let paths = NoemaPaths::from_process_env()?;
    init_noema_home(
        &paths,
        NoemaHomeInitOptions {
            force: false,
            write_config: !paths.config_exists(),
        },
    )?;

    let config = Config::load_daemon(None, ConfigOverrides::default())?;
    run_daemon_web(config.provider, config.web, local_model_runtime_root()).await?;
    Ok(())
}

fn local_model_runtime_root() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        Some(development_local_model_runtime_root(Path::new(env!(
            "CARGO_MANIFEST_DIR"
        ))))
    } else {
        None
    }
}

fn development_local_model_runtime_root(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join("../noema-desktop/binaries/runtime")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_runtime_root_targets_prepared_desktop_assets() {
        assert_eq!(
            development_local_model_runtime_root(Path::new("/workspace/crates/noema-server")),
            PathBuf::from("/workspace/crates/noema-server/../noema-desktop/binaries/runtime")
        );
    }
}
