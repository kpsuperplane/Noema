//! Chat and one-shot prompt commands.

use crate::{
    Args, CliError, cli_overrides,
    commands::ConnectedDaemon,
    graphql_client::{
        start_primary_conversation, stream_conversation_turn, validate_graphql_base_url,
    },
};
use noema_cli::collect_prompt;
use noema_core::{
    CodexResponsesProvider, Config, GenerateInput, GenerateOptions, GenerateRequest, ModelProvider,
    NoemaPaths, OpenAiProvider, ProviderConfig, TurnActivityStatus, TurnTranscriptItem,
};
use std::env;
use std::io::{self, IsTerminal, Read, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

pub(crate) async fn run_chat(args: &Args, prompt_args: &[String]) -> Result<(), CliError> {
    let mut daemon = ConnectedDaemon::connect_or_start(args).await?;
    let daemon_config = Config::load_daemon(args.config.clone(), cli_overrides(args))?;
    let graphql_base_url = daemon_config.web.url();
    validate_graphql_base_url(&graphql_base_url)
        .await
        .map_err(CliError::Graphql)?;
    let cwd = env::current_dir()
        .map_err(CliError::CurrentDir)?
        .to_string_lossy()
        .to_string();
    let conversation_id =
        start_primary_conversation(&graphql_base_url, args.model.clone(), Some(cwd))
            .await
            .map_err(CliError::Graphql)?;

    let result = if prompt_args.is_empty() {
        run_interactive_chat(&graphql_base_url, &conversation_id).await
    } else {
        async {
            let prompt = collect_prompt(prompt_args, "")?;
            print_chat_turn_graphql(&graphql_base_url, &conversation_id, prompt).await
        }
        .await
    };

    let end_result = daemon
        .client
        .end_conversation(conversation_id)
        .await
        .map_err(CliError::Daemon);
    let shutdown_result = daemon.shutdown_if_temporary().await;

    result?;
    end_result?;
    shutdown_result?;
    Ok(())
}

async fn run_interactive_chat(base_url: &str, conversation_id: &str) -> Result<(), CliError> {
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

        print_chat_turn_graphql(base_url, conversation_id, prompt.to_string()).await?;
    }

    Ok(())
}

async fn print_chat_turn_graphql(
    base_url: &str,
    conversation_id: &str,
    prompt: String,
) -> Result<(), CliError> {
    let mut events = stream_conversation_turn(base_url, conversation_id, prompt)
        .await
        .map_err(CliError::Graphql)?;
    let mut terminal_error = None;

    while let Some(event) = events.recv().await {
        let event = event.map_err(CliError::Graphql)?;
        if terminal_error.is_none() {
            terminal_error.clone_from(&event.terminal_error);
        }
        if let Some(item) = event.transcript_item {
            print_transcript_item(&item)?;
        }

        if event.completed {
            if let Some(error) = terminal_error {
                return Err(CliError::Graphql(error));
            }
            return Ok(());
        }
    }

    Err(CliError::Graphql(
        "GraphQL subscription ended before the conversation turn completed".to_string(),
    ))
}

pub(crate) async fn run_one_shot(args: Args) -> Result<(), CliError> {
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

            print_response(&response.assistant_text())?;
        }
        ProviderConfig::Codex(mut codex_config) => {
            let model = codex_config.default_model.clone();
            let paths = NoemaPaths::from_process_env()?;
            codex_config.account_home = Some(paths.provider_account_home("codex", "default"));
            let provider = CodexResponsesProvider::new(codex_config)?;
            let response = provider
                .generate(GenerateRequest {
                    model,
                    input: GenerateInput::Text(prompt),
                    instructions: None,
                    options: GenerateOptions::default(),
                })
                .await?;

            print_response(&response.assistant_text())?;
        }
    }

    Ok(())
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

fn print_transcript_item(item: &TurnTranscriptItem) -> Result<(), CliError> {
    match item {
        TurnTranscriptItem::UserText { .. } => Ok(()),
        TurnTranscriptItem::AssistantText { text } => print_response(text),
        TurnTranscriptItem::Activity {
            activity_kind,
            status,
            title,
            summary,
            ..
        } => {
            let label = activity_kind.replace('_', " ");
            let status = activity_status_label(*status);
            match summary {
                Some(summary) if !summary.trim().is_empty() => {
                    println!("[{label}] {status}: {title} - {}", summary.trim());
                }
                _ => println!("[{label}] {status}: {title}"),
            }
            Ok(())
        }
        TurnTranscriptItem::A2uiCard { id, schema, .. } => {
            println!("[card] {schema} ({id})");
            Ok(())
        }
        TurnTranscriptItem::ErrorNotice {
            message,
            recoverable,
        } => {
            let label = if *recoverable { "notice" } else { "error" };
            println!("[{label}] {message}");
            Ok(())
        }
    }
}

fn activity_status_label(status: TurnActivityStatus) -> &'static str {
    match status {
        TurnActivityStatus::Started => "started",
        TurnActivityStatus::Completed => "completed",
        TurnActivityStatus::Failed => "failed",
    }
}
