//! Standalone local web server entrypoint for Noema.

use noema_core::{Config, ConfigOverrides, NoemaHomeInitOptions, NoemaPaths, init_noema_home};
use noema_server::{DaemonWebServerConfig, run_daemon_web};

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
    run_daemon_web(DaemonWebServerConfig::new(config.provider, config.web)).await?;
    Ok(())
}
