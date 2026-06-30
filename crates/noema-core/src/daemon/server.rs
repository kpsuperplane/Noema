use std::{fs, path::Path, sync::Arc};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream, unix::OwnedWriteHalf},
    sync::{mpsc, watch},
};

use super::{
    protocol::{DaemonError, DaemonRequest, DaemonResponse, DaemonServerConfig},
    runtime::CodexRuntimeHandle,
    web::{self, WebState},
};

/// Run the daemon until it receives a shutdown request.
///
/// # Errors
///
/// Returns [`DaemonError`] when the socket cannot be bound, the runtime cannot
/// start, or accepting a client connection fails.
pub async fn run_daemon(config: DaemonServerConfig) -> Result<(), DaemonError> {
    let listener = bind_listener(&config.socket_path).await?;
    let web_listener = web::bind_listener(&config.web).await?;
    let host = crate::NoemaRuntimeHost::start(config.codex)
        .await
        .map_err(|source| DaemonError::Protocol(source.to_string()))?;
    let runtime = host.runtime().clone();
    let graphql_state = crate::graphql::GraphqlState::from_runtime_host(&host);
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let state = Arc::new(DaemonState {
        runtime: runtime.clone(),
        shutdown_tx,
    });
    let web_state = WebState::new(graphql_state);

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|source| DaemonError::Socket {
                    path: config.socket_path.clone(),
                    source,
                })?;
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    let _ = handle_connection(stream, state).await;
                });
            }
            accepted = web_listener.accept() => {
                let (stream, _) = accepted?;
                let web_state = web_state.clone();
                tokio::spawn(async move {
                    let _ = web::handle_connection(stream, web_state).await;
                });
            }
        }
    }

    host.shutdown().await;
    let _ = fs::remove_file(&config.socket_path);
    Ok(())
}

pub(super) async fn bind_listener(socket_path: &Path) -> Result<UnixListener, DaemonError> {
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent).map_err(|source| DaemonError::Socket {
            path: socket_path.to_path_buf(),
            source,
        })?;
    }

    if socket_path.exists() {
        match UnixStream::connect(socket_path).await {
            Ok(_) => {
                return Err(DaemonError::AlreadyRunning {
                    path: socket_path.to_path_buf(),
                });
            }
            Err(_) => {
                fs::remove_file(socket_path).map_err(|source| DaemonError::Socket {
                    path: socket_path.to_path_buf(),
                    source,
                })?;
            }
        }
    }

    UnixListener::bind(socket_path).map_err(|source| DaemonError::Socket {
        path: socket_path.to_path_buf(),
        source,
    })
}

#[derive(Debug)]
struct DaemonState {
    runtime: CodexRuntimeHandle,
    shutdown_tx: watch::Sender<bool>,
}

async fn handle_connection(stream: UnixStream, state: Arc<DaemonState>) -> Result<(), DaemonError> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let request = match serde_json::from_str::<DaemonRequest>(&line) {
            Ok(request) => request,
            Err(source) => {
                send_response(
                    &mut writer,
                    &DaemonResponse::Error {
                        message: format!("invalid daemon request: {source}"),
                    },
                )
                .await?;
                continue;
            }
        };

        if let DaemonRequest::ConversationTurn {
            conversation_id,
            input,
        } = request
        {
            handle_turn_request(&mut writer, conversation_id, input, &state).await?;
            continue;
        }

        let response = handle_request(request, &state).await;
        send_response(&mut writer, &response).await?;
    }

    Ok(())
}

async fn handle_turn_request(
    writer: &mut OwnedWriteHalf,
    conversation_id: String,
    input: String,
    state: &DaemonState,
) -> Result<(), DaemonError> {
    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    let completion = state.runtime.turn(conversation_id.clone(), input, item_tx);
    tokio::pin!(completion);

    loop {
        tokio::select! {
            Some(event) = item_rx.recv() => {
                send_response(writer, &event.into_daemon_response()).await?;
            }
            result = &mut completion => {
                while let Ok(event) = item_rx.try_recv() {
                    send_response(writer, &event.into_daemon_response()).await?;
                }

                match result {
                    Ok(()) => {
                        send_response(writer, &DaemonResponse::TurnCompleted { conversation_id }).await?;
                    }
                    Err(error) => {
                        send_response(
                            writer,
                            &DaemonResponse::Error {
                                message: error.to_string(),
                            },
                        )
                        .await?;
                    }
                }
                break;
            }
        }
    }
    Ok(())
}
async fn handle_request(request: DaemonRequest, state: &DaemonState) -> DaemonResponse {
    match request {
        DaemonRequest::Hello => DaemonResponse::Ok {
            message: Some("noema daemon ready".to_string()),
        },
        DaemonRequest::ConversationStart {
            model,
            cwd,
            instructions: _,
        } => match state.runtime.start_conversation(model, cwd).await {
            Ok(started) => DaemonResponse::ConversationStarted {
                conversation_id: started.conversation_id,
                provider: "codex".to_string(),
            },
            Err(error) => DaemonResponse::Error {
                message: error.to_string(),
            },
        },
        DaemonRequest::ConversationTurn { .. } => DaemonResponse::Error {
            message: "conversation turns must be handled by the streaming path".to_string(),
        },
        DaemonRequest::ConversationEnd { conversation_id } => {
            match state.runtime.end_conversation(conversation_id).await {
                Ok(()) => DaemonResponse::Ok { message: None },
                Err(error) => DaemonResponse::Error {
                    message: error.to_string(),
                },
            }
        }
        DaemonRequest::Shutdown => {
            let _ = state.shutdown_tx.send(true);
            DaemonResponse::Ok {
                message: Some("daemon shutting down".to_string()),
            }
        }
    }
}

async fn send_response(
    writer: &mut OwnedWriteHalf,
    response: &DaemonResponse,
) -> Result<(), DaemonError> {
    let mut bytes =
        serde_json::to_vec(response).map_err(|source| DaemonError::Protocol(source.to_string()))?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
