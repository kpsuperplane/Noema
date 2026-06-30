//! `noema start` command.

use crate::{Args, CliError, cli_overrides};
use noema_core::{
    Config, DaemonServerConfig, NoemaHomeInitOptions, NoemaPaths, init_noema_home, run_daemon,
};

pub(crate) async fn run_start(args: &Args) -> Result<(), CliError> {
    let paths = ensure_noema_home_for_start(args)?;
    let daemon_config = Config::load_daemon(args.config.clone(), cli_overrides(args))?;
    let socket_path = paths.socket_path();
    eprintln!("noema daemon listening at {}", socket_path.display());
    eprintln!("noema web chat available at {}", daemon_config.web.url());
    run_daemon(DaemonServerConfig::new(
        socket_path,
        daemon_config.codex,
        daemon_config.web,
    ))
    .await?;
    Ok(())
}

fn ensure_noema_home_for_start(args: &Args) -> Result<NoemaPaths, CliError> {
    let paths = NoemaPaths::from_process_env()?;
    let should_write_config = args.config.is_none() && !paths.config_exists();
    let should_initialize = !paths.exists() || should_write_config;

    if should_initialize {
        eprintln!(
            "noema directory is not initialized at {}; running `noema config` defaults.",
            paths.root().display()
        );
        let result = init_noema_home(
            &paths,
            NoemaHomeInitOptions {
                force: false,
                write_config: should_write_config,
            },
        )?;

        if result.wrote_config {
            eprintln!("wrote default config at {}", result.config_path.display());
        } else {
            eprintln!("prepared noema directory at {}", result.root.display());
        }
    }

    Ok(paths)
}
