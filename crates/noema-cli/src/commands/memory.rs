//! `noema memory` command.

use crate::{
    Args, CliError, cli_overrides,
    commands::ConnectedDaemon,
    graphql_client::validate_graphql_base_url,
    inspection::{MemoryCommand, run_memory},
};
use noema_core::Config;

pub(crate) async fn run_memory_graphql(
    args: &Args,
    command: &MemoryCommand,
) -> Result<(), CliError> {
    let mut daemon = ConnectedDaemon::connect_or_start(args).await?;
    let daemon_config = Config::load_daemon(args.config.clone(), cli_overrides(args))?;
    let graphql_base_url = daemon_config.web.url();
    validate_graphql_base_url(&graphql_base_url)
        .await
        .map_err(CliError::Graphql)?;

    let result = run_memory(command, &graphql_base_url).await;
    let shutdown_result = daemon.shutdown_if_temporary().await;

    result?;
    shutdown_result?;
    Ok(())
}
