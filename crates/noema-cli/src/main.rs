use clap::{Parser, Subcommand};
use noema_cli::collect_prompt;
use noema_core::{
    CliOverrides, CodexProvider, Config, DaemonClient, DaemonError, DaemonServerConfig,
    GenerateInput, GenerateOptions, GenerateRequest, ModelProvider, OpenAiProvider, ProviderConfig,
    ProviderError, default_socket_path, is_connection_refused, run_daemon,
};
use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time,
};

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
    #[command(about = "Start a chat session through the Noema daemon.")]
    Chat {
        #[arg(value_name = "PROMPT", trailing_var_arg = true)]
        prompt: Vec<String>,
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
        Some(CommandKind::Chat { prompt }) => run_chat(&args, prompt).await,
        None => run_one_shot(args).await,
    }
}

async fn run_start(args: &Args) -> Result<(), CliError> {
    let codex = Config::load_codex(args.config.clone(), cli_overrides(args))?;
    let socket_path = default_socket_path()?;
    eprintln!("noema daemon listening at {}", socket_path.display());
    run_daemon(DaemonServerConfig::new(socket_path, codex)).await?;
    Ok(())
}

async fn run_chat(args: &Args, prompt_args: &[String]) -> Result<(), CliError> {
    let mut daemon = ConnectedDaemon::connect_or_start(args).await?;
    let cwd = env::current_dir()
        .map_err(CliError::CurrentDir)?
        .to_string_lossy()
        .to_string();
    let conversation = daemon
        .client
        .start_conversation(args.model.clone(), Some(cwd))
        .await?;

    let result = if prompt_args.is_empty() {
        run_interactive_chat(&mut daemon.client, &conversation.conversation_id).await
    } else {
        let prompt = collect_prompt(prompt_args, "")?;
        let response = daemon
            .client
            .turn(conversation.conversation_id.clone(), prompt)
            .await?;
        print_response(&response)?;
        Ok(())
    };

    let end_result = daemon
        .client
        .end_conversation(conversation.conversation_id)
        .await
        .map_err(CliError::Daemon);
    let shutdown_result = daemon.shutdown_if_temporary().await;

    result?;
    end_result?;
    shutdown_result?;
    Ok(())
}

async fn run_interactive_chat(
    client: &mut DaemonClient,
    conversation_id: &str,
) -> Result<(), CliError> {
    let is_terminal = io::stdin().is_terminal();
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    loop {
        if is_terminal {
            eprint!("noema> ");
            io::stderr().flush().map_err(CliError::WriteOutput)?;
        }

        let Some(line) = lines.next_line().await.map_err(CliError::ReadStdin)? else {
            break;
        };
        let prompt = line.trim();
        if prompt == "/quit" {
            break;
        }
        if prompt.is_empty() {
            continue;
        }

        let response = client
            .turn(conversation_id.to_string(), prompt.to_string())
            .await?;
        print_response(&response)?;
    }

    Ok(())
}

async fn run_one_shot(args: Args) -> Result<(), CliError> {
    let stdin = read_stdin_if_needed(args.prompt.is_empty())?;
    let prompt = collect_prompt(&args.prompt, &stdin)?;
    let overrides = cli_overrides(&args);

    let config = Config::load(args.config, overrides)?;

    match config.provider {
        ProviderConfig::OpenAi(openai_config) => {
            let model = openai_config.default_model.clone();
            let provider = OpenAiProvider::new(openai_config)?;
            let response = provider
                .generate(GenerateRequest {
                    model: Some(model),
                    input: GenerateInput::Text(prompt),
                    instructions: None,
                    options: GenerateOptions::default(),
                })
                .await?;

            print_response(&response.text)?;
        }
        ProviderConfig::Codex(codex_config) => {
            let model = codex_config.default_model.clone();
            let provider = CodexProvider::new(codex_config)?;
            let response = provider
                .generate(GenerateRequest {
                    model,
                    input: GenerateInput::Text(prompt),
                    instructions: None,
                    options: GenerateOptions::default(),
                })
                .await?;

            print_response(&response.text)?;
        }
    }

    Ok(())
}

