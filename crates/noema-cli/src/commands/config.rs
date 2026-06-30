//! `noema config` command.

use crate::{Args, CliError};
use noema_core::{NoemaHomeInitOptions, NoemaPaths, init_noema_home};

pub(crate) fn run_config(_args: &Args, force: bool) -> Result<(), CliError> {
    let paths = NoemaPaths::from_process_env()?;
    let result = init_noema_home(
        &paths,
        NoemaHomeInitOptions {
            force,
            write_config: true,
        },
    )?;

    println!("Noema directory: {}", result.root.display());
    println!("Run directory: {}", result.run_dir.display());
    if result.wrote_config {
        println!("Config file: {} (written)", result.config_path.display());
    } else {
        println!(
            "Config file: {} (already exists; use --force to rewrite)",
            result.config_path.display()
        );
    }

    Ok(())
}
