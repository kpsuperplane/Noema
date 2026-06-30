//! Noema command-line entrypoint.

use clap::{Parser, Subcommand};
use commands::{
    run_chat, run_config, run_dev_daemon_command, run_memory_graphql, run_one_shot, run_start,
};
use inspection::{ContextCommand, MemoryCommand, run_context};
use noema_core::{
    CliOverrides, DaemonError, MemoryPersistenceError, NoemaHomeError, NoemaPathError,
    ProviderError,
};
#[cfg(test)]
use std::path::Path;
use std::{io, path::PathBuf};
use thiserror::Error;

mod commands;
mod dev;
mod graphql;
mod graphql_client;
mod inspection;

#[derive(Debug, Parser)]
#[command(name = "noema")]
#[command(about = "Send prompts through Noema providers or the Noema daemon.")]
struct Args {
    #[arg(long, global = true)]
    provider: Option<String>,

    #[arg(long, global = true)]
    model: Option<String>,

    #[arg(long = "base-url", global = true)]
    base_url: Option<String>,

    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<CommandKind>,

    #[arg(value_name = "PROMPT", trailing_var_arg = true)]
    prompt: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum CommandKind {
    #[command(about = "Run the Noema daemon in the foreground.")]
    Start,
    #[command(about = "Initialize or update the Noema directory.")]
    Config {
        #[arg(long, help = "Rewrite config.yaml with the default template.")]
        force: bool,
    },
    #[command(about = "Start a chat session through the Noema daemon.")]
    Chat {
        #[arg(value_name = "PROMPT", trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    #[command(about = "Run Rust daemon and core web asset watchers together.")]
    DevDaemon,
    #[command(about = "Inspect local owner/admin memory state.")]
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    #[command(about = "Inspect local context assembly state.")]
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
}

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Config(#[from] noema_core::ConfigError),

    #[error(transparent)]
    Prompt(#[from] noema_cli::PromptError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Daemon(#[from] DaemonError),

    #[error(transparent)]
    NoemaHome(#[from] NoemaHomeError),

    #[error(transparent)]
    NoemaPath(#[from] NoemaPathError),

    #[error(transparent)]
    Memory(#[from] MemoryPersistenceError),

    #[error("{0}")]
    Unavailable(String),

    #[error("failed to read stdin: {0}")]
    ReadStdin(io::Error),

    #[error("failed to write response: {0}")]
    WriteOutput(io::Error),

    #[error("failed to determine current directory: {0}")]
    CurrentDir(io::Error),

    #[error("failed to determine current executable: {0}")]
    CurrentExe(io::Error),

    #[error("failed to start temporary daemon: {0}")]
    SpawnDaemon(io::Error),

    #[error("GraphQL request failed: {0}")]
    Graphql(String),

    #[error(transparent)]
    DevDaemon(#[from] dev::DevDaemonError),
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), CliError> {
    let args = Args::parse();

    match &args.command {
        Some(CommandKind::Start) => run_start(&args).await,
        Some(CommandKind::Config { force }) => run_config(&args, *force),
        Some(CommandKind::Chat { prompt }) => run_chat(&args, prompt).await,
        Some(CommandKind::DevDaemon) => run_dev_daemon_command(&args).await,
        Some(CommandKind::Memory { command }) => run_memory_graphql(&args, command).await,
        Some(CommandKind::Context { command }) => run_context(command, args.config.clone()).await,
        None => run_one_shot(args).await,
    }
}

pub(crate) fn cli_overrides(args: &Args) -> CliOverrides {
    CliOverrides::new(
        args.provider.clone(),
        args.model.clone(),
        args.base_url.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_start_subcommand() {
        let args = Args::try_parse_from(["noema", "start"]).expect("args");

        assert!(matches!(args.command, Some(CommandKind::Start)));
    }

    #[test]
    fn parses_config_subcommand() {
        let args = Args::try_parse_from(["noema", "config"]).expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Config { force: false })
        ));
    }

    #[test]
    fn parses_config_force_subcommand() {
        let args = Args::try_parse_from(["noema", "config", "--force"]).expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Config { force: true })
        ));
    }

    #[test]
    fn parses_provider_before_start_subcommand() {
        let args = Args::try_parse_from(["noema", "--provider", "codex", "start"]).expect("args");

        assert_eq!(args.provider.as_deref(), Some("codex"));
        assert!(matches!(args.command, Some(CommandKind::Start)));
    }

    #[test]
    fn parses_chat_prompt_subcommand() {
        let args = Args::try_parse_from(["noema", "chat", "hello", "there"]).expect("args");

        let Some(CommandKind::Chat { prompt }) = args.command else {
            panic!("expected chat command");
        };
        assert_eq!(prompt, ["hello", "there"]);
    }

    #[test]
    fn parses_dev_daemon_subcommand() {
        let args = Args::try_parse_from(["noema", "dev-daemon"]).expect("args");

        assert!(matches!(args.command, Some(CommandKind::DevDaemon)));
    }

    #[test]
    fn parses_provider_before_chat_subcommand() {
        let args = Args::try_parse_from(["noema", "--provider", "codex", "chat"]).expect("args");

        assert_eq!(args.provider.as_deref(), Some("codex"));
        assert!(matches!(args.command, Some(CommandKind::Chat { .. })));
    }

    #[test]
    fn parses_memory_list_subcommand() {
        let args = Args::try_parse_from(["noema", "memory", "list", "--limit", "7"]).expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Memory {
                command: MemoryCommand::List {
                    limit: 7,
                    query: None,
                    status: None,
                    predicate_id: None,
                }
            })
        ));
    }

    #[test]
    fn parses_memory_list_filters() {
        let args = Args::try_parse_from([
            "noema",
            "memory",
            "list",
            "--query",
            "trains",
            "--status",
            "confirmed",
            "--predicate-id",
            "likes",
        ])
        .expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Memory {
                command: MemoryCommand::List {
                    query: Some(query),
                    status: Some(status),
                    predicate_id: Some(predicate_id),
                    ..
                }
            }) if query == "trains" && status == "confirmed" && predicate_id == "likes"
        ));
    }

    #[test]
    fn parses_config_before_memory_subcommand() {
        let args = Args::try_parse_from(["noema", "--config", "custom.yaml", "memory", "list"])
            .expect("args");

        assert_eq!(args.config.as_deref(), Some(Path::new("custom.yaml")));
        assert!(matches!(
            args.command,
            Some(CommandKind::Memory {
                command: MemoryCommand::List { .. }
            })
        ));
    }

    #[test]
    fn parses_memory_show_subcommand() {
        let args = Args::try_parse_from(["noema", "memory", "show", "mem_123"]).expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Memory {
                command: MemoryCommand::Show { memory_id }
            }) if memory_id == "mem_123"
        ));
    }

    #[test]
    fn parses_memory_predicate_proposals_subcommand() {
        let args = Args::try_parse_from([
            "noema",
            "memory",
            "predicate-proposals",
            "--limit",
            "9",
            "--status",
            "candidate",
        ])
        .expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Memory {
                command: MemoryCommand::PredicateProposals {
                    limit: 9,
                    status: Some(status),
                }
            }) if status == "candidate"
        ));
    }

    #[test]
    fn parses_memory_predicate_proposal_subcommand() {
        let args = Args::try_parse_from(["noema", "memory", "predicate-proposal", "proposal_123"])
            .expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Memory {
                command: MemoryCommand::PredicateProposal { proposal_id }
            }) if proposal_id == "proposal_123"
        ));
    }

    #[test]
    fn parses_context_graph_subcommand() {
        let args = Args::try_parse_from(["noema", "context", "graph"]).expect("args");

        assert!(matches!(
            args.command,
            Some(CommandKind::Context {
                command: ContextCommand::Graph
            })
        ));
    }

    #[test]
    fn parses_config_before_context_subcommand() {
        let args = Args::try_parse_from(["noema", "--config", "custom.yaml", "context", "graph"])
            .expect("args");

        assert_eq!(args.config.as_deref(), Some(Path::new("custom.yaml")));
        assert!(matches!(
            args.command,
            Some(CommandKind::Context {
                command: ContextCommand::Graph
            })
        ));
    }

    #[test]
    fn parses_one_shot_prompt_without_subcommand() {
        let args = Args::try_parse_from(["noema", "hello", "there"]).expect("args");

        assert!(args.command.is_none());
        assert_eq!(args.prompt, ["hello", "there"]);
    }
}