fn cli_overrides(args: &Args) -> CliOverrides {
    CliOverrides {
        provider: args.provider.clone(),
        model: args.model.clone(),
        base_url: args.base_url.clone(),
    }
}

struct ConnectedDaemon {
    client: DaemonClient,
    temporary_child: Option<Child>,
}

impl ConnectedDaemon {
    async fn connect_or_start(args: &Args) -> Result<Self, CliError> {
        let socket_path = default_socket_path()?;
        match DaemonClient::connect(&socket_path).await {
            Ok(mut client) => {
                client.hello().await?;
                Ok(Self {
                    client,
                    temporary_child: None,
                })
            }
            Err(error) if is_connection_refused(&error) => {
                eprintln!(
                    "noema daemon is not running; starting a temporary daemon for this chat. Run `noema start` to keep it warm."
                );
                Self::start_temporary(args, socket_path).await
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn start_temporary(args: &Args, socket_path: PathBuf) -> Result<Self, CliError> {
        let mut child = spawn_temporary_daemon(args)?;
        let client = wait_for_daemon(&socket_path, &mut child).await?;

        Ok(Self {
            client,
            temporary_child: Some(child),
        })
    }

    async fn shutdown_if_temporary(&mut self) -> Result<(), CliError> {
        let Some(mut child) = self.temporary_child.take() else {
            return Ok(());
        };

        let _ = self.client.shutdown().await;
        match time::timeout(Duration::from_secs(5), child.wait()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(CliError::SpawnDaemon(error)),
            Err(_) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                Ok(())
            }
        }
    }
}

fn spawn_temporary_daemon(args: &Args) -> Result<Child, CliError> {
    let exe = env::current_exe().map_err(CliError::CurrentExe)?;
    let mut command = Command::new(exe);

    if let Some(config) = args.config.as_ref() {
        command.arg("--config").arg(config);
    }
    if let Some(model) = args.model.as_ref() {
        command.arg("--model").arg(model);
    }
    if let Some(base_url) = args.base_url.as_ref() {
        command.arg("--base-url").arg(base_url);
    }

    command
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.kill_on_drop(true);
    command.spawn().map_err(CliError::SpawnDaemon)
}

async fn wait_for_daemon(
    socket_path: &PathBuf,
    child: &mut Child,
) -> Result<DaemonClient, CliError> {
    let deadline = time::Instant::now() + Duration::from_secs(10);

    loop {
        if let Some(status) = child.try_wait().map_err(CliError::SpawnDaemon)? {
            return Err(CliError::Daemon(DaemonError::Protocol(format!(
                "temporary daemon exited before accepting connections: {status}"
            ))));
        }

        match DaemonClient::connect(socket_path).await {
            Ok(mut client) => {
                client.hello().await?;
                return Ok(client);
            }
            Err(error) if is_connection_refused(&error) && time::Instant::now() < deadline => {
                time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn read_stdin_if_needed(needed: bool) -> Result<String, CliError> {
    if !needed {
        return Ok(String::new());
    }

    if io::stdin().is_terminal() {
        return Ok(String::new());
    }

    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .map_err(CliError::ReadStdin)?;
    Ok(stdin)
}

fn print_response(text: &str) -> Result<(), CliError> {
    let mut stdout = io::stdout();
    stdout
        .write_all(text.as_bytes())
        .map_err(CliError::WriteOutput)?;

    if !text.ends_with('\n') {
        stdout.write_all(b"\n").map_err(CliError::WriteOutput)?;
    }

    Ok(())
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
    fn parses_chat_prompt_subcommand() {
        let args = Args::try_parse_from(["noema", "chat", "hello", "there"]).expect("args");

        let Some(CommandKind::Chat { prompt }) = args.command else {
            panic!("expected chat command");
        };
        assert_eq!(prompt, ["hello", "there"]);
    }

    #[test]
    fn parses_one_shot_prompt_without_subcommand() {
        let args = Args::try_parse_from(["noema", "hello", "there"]).expect("args");

        assert!(args.command.is_none());
        assert_eq!(args.prompt, ["hello", "there"]);
    }
}
