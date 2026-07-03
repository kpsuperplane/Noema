//! Standalone local web server entrypoint for Noema.

use noema_core::{Config, ConfigOverrides, DaemonWebServerConfig, run_daemon_web};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("failed to run Noema web server: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load_daemon(None, ConfigOverrides::default())?;
    eprintln!("Noema web server listening at {}", config.web.url());
    run_daemon_web(DaemonWebServerConfig::new(config.provider, config.web)).await?;
    Ok(())
}
