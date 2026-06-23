use crate::{
    provider::{GenerateResponse, ProviderError},
    providers::{
        codex::CodexProviderConfig,
        codex_app_server::{CodexAppServerConversation, CodexAppServerRuntime},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::{
        UnixListener, UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::{mpsc, oneshot, watch},
};

pub const DEFAULT_DAEMON_SOCKET_NAME: &str = "noema.sock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    Hello,
    ConversationStart {
        model: Option<String>,
        cwd: Option<String>,
        instructions: Option<String>,
    },
    ConversationTurn {
        conversation_id: String,
        input: String,
    },
    ConversationEnd {
        conversation_id: String,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    Ok {
        message: Option<String>,
    },
    ConversationStarted {
        conversation_id: String,
        provider: String,
        provider_thread_id: String,
    },
    TurnCompleted {
        conversation_id: String,
        text: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonServerConfig {
    pub socket_path: PathBuf,
    pub codex: CodexProviderConfig,
}

impl DaemonServerConfig {
    pub fn new(socket_path: PathBuf, codex: CodexProviderConfig) -> Self {
        Self { socket_path, codex }
    }
}

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("could not determine home directory for daemon socket")]
    MissingHome,

    #[error("daemon is already running at {}", path.display())]
    AlreadyRunning { path: PathBuf },

    #[error("daemon socket error at {}: {source}", path.display())]
    Socket {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("daemon I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("daemon protocol error: {0}")]
    Protocol(String),

    #[error("daemon returned error: {0}")]
    Remote(String),

    #[error(transparent)]
    Provider(#[from] ProviderError),
}

pub fn default_socket_path() -> Result<PathBuf, DaemonError> {
    let home = env::var_os("HOME").ok_or(DaemonError::MissingHome)?;
    Ok(socket_path_for_home(home))
}

pub fn socket_path_for_home(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref()
        .join(".noema")
        .join("run")
        .join(DEFAULT_DAEMON_SOCKET_NAME)
}

pub async fn run_daemon(config: DaemonServerConfig) -> Result<(), DaemonError> {
    let listener = bind_listener(&config.socket_path).await?;
    let runtime = CodexRuntimeHandle::spawn(config.codex)?;
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let state = Arc::new(DaemonState {
        runtime,
        shutdown_tx,
    });

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
        }
    }

    state.runtime.shutdown().await;
    let _ = fs::remove_file(&config.socket_path);
    Ok(())
}

async fn bind_listener(socket_path: &Path) -> Result<UnixListener, DaemonError> {
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

        let response = match serde_json::from_str::<DaemonRequest>(&line) {
            Ok(request) => handle_request(request, &state).await,
            Err(source) => DaemonResponse::Error {
                message: format!("invalid daemon request: {source}"),
            },
        };
        send_response(&mut writer, &response).await?;
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
                provider_thread_id: started.provider_thread_id,
            },
            Err(error) => DaemonResponse::Error {
                message: error.to_string(),
            },
        },
        DaemonRequest::ConversationTurn {
            conversation_id,
            input,
        } => match state.runtime.turn(conversation_id.clone(), input).await {
            Ok(response) => DaemonResponse::TurnCompleted {
                conversation_id,
                text: response.text,
            },
            Err(error) => DaemonResponse::Error {
                message: error.to_string(),
            },
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

#[derive(Debug)]
pub struct DaemonClient {
    reader: Lines<BufReader<OwnedReadHalf>>,
    writer: OwnedWriteHalf,
}

impl DaemonClient {
    pub async fn connect(socket_path: &Path) -> Result<Self, DaemonError> {
        let stream =
            UnixStream::connect(socket_path)
                .await
                .map_err(|source| DaemonError::Socket {
                    path: socket_path.to_path_buf(),
                    source,
                })?;
        let (reader, writer) = stream.into_split();

        Ok(Self {
            reader: BufReader::new(reader).lines(),
            writer,
        })
    }

    pub async fn request(&mut self, request: DaemonRequest) -> Result<DaemonResponse, DaemonError> {
        let mut bytes = serde_json::to_vec(&request)
            .map_err(|source| DaemonError::Protocol(source.to_string()))?;
        bytes.push(b'\n');
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;

        let line = self
            .reader
            .next_line()
            .await?
            .ok_or_else(|| DaemonError::Protocol("daemon closed connection".to_string()))?;
        let response = serde_json::from_str::<DaemonResponse>(&line)
            .map_err(|source| DaemonError::Protocol(source.to_string()))?;

        if let DaemonResponse::Error { message } = response {
            return Err(DaemonError::Remote(message));
        }

        Ok(response)
    }

    pub async fn hello(&mut self) -> Result<(), DaemonError> {
        let response = self.request(DaemonRequest::Hello).await?;
        match response {
            DaemonResponse::Ok { .. } => Ok(()),
            other => Err(DaemonError::Protocol(format!(
                "unexpected hello response: {other:?}"
            ))),
        }
    }

    pub async fn start_conversation(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let response = self
            .request(DaemonRequest::ConversationStart {
                model,
                cwd,
                instructions: None,
            })
            .await?;

        match response {
            DaemonResponse::ConversationStarted {
                conversation_id,
                provider: _,
                provider_thread_id,
            } => Ok(StartedConversation {
                conversation_id,
                provider_thread_id,
            }),
            other => Err(DaemonError::Protocol(format!(
                "unexpected conversation_start response: {other:?}"
            ))),
        }
    }

    pub async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
    ) -> Result<String, DaemonError> {
        let response = self
            .request(DaemonRequest::ConversationTurn {
                conversation_id: conversation_id.clone(),
                input,
            })
            .await?;

        match response {
            DaemonResponse::TurnCompleted {
                conversation_id: _,
                text,
            } => Ok(text),
            other => Err(DaemonError::Protocol(format!(
                "unexpected conversation_turn response: {other:?}"
            ))),
        }
    }

    pub async fn end_conversation(&mut self, conversation_id: String) -> Result<(), DaemonError> {
        let response = self
            .request(DaemonRequest::ConversationEnd { conversation_id })
            .await?;
        match response {
            DaemonResponse::Ok { .. } => Ok(()),
            other => Err(DaemonError::Protocol(format!(
                "unexpected conversation_end response: {other:?}"
            ))),
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), DaemonError> {
        let response = self.request(DaemonRequest::Shutdown).await?;
        match response {
            DaemonResponse::Ok { .. } => Ok(()),
            other => Err(DaemonError::Protocol(format!(
                "unexpected shutdown response: {other:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedConversation {
    pub conversation_id: String,
    pub provider_thread_id: String,
}

#[derive(Debug, Clone)]
struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
}

impl CodexRuntimeHandle {
    fn spawn(config: CodexProviderConfig) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new(config)?;
        tokio::spawn(actor.run(receiver));
        Ok(Self { sender })
    }

    async fn start_conversation(
        &self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::StartConversation { model, cwd, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    async fn turn(
        &self,
        conversation_id: String,
        input: String,
    ) -> Result<GenerateResponse, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::Turn {
                conversation_id,
                input,
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    async fn end_conversation(&self, conversation_id: String) -> Result<(), DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::EndConversation {
                conversation_id,
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    async fn shutdown(&self) {
        let (reply, reply_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(CodexRuntimeCommand::Shutdown { reply })
            .await;
        let _ = reply_rx.await;
    }
}

#[derive(Debug)]
enum CodexRuntimeCommand {
    StartConversation {
        model: Option<String>,
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
    },
    Turn {
        conversation_id: String,
        input: String,
        reply: oneshot::Sender<Result<GenerateResponse, DaemonError>>,
    },
    EndConversation {
        conversation_id: String,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Debug)]
struct CodexRuntimeActor {
    runtime: CodexAppServerRuntime,
    conversations: HashMap<String, CodexAppServerConversation>,
    next_conversation_id: u64,
}

impl CodexRuntimeActor {
    fn new(config: CodexProviderConfig) -> Result<Self, DaemonError> {
        Ok(Self {
            runtime: CodexAppServerRuntime::new(config)?,
            conversations: HashMap::new(),
            next_conversation_id: 1,
        })
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<CodexRuntimeCommand>) {
        while let Some(command) = receiver.recv().await {
            match command {
                CodexRuntimeCommand::StartConversation { model, cwd, reply } => {
                    let _ = reply.send(self.start_conversation(model, cwd).await);
                }
                CodexRuntimeCommand::Turn {
                    conversation_id,
                    input,
                    reply,
                } => {
                    let _ = reply.send(self.turn(conversation_id, input).await);
                }
                CodexRuntimeCommand::EndConversation {
                    conversation_id,
                    reply,
                } => {
                    self.conversations.remove(&conversation_id);
                    let _ = reply.send(Ok(()));
                }
                CodexRuntimeCommand::Shutdown { reply } => {
                    self.runtime.shutdown().await;
                    let _ = reply.send(());
                    break;
                }
            }
        }
    }

    async fn start_conversation(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let conversation = self.runtime.start_conversation(model, cwd).await?;
        let conversation_id = format!("conversation_{}", self.next_conversation_id);
        self.next_conversation_id += 1;
        let provider_thread_id = conversation.thread_id.clone();
        self.conversations
            .insert(conversation_id.clone(), conversation);

        Ok(StartedConversation {
            conversation_id,
            provider_thread_id,
        })
    }

    async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
    ) -> Result<GenerateResponse, DaemonError> {
        let conversation = self
            .conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| {
                DaemonError::Protocol(format!("unknown conversation id: {conversation_id}"))
            })?;

        match self.runtime.turn(&conversation, input).await {
            Ok(response) => Ok(response),
            Err(error) => {
                self.conversations.remove(&conversation_id);
                Err(error.into())
            }
        }
    }
}

pub fn is_connection_refused(error: &DaemonError) -> bool {
    matches!(
        error,
        DaemonError::Socket { source, .. }
            if matches!(source.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::codex::CodexProviderConfig;

    #[test]
    fn protocol_round_trips_requests_and_responses() {
        let request = DaemonRequest::ConversationTurn {
            conversation_id: "conversation_1".to_string(),
            input: "hello".to_string(),
        };
        let encoded = serde_json::to_string(&request).expect("encode");
        let decoded: DaemonRequest = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, request);

        let response = DaemonResponse::ConversationStarted {
            conversation_id: "conversation_1".to_string(),
            provider: "codex".to_string(),
            provider_thread_id: "thread_1".to_string(),
        };
        let encoded = serde_json::to_string(&response).expect("encode");
        let decoded: DaemonResponse = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn socket_path_is_under_noema_run_directory() {
        let path = socket_path_for_home("/tmp/noema-test-home");

        assert_eq!(
            path,
            PathBuf::from("/tmp/noema-test-home/.noema/run/noema.sock")
        );
    }

    #[tokio::test]
    async fn second_listener_on_same_socket_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let socket_path = dir.path().join("noema.sock");
        let _listener = bind_listener(&socket_path).await.expect("listener");

        let error = bind_listener(&socket_path).await.unwrap_err();

        assert!(matches!(error, DaemonError::AlreadyRunning { .. }));
    }

    #[tokio::test]
    async fn runtime_actor_allocates_distinct_conversation_ids() {
        let script = fake_codex_app_server_script();
        let handle = CodexRuntimeHandle::spawn(CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        })
        .expect("runtime");

        let first = handle
            .start_conversation(None, None)
            .await
            .expect("first conversation");
        let second = handle
            .start_conversation(None, None)
            .await
            .expect("second conversation");

        assert_ne!(first.conversation_id, second.conversation_id);
        assert_ne!(first.provider_thread_id, second.provider_thread_id);

        let response = handle
            .turn(first.conversation_id.clone(), "hello".to_string())
            .await
            .expect("turn response");
        assert_eq!(response.text, "fake answer");

        handle.shutdown().await;
    }

    fn fake_codex_app_server_script() -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("temp dir").keep();
        let path = dir.join("fake-codex");
        std::fs::write(
            &path,
            r#"#!/usr/bin/env python3
import json
import sys

next_thread = 1

for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"id": msg["id"], "result": {"userAgent": "fake"}}), flush=True)
    elif method == "initialized":
        pass
    elif method == "thread/start":
        thread = f"thread_{next_thread}"
        next_thread += 1
        print(json.dumps({"id": msg["id"], "result": {"thread": {"id": thread}}}), flush=True)
    elif method == "turn/start":
        print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
        print(json.dumps({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": "fake answer"}}}), flush=True)
        print(json.dumps({"method": "turn/completed", "params": {"turn": {"status": "completed"}}}), flush=True)
"#,
        )
        .expect("write script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&path, permissions).expect("chmod");
        }

        path
    }
}
