//! Local daemon protocol and Unix-socket runtime.

use crate::{
    NoemaPathError, NoemaPaths,
    memory::{ParticipantRole, Sensitivity, SubjectRole},
    memory_extraction::{
        MemoryExtractionSubject, MemoryExtractionSubjectKind, MemoryExtractionSubjectRole,
        ValidatedMemoryProposal, build_memory_extraction_prompt, parse_memory_extraction_proposals,
    },
    memory_persistence::{
        ChatMemorySource, MemoryAuthorityLevel, MemoryExtractionMethod, MemoryPersistenceError,
        MemoryType, NewChatMemoryCandidate, NewChatTurn, NewMemoryParticipant, NewMemorySubject,
        SqliteMemoryRepository,
    },
    provider::ProviderError,
    providers::{
        codex::CodexProviderConfig,
        codex_app_server::{CodexAppServerConversation, CodexAppServerRuntime},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    fs,
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

/// Filename used for the daemon Unix socket.
pub const DEFAULT_DAEMON_SOCKET_NAME: &str = "noema.sock";

/// Requests accepted by the Noema daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    /// Health-check request.
    Hello,
    /// Start a provider-backed conversation.
    ConversationStart {
        /// Optional model override.
        model: Option<String>,
        /// Optional working directory for the conversation.
        cwd: Option<String>,
        /// Optional conversation instructions.
        instructions: Option<String>,
    },
    /// Send one user turn to an existing conversation.
    ConversationTurn {
        /// Daemon conversation id.
        conversation_id: String,
        /// User input.
        input: String,
    },
    /// End a conversation and release provider state.
    ConversationEnd {
        /// Daemon conversation id.
        conversation_id: String,
    },
    /// Ask the daemon to shut down.
    Shutdown,
}

/// Responses emitted by the Noema daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    /// Generic success response.
    Ok {
        /// Optional success message.
        message: Option<String>,
    },
    /// Conversation start response.
    ConversationStarted {
        /// Daemon conversation id.
        conversation_id: String,
        /// Provider used for the conversation.
        provider: String,
        /// Provider-native thread id.
        provider_thread_id: String,
    },
    /// One transcript item emitted by an in-progress turn.
    TurnTranscriptItem {
        /// Daemon conversation id.
        conversation_id: String,
        /// Transcript item to render in chat.
        item: TurnTranscriptItem,
    },
    /// Turn completion response.
    TurnCompleted {
        /// Daemon conversation id.
        conversation_id: String,
    },
    /// Error response returned over the daemon protocol.
    Error {
        /// Human-readable error message.
        message: String,
    },
}

/// Transcript item emitted by a daemon turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TurnTranscriptItem {
    /// Assistant text.
    AssistantText {
        /// Text to render as the assistant response.
        text: String,
    },
    /// Activity notice for work that runs alongside the turn.
    Activity {
        /// Stable activity id for correlating updates.
        id: String,
        /// Activity kind, such as `memory_extraction`.
        activity_kind: String,
        /// Activity status.
        status: TurnActivityStatus,
        /// Short display title.
        title: String,
        /// Optional concise summary.
        summary: Option<String>,
        /// Structured metadata for future clients.
        metadata: serde_json::Value,
    },
    /// Future structured UI card.
    A2uiCard {
        /// Stable card id.
        id: String,
        /// Card schema name.
        schema: String,
        /// Card payload.
        payload: serde_json::Value,
    },
    /// Recoverable or terminal notice shown in the transcript.
    ErrorNotice {
        /// Human-readable message.
        message: String,
        /// Whether the chat turn itself can continue.
        recoverable: bool,
    },
}

/// Status for transcript activity notices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnActivityStatus {
    /// Activity started.
    Started,
    /// Activity completed.
    Completed,
    /// Activity failed.
    Failed,
}

/// Configuration required to start the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonServerConfig {
    /// Unix socket path to bind.
    pub socket_path: PathBuf,
    /// Codex provider configuration used by daemon conversations.
    pub codex: CodexProviderConfig,
    /// Canonical SQLite database path.
    pub database_path: PathBuf,
}

impl DaemonServerConfig {
    /// Create daemon server configuration.
    #[must_use]
    pub fn new(socket_path: PathBuf, codex: CodexProviderConfig, database_path: PathBuf) -> Self {
        Self {
            socket_path,
            codex,
            database_path,
        }
    }
}

/// Errors produced by daemon client and server operations.
#[derive(Debug, Error)]
pub enum DaemonError {
    /// Another daemon appears to be listening at the socket path.
    #[error("daemon is already running at {}", path.display())]
    AlreadyRunning {
        /// Existing daemon socket path.
        path: PathBuf,
    },

    /// Unix-socket operation failed.
    #[error("daemon socket error at {}: {source}", path.display())]
    Socket {
        /// Socket path involved in the operation.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Generic daemon I/O failure.
    #[error("daemon I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Local protocol serialization or state error.
    #[error("daemon protocol error: {0}")]
    Protocol(String),

    /// Remote daemon returned an error response.
    #[error("daemon returned error: {0}")]
    Remote(String),

    /// Provider operation failed.
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// Memory persistence failed.
    #[error(transparent)]
    Memory(#[from] MemoryPersistenceError),

    /// Path resolution failed.
    #[error(transparent)]
    Path(#[from] NoemaPathError),
}

/// Return the default daemon socket path for the current process environment.
///
/// # Errors
///
/// Returns [`DaemonError`] when Noema path resolution fails.
pub fn default_socket_path() -> Result<PathBuf, DaemonError> {
    Ok(NoemaPaths::from_process_env()?.socket_path())
}

/// Return the daemon socket path under a home directory.
#[must_use]
pub fn socket_path_for_home(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref()
        .join(".noema")
        .join("run")
        .join(DEFAULT_DAEMON_SOCKET_NAME)
}

/// Run the daemon until it receives a shutdown request.
///
/// # Errors
///
/// Returns [`DaemonError`] when the socket cannot be bound, the runtime cannot
/// start, or accepting a client connection fails.
pub async fn run_daemon(config: DaemonServerConfig) -> Result<(), DaemonError> {
    let listener = bind_listener(&config.socket_path).await?;
    let runtime = CodexRuntimeHandle::spawn(config.codex, config.database_path)?;
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
            Some(item) = item_rx.recv() => {
                send_response(
                    writer,
                    &DaemonResponse::TurnTranscriptItem {
                        conversation_id: conversation_id.clone(),
                        item,
                    },
                )
                .await?;
            }
            result = &mut completion => {
                while let Ok(item) = item_rx.try_recv() {
                    send_response(
                        writer,
                        &DaemonResponse::TurnTranscriptItem {
                            conversation_id: conversation_id.clone(),
                            item,
                        },
                    )
                    .await?;
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
                provider_thread_id: started.provider_thread_id,
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

/// Client for the local Noema daemon protocol.
#[derive(Debug)]
pub struct DaemonClient {
    reader: Lines<BufReader<OwnedReadHalf>>,
    writer: OwnedWriteHalf,
}

impl DaemonClient {
    /// Connect to a daemon Unix socket.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when the socket cannot be opened.
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

    /// Send a raw daemon request and wait for one response.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when serialization, I/O, response parsing, or
    /// remote daemon handling fails.
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

    /// Send a daemon health-check request.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when the daemon is unavailable or replies with
    /// an unexpected response.
    pub async fn hello(&mut self) -> Result<(), DaemonError> {
        let response = self.request(DaemonRequest::Hello).await?;
        match response {
            DaemonResponse::Ok { .. } => Ok(()),
            other => Err(DaemonError::Protocol(format!(
                "unexpected hello response: {other:?}"
            ))),
        }
    }

    /// Start a daemon conversation.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when the daemon request fails or replies with an
    /// unexpected response.
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

    /// Send one turn to a daemon conversation.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when the daemon request fails or replies with an
    /// unexpected response.
    pub async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
    ) -> Result<Vec<TurnTranscriptItem>, DaemonError> {
        let mut items = Vec::new();
        self.turn_streaming(conversation_id, input, |item| items.push(item))
            .await?;
        Ok(items)
    }

    /// Send one turn and handle transcript items as they arrive.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when the daemon request fails or replies with an
    /// unexpected response.
    pub async fn turn_streaming<F>(
        &mut self,
        conversation_id: String,
        input: String,
        mut on_item: F,
    ) -> Result<(), DaemonError>
    where
        F: FnMut(TurnTranscriptItem),
    {
        let request = DaemonRequest::ConversationTurn {
            conversation_id: conversation_id.clone(),
            input,
        };
        let mut bytes = serde_json::to_vec(&request)
            .map_err(|source| DaemonError::Protocol(source.to_string()))?;
        bytes.push(b'\n');
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;

        loop {
            let line = self
                .reader
                .next_line()
                .await?
                .ok_or_else(|| DaemonError::Protocol("daemon closed connection".to_string()))?;
            let response = serde_json::from_str::<DaemonResponse>(&line)
                .map_err(|source| DaemonError::Protocol(source.to_string()))?;

            match response {
                DaemonResponse::TurnTranscriptItem {
                    conversation_id: item_conversation_id,
                    item,
                } if item_conversation_id == conversation_id => on_item(item),
                DaemonResponse::TurnCompleted {
                    conversation_id: completed_conversation_id,
                } if completed_conversation_id == conversation_id => return Ok(()),
                DaemonResponse::Error { message } => return Err(DaemonError::Remote(message)),
                other => {
                    return Err(DaemonError::Protocol(format!(
                        "unexpected conversation_turn response: {other:?}"
                    )));
                }
            }
        }
    }

    /// End a daemon conversation.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when the daemon request fails or replies with an
    /// unexpected response.
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

    /// Request daemon shutdown.
    ///
    /// # Errors
    ///
    /// Returns [`DaemonError`] when the daemon request fails or replies with an
    /// unexpected response.
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

/// Conversation ids allocated by the daemon and provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedConversation {
    /// Daemon-local conversation id.
    pub conversation_id: String,
    /// Provider-native thread id.
    pub provider_thread_id: String,
}

#[derive(Debug, Clone)]
struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
}

impl CodexRuntimeHandle {
    fn spawn(config: CodexProviderConfig, database_path: PathBuf) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new(config, database_path)?;
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
        item_tx: mpsc::UnboundedSender<TurnTranscriptItem>,
    ) -> Result<(), DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::Turn {
                conversation_id,
                input,
                item_tx,
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
        item_tx: mpsc::UnboundedSender<TurnTranscriptItem>,
        reply: oneshot::Sender<Result<(), DaemonError>>,
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
    memory_extraction_runtime: CodexAppServerRuntime,
    memory_repository: SqliteMemoryRepository,
    conversations: HashMap<String, ActiveConversation>,
    next_conversation_id: u64,
}

impl CodexRuntimeActor {
    fn new(config: CodexProviderConfig, database_path: PathBuf) -> Result<Self, DaemonError> {
        Ok(Self {
            memory_extraction_runtime: CodexAppServerRuntime::new(config.clone())?,
            runtime: CodexAppServerRuntime::new(config)?,
            memory_repository: SqliteMemoryRepository::open_at(database_path)?,
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
                    item_tx,
                    reply,
                } => {
                    let _ = reply.send(self.turn(conversation_id, input, item_tx).await);
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
                    self.memory_extraction_runtime.shutdown().await;
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
        let conversation = self.runtime.start_conversation(model, cwd.clone()).await?;
        let conversation_id = format!("conversation_{}", self.next_conversation_id);
        self.next_conversation_id += 1;
        let provider_thread_id = conversation.thread_id.clone();
        self.conversations.insert(
            conversation_id.clone(),
            ActiveConversation {
                provider: conversation,
                cwd,
                next_turn_index: 1,
            },
        );

        Ok(StartedConversation {
            conversation_id,
            provider_thread_id,
        })
    }

    async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnTranscriptItem>,
    ) -> Result<(), DaemonError> {
        let conversation = self
            .conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| {
                DaemonError::Protocol(format!("unknown conversation id: {conversation_id}"))
            })?;
        let turn_index = conversation.next_turn_index;
        let is_explicit_memory = explicit_memory_content(&input).is_some();

        self.persist_chat_memory_candidate(&conversation_id, turn_index, &input)?;

        match self
            .runtime
            .turn(&conversation.provider, input.clone())
            .await
        {
            Ok(response) => {
                let assistant_text = response.text;
                let _ = item_tx.send(TurnTranscriptItem::AssistantText {
                    text: assistant_text.clone(),
                });

                if let Some(conversation) = self.conversations.get_mut(&conversation_id) {
                    conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
                }

                let turn = NewChatTurn::new(
                    format!("conversation:{conversation_id}"),
                    turn_index,
                    "human:local",
                    "agent:primary",
                    input.clone(),
                    assistant_text.clone(),
                );
                if let Err(error) = self.memory_repository.record_chat_turn(&turn) {
                    let _ = item_tx.send(memory_activity_failed(
                        "memory_extraction:failed",
                        format!("failed to record chat turn provenance: {error}"),
                    ));
                    return Ok(());
                }

                if !is_explicit_memory {
                    self.extract_ordinary_chat_memories(
                        &conversation_id,
                        conversation.cwd.as_deref(),
                        &turn,
                        &item_tx,
                    )
                    .await;
                }

                Ok(())
            }
            Err(error) => {
                self.conversations.remove(&conversation_id);
                Err(error.into())
            }
        }
    }

    async fn extract_ordinary_chat_memories(
        &mut self,
        conversation_id: &str,
        cwd: Option<&str>,
        turn: &NewChatTurn,
        item_tx: &mpsc::UnboundedSender<TurnTranscriptItem>,
    ) {
        let turn_index = turn.turn_index;
        let activity_id = format!("memory_extraction:{conversation_id}:{turn_index}");
        let _ = item_tx.send(memory_activity(
            &activity_id,
            TurnActivityStatus::Started,
            "Extracting memory proposals",
            Some("ordinary chat memory extraction is running"),
            json!({ "turn_index": turn_index }),
        ));

        let project_hint = project_scope_from_cwd(cwd);
        let prompt = build_memory_extraction_prompt(
            &turn.user_content,
            &turn.assistant_content,
            conversation_id,
            turn_index,
            project_hint.as_deref(),
        );

        let extraction = match self
            .memory_extraction_runtime
            .start_conversation(None, cwd.map(ToOwned::to_owned))
            .await
        {
            Ok(extraction_conversation) => {
                self.memory_extraction_runtime
                    .turn(&extraction_conversation, prompt)
                    .await
            }
            Err(error) => Err(error),
        };

        let extraction_text = match extraction {
            Ok(response) => response.text,
            Err(error) => {
                let _ = item_tx.send(memory_activity_failed(
                    &activity_id,
                    format!("memory extraction model failed: {error}"),
                ));
                return;
            }
        };

        let proposals = match parse_memory_extraction_proposals(
            &extraction_text,
            &turn.user_content,
            &turn.assistant_content,
        ) {
            Ok(proposals) => proposals,
            Err(error) => {
                let _ = item_tx.send(memory_activity_failed(
                    &activity_id,
                    format!("memory extraction output was rejected: {error}"),
                ));
                return;
            }
        };

        let conversation_scope_id = format!("conversation:{conversation_id}");
        let mut created_memory_ids = Vec::new();
        for proposal in proposals {
            let candidate = match extracted_proposal_to_candidate(
                &proposal,
                &conversation_scope_id,
                project_hint.as_deref(),
                turn,
                &turn.user_content,
            ) {
                Ok(candidate) => candidate,
                Err(error) => {
                    let _ = item_tx.send(memory_activity_failed(&activity_id, error));
                    return;
                }
            };

            match self
                .memory_repository
                .append_chat_memory_candidate(&candidate)
            {
                Ok(summary) => created_memory_ids.push(summary.id),
                Err(error) => {
                    let _ = item_tx.send(memory_activity_failed(
                        &activity_id,
                        format!("failed to persist extracted memory: {error}"),
                    ));
                    return;
                }
            }
        }

        let summary = match created_memory_ids.len() {
            0 => "created no memory candidates".to_string(),
            1 => "created 1 memory candidate".to_string(),
            count => format!("created {count} memory candidates"),
        };
        let _ = item_tx.send(memory_activity(
            &activity_id,
            TurnActivityStatus::Completed,
            "Memory extraction completed",
            Some(&summary),
            json!({
                "turn_index": turn_index,
                "created_memory_ids": created_memory_ids,
            }),
        ));
    }

    fn persist_chat_memory_candidate(
        &mut self,
        conversation_id: &str,
        turn_index: u64,
        user_input: &str,
    ) -> Result<(), DaemonError> {
        let Some(memory_content) = explicit_memory_content(user_input) else {
            return Ok(());
        };

        let conversation_scope_id = format!("conversation:{conversation_id}");
        let message_id = format!("message:{conversation_scope_id}:user:{turn_index}");
        let mut candidate = NewChatMemoryCandidate::new(
            conversation_scope_id.clone(),
            memory_content,
            "human:local",
        );
        candidate.memory_type = infer_chat_memory_type(&candidate.content);
        candidate.title = Some(title_from_memory_content(&candidate.content));
        candidate.sensitivity = infer_chat_sensitivity(&candidate.content);
        candidate.status = crate::memory::MemoryStatus::Confirmed;
        candidate.owner_principal_id = Some("human:local".to_string());
        candidate.authority_level = MemoryAuthorityLevel::ExplicitHumanStatement;
        candidate.extraction_method = MemoryExtractionMethod::ExplicitHuman;
        candidate.source = Some(ChatMemorySource {
            conversation_id: conversation_scope_id,
            message_id: Some(message_id),
            evidence_excerpt: Some(user_input.trim().to_string()),
        });
        candidate.participants = vec![
            NewMemoryParticipant::new("human:local", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];
        candidate.metadata = json!({
            "trigger": "explicit_remember",
            "turn_index": turn_index,
        });

        self.memory_repository
            .append_chat_memory_candidate(&candidate)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ActiveConversation {
    provider: CodexAppServerConversation,
    cwd: Option<String>,
    next_turn_index: u64,
}

fn explicit_memory_content(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let lowered = trimmed.to_ascii_lowercase();
    for prefix in ["remember this:", "remember that:", "remember:"] {
        if lowered.starts_with(prefix) {
            let content = trimmed[prefix.len()..].trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }
    if lowered.starts_with("/remember ") {
        let content = trimmed["/remember ".len()..].trim();
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    if lowered.starts_with("/remember:") {
        let content = trimmed["/remember:".len()..].trim();
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    None
}

fn infer_chat_memory_type(content: &str) -> MemoryType {
    let lowered = content.to_ascii_lowercase();
    if lowered.contains("prefer") || lowered.contains("preference") {
        MemoryType::Preference
    } else if lowered.contains("decided") || lowered.contains("decision") {
        MemoryType::Decision
    } else {
        MemoryType::Note
    }
}

fn infer_chat_sensitivity(content: &str) -> Sensitivity {
    let lowered = content.to_ascii_lowercase();
    if contains_any(
        &lowered,
        &[
            "api key",
            "access key",
            "access token",
            "auth token",
            "bearer token",
            "client secret",
            "password",
            "passphrase",
            "private key",
            "secret key",
            "ssh key",
            "ssn",
            "social security number",
            "recovery code",
        ],
    ) || looks_like_secret_token(content)
    {
        return Sensitivity::Secret;
    }

    if contains_any(
        &lowered,
        &[
            "bank account",
            "compensation",
            "credit card",
            "diagnosed",
            "diagnosis",
            "doctor",
            "driver license",
            "health insurance",
            "lawyer",
            "legal matter",
            "medical",
            "medication",
            "passport",
            "routing number",
            "salary",
            "tax return",
            "therapist",
            "therapy",
        ],
    ) {
        return Sensitivity::Sensitive;
    }

    Sensitivity::Normal
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn looks_like_secret_token(content: &str) -> bool {
    content.split_whitespace().any(|token| {
        let token =
            token.trim_matches(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')));
        let lowered = token.to_ascii_lowercase();
        lowered.starts_with("sk-")
            || lowered.starts_with("ghp_")
            || lowered.starts_with("xoxb-")
            || (lowered.starts_with("akia") && lowered.len() >= 16)
    })
}

fn title_from_memory_content(content: &str) -> String {
    content.trim().chars().take(80).collect()
}

fn extracted_proposal_to_candidate(
    validated: &ValidatedMemoryProposal,
    conversation_scope_id: &str,
    project_scope_id: Option<&str>,
    turn: &NewChatTurn,
    user_input: &str,
) -> Result<NewChatMemoryCandidate, String> {
    let proposal = &validated.proposal;
    let home_scope_id =
        home_scope_for_extracted_proposal(proposal, conversation_scope_id, project_scope_id);
    let mut candidate = NewChatMemoryCandidate::new(
        home_scope_id,
        proposal.content.clone(),
        "agent:memory_extractor",
    );
    candidate.memory_type = proposal.memory_type;
    candidate.title = proposal.title.clone();
    candidate.sensitivity = proposal.sensitivity;
    candidate.status = validated.status;
    candidate.confidence = Some(f64::from(proposal.confidence));
    candidate.retrieval_hints =
        serde_json::to_value(&proposal.retrieval_hints).map_err(|error| error.to_string())?;
    candidate.owner_principal_id = Some("human:local".to_string());
    candidate.authority_level = MemoryAuthorityLevel::AgentInference;
    candidate.extraction_method = MemoryExtractionMethod::LlmExtracted;
    candidate.source = Some(ChatMemorySource {
        conversation_id: conversation_scope_id.to_string(),
        message_id: Some(source_message_id_for_evidence(
            &proposal.evidence_excerpt,
            user_input,
            turn,
        )),
        evidence_excerpt: Some(proposal.evidence_excerpt.clone()),
    });
    candidate.participants = vec![
        NewMemoryParticipant::new("human:local", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];
    candidate.subjects = proposal
        .subjects
        .iter()
        .map(memory_extraction_subject_to_persistence)
        .collect();
    candidate.metadata = json!({
        "trigger": "ordinary_chat_extraction",
        "turn_index": turn.turn_index,
        "risk_flags": proposal.risk_flags,
    });

    Ok(candidate)
}

fn home_scope_for_extracted_proposal(
    proposal: &crate::memory_extraction::ExtractorMemoryProposal,
    conversation_scope_id: &str,
    project_scope_id: Option<&str>,
) -> String {
    if proposal
        .subjects
        .iter()
        .any(memory_extraction_subject_is_local_human)
        && matches!(
            proposal.memory_type,
            MemoryType::Fact | MemoryType::Preference
        )
    {
        return "human:local".to_string();
    }

    if matches!(
        proposal.memory_type,
        MemoryType::Decision
            | MemoryType::Procedure
            | MemoryType::Constraint
            | MemoryType::Project
            | MemoryType::Policy
    ) && let Some(project_scope_id) = project_scope_id
    {
        return project_scope_id.to_string();
    }

    conversation_scope_id.to_string()
}

fn source_message_id_for_evidence(
    evidence_excerpt: &str,
    user_input: &str,
    turn: &NewChatTurn,
) -> String {
    if user_input.contains(evidence_excerpt) {
        turn.user_message_id.clone()
    } else {
        turn.assistant_message_id.clone()
    }
}

fn memory_extraction_subject_to_persistence(subject: &MemoryExtractionSubject) -> NewMemorySubject {
    let entity_id = subject
        .id
        .clone()
        .unwrap_or_else(|| generated_entity_id(subject.kind, &subject.name));
    let mut stored = NewMemorySubject::new(
        entity_id,
        subject_kind_to_entity_type(subject.kind),
        subject.name.clone(),
        subject_role_to_memory_role(subject.role),
    );
    if memory_extraction_subject_is_local_human(subject) {
        stored.linked_principal_id = Some("human:local".to_string());
    }
    stored
}

fn memory_extraction_subject_is_local_human(subject: &MemoryExtractionSubject) -> bool {
    subject.id.as_deref().is_some_and(|id| id == "human:local")
        || matches!(
            subject.name.trim().to_ascii_lowercase().as_str(),
            "current human" | "user" | "me"
        )
}

fn generated_entity_id(kind: MemoryExtractionSubjectKind, name: &str) -> String {
    format!(
        "{}:{}",
        subject_kind_to_entity_type(kind),
        slug_fragment(name)
    )
}

fn subject_kind_to_entity_type(kind: MemoryExtractionSubjectKind) -> &'static str {
    match kind {
        MemoryExtractionSubjectKind::Human => "human",
        MemoryExtractionSubjectKind::Agent => "agent",
        MemoryExtractionSubjectKind::Conversation => "conversation",
        MemoryExtractionSubjectKind::Workspace => "workspace",
        MemoryExtractionSubjectKind::Project => "project",
        MemoryExtractionSubjectKind::Task => "task",
        MemoryExtractionSubjectKind::Cron => "other",
        MemoryExtractionSubjectKind::Relationship => "other",
        MemoryExtractionSubjectKind::Tool => "tool",
        MemoryExtractionSubjectKind::Organization => "organization",
        MemoryExtractionSubjectKind::Place => "place",
        MemoryExtractionSubjectKind::Concept => "concept",
        MemoryExtractionSubjectKind::Other => "other",
    }
}

fn subject_role_to_memory_role(role: MemoryExtractionSubjectRole) -> SubjectRole {
    match role {
        MemoryExtractionSubjectRole::About | MemoryExtractionSubjectRole::Participant => {
            SubjectRole::About
        }
        MemoryExtractionSubjectRole::Owner => SubjectRole::Owner,
        MemoryExtractionSubjectRole::Affected => SubjectRole::Affected,
        MemoryExtractionSubjectRole::Assignee => SubjectRole::Assignee,
        MemoryExtractionSubjectRole::Source => SubjectRole::Source,
        MemoryExtractionSubjectRole::Target => SubjectRole::Target,
    }
}

fn project_scope_from_cwd(cwd: Option<&str>) -> Option<String> {
    let cwd = cwd?.trim();
    if cwd.is_empty() {
        return None;
    }

    let path = Path::new(cwd);
    if !path.join(".git").exists() {
        return None;
    }

    let name = path.file_name()?.to_string_lossy();
    Some(format!("project:{}", slug_fragment(&name)))
}

fn slug_fragment(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        "unknown".to_string()
    } else {
        slug
    }
}

fn memory_activity(
    id: &str,
    status: TurnActivityStatus,
    title: &str,
    summary: Option<&str>,
    metadata: serde_json::Value,
) -> TurnTranscriptItem {
    TurnTranscriptItem::Activity {
        id: id.to_string(),
        activity_kind: "memory_extraction".to_string(),
        status,
        title: title.to_string(),
        summary: summary.map(ToOwned::to_owned),
        metadata,
    }
}

fn memory_activity_failed(id: &str, message: String) -> TurnTranscriptItem {
    memory_activity(
        id,
        TurnActivityStatus::Failed,
        "Memory extraction failed",
        Some(&message),
        json!({}),
    )
}

/// Return whether a daemon connection failure means no daemon is listening.
#[must_use]
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

        let response = DaemonResponse::TurnTranscriptItem {
            conversation_id: "conversation_1".to_string(),
            item: TurnTranscriptItem::Activity {
                id: "memory_extraction:conversation_1:1".to_string(),
                activity_kind: "memory_extraction".to_string(),
                status: TurnActivityStatus::Started,
                title: "Extracting memory proposals".to_string(),
                summary: Some("ordinary chat memory extraction is running".to_string()),
                metadata: json!({ "turn_index": 1 }),
            },
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
        let dir = tempfile::tempdir().expect("temp dir");
        let handle = CodexRuntimeHandle::spawn(
            CodexProviderConfig {
                command: script.to_string_lossy().to_string(),
                startup_timeout_seconds: 2,
                turn_timeout_seconds: 2,
                ..CodexProviderConfig::default()
            },
            dir.path().join("db").join("noema.sqlite"),
        )
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

        let items = collect_turn(&handle, first.conversation_id.clone(), "hello".to_string())
            .await
            .expect("turn response");
        assert_eq!(assistant_text(&items), "fake answer");

        handle.shutdown().await;
    }

    #[test]
    fn explicit_memory_parser_accepts_only_top_level_commands() {
        assert_eq!(
            explicit_memory_content("remember this: Kevin prefers inspectable memory").as_deref(),
            Some("Kevin prefers inspectable memory")
        );
        assert_eq!(
            explicit_memory_content(" remember that: project decisions belong to projects ")
                .as_deref(),
            Some("project decisions belong to projects")
        );
        assert_eq!(
            explicit_memory_content("/remember Kevin likes concise inspection output").as_deref(),
            Some("Kevin likes concise inspection output")
        );
        assert_eq!(
            explicit_memory_content("/remember: Kevin likes durable memory").as_deref(),
            Some("Kevin likes durable memory")
        );
        assert_eq!(explicit_memory_content("hello remember this: nope"), None);
        assert_eq!(explicit_memory_content("> remember this: quoted"), None);
        assert_eq!(explicit_memory_content("don't remember this: nope"), None);
        assert_eq!(explicit_memory_content("remember this:"), None);
    }

    #[test]
    fn deterministic_sensitivity_classifier_fails_closed_for_common_secrets() {
        assert_eq!(
            infer_chat_sensitivity("my API key is sk-test1234567890"),
            Sensitivity::Secret
        );
        assert_eq!(
            infer_chat_sensitivity("my doctor diagnosed this last week"),
            Sensitivity::Sensitive
        );
        assert_eq!(
            infer_chat_sensitivity("Kevin prefers CLI memory inspection"),
            Sensitivity::Normal
        );
    }

    #[tokio::test]
    async fn runtime_actor_persists_explicit_remember_confirmed() {
        let script = fake_codex_app_server_script_with_memory_extraction();
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("db").join("noema.sqlite");
        let handle = CodexRuntimeHandle::spawn(
            CodexProviderConfig {
                command: script.to_string_lossy().to_string(),
                startup_timeout_seconds: 2,
                turn_timeout_seconds: 2,
                ..CodexProviderConfig::default()
            },
            db_path.clone(),
        )
        .expect("runtime");

        let conversation = handle
            .start_conversation(None, None)
            .await
            .expect("conversation");
        let items = collect_turn(
            &handle,
            conversation.conversation_id.clone(),
            "remember this: Kevin prefers CLI memory inspection.".to_string(),
        )
        .await
        .expect("turn");
        assert_eq!(assistant_text(&items), "fake answer");
        assert!(
            !items
                .iter()
                .any(|item| matches!(item, TurnTranscriptItem::Activity { .. })),
            "explicit memory should not trigger automatic extraction activity: {items:?}"
        );
        handle.shutdown().await;

        let repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");
        let memories = repo.list_recent_memories(Some(10)).expect("memories");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].content, "Kevin prefers CLI memory inspection.");
        assert_eq!(memories[0].status, crate::memory::MemoryStatus::Confirmed);
        assert_eq!(memories[0].memory_type, MemoryType::Preference);
        assert_eq!(
            memories[0].conversation_id.as_deref(),
            Some("conversation:conversation_1")
        );

        let conn = rusqlite::Connection::open(&db_path).expect("raw conn");
        let message_provenance_count: i64 = conn
            .query_row(
                r"
                SELECT COUNT(*)
                FROM memory_provenance_edges pe
                JOIN messages m ON m.message_id = pe.source_id
                WHERE pe.memory_id = ?1 AND pe.source_type = 'message'
                ",
                rusqlite::params![memories[0].id],
                |row| row.get(0),
            )
            .expect("message provenance count");
        assert_eq!(message_provenance_count, 1);
    }

    #[tokio::test]
    async fn runtime_actor_extracts_ordinary_chat_memory_as_activity() {
        let script = fake_codex_app_server_script_with_memory_extraction();
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("db").join("noema.sqlite");
        let handle = CodexRuntimeHandle::spawn(
            CodexProviderConfig {
                command: script.to_string_lossy().to_string(),
                startup_timeout_seconds: 2,
                turn_timeout_seconds: 2,
                ..CodexProviderConfig::default()
            },
            db_path.clone(),
        )
        .expect("runtime");

        let conversation = handle
            .start_conversation(None, None)
            .await
            .expect("conversation");
        let items = collect_turn(
            &handle,
            conversation.conversation_id,
            "I prefer automatic memory extraction in chat.".to_string(),
        )
        .await
        .expect("turn");
        assert_eq!(assistant_text(&items), "fake answer");
        assert!(items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Started,
                    title,
                    ..
                } if activity_kind == "memory_extraction" && title == "Extracting memory proposals"
            )
        }));
        assert!(items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    summary: Some(summary),
                    ..
                } if activity_kind == "memory_extraction" && summary == "created 1 memory candidate"
            )
        }));
        handle.shutdown().await;

        let repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");
        let memories = repo.list_recent_memories(Some(10)).expect("memories");
        assert_eq!(memories.len(), 1);
        assert_eq!(
            memories[0].content,
            "Kevin prefers automatic memory extraction in chat."
        );
        assert_eq!(memories[0].status, crate::memory::MemoryStatus::Active);
        assert_eq!(memories[0].memory_type, MemoryType::Preference);
        assert_eq!(
            memories[0].conversation_id.as_deref(),
            Some("conversation:conversation_1")
        );
    }

    #[tokio::test]
    async fn runtime_actor_keeps_third_party_subject_conversation_scoped() {
        let script = fake_codex_app_server_script_with_memory_extraction();
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("db").join("noema.sqlite");
        let handle = CodexRuntimeHandle::spawn(
            CodexProviderConfig {
                command: script.to_string_lossy().to_string(),
                startup_timeout_seconds: 2,
                turn_timeout_seconds: 2,
                ..CodexProviderConfig::default()
            },
            db_path.clone(),
        )
        .expect("runtime");

        let conversation = handle
            .start_conversation(None, None)
            .await
            .expect("conversation");
        let items = collect_turn(
            &handle,
            conversation.conversation_id,
            "Alice prefers decaf.".to_string(),
        )
        .await
        .expect("turn");
        assert_eq!(assistant_text(&items), "fake answer");
        handle.shutdown().await;

        let repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");
        let memories = repo.list_recent_memories(Some(10)).expect("memories");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].content, "Alice prefers decaf.");
        assert_eq!(memories[0].status, crate::memory::MemoryStatus::Candidate);
        assert_eq!(memories[0].home_scope_id, "conversation:conversation_1");

        let conn = rusqlite::Connection::open(&db_path).expect("raw conn");
        let linked_principal_id: Option<String> = conn
            .query_row(
                "SELECT linked_principal_id FROM entities WHERE entity_id = 'human:alice'",
                [],
                |row| row.get(0),
            )
            .expect("Alice entity");
        assert_eq!(linked_principal_id, None);
    }

    #[tokio::test]
    async fn runtime_actor_persists_explicit_remember_before_provider_failure() {
        let script = fake_codex_app_server_script_with_turn_error();
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("db").join("noema.sqlite");
        let handle = CodexRuntimeHandle::spawn(
            CodexProviderConfig {
                command: script.to_string_lossy().to_string(),
                startup_timeout_seconds: 2,
                turn_timeout_seconds: 2,
                ..CodexProviderConfig::default()
            },
            db_path.clone(),
        )
        .expect("runtime");

        let conversation = handle
            .start_conversation(None, None)
            .await
            .expect("conversation");
        let error = collect_turn(
            &handle,
            conversation.conversation_id,
            "/remember Kevin wants failed turns to keep explicit memory.".to_string(),
        )
        .await
        .expect_err("provider error");
        assert!(matches!(error, DaemonError::Provider(_)));
        handle.shutdown().await;

        let repo = SqliteMemoryRepository::open_at(db_path).expect("repo");
        let memories = repo.list_recent_memories(Some(10)).expect("memories");
        assert_eq!(memories.len(), 1);
        assert_eq!(
            memories[0].content,
            "Kevin wants failed turns to keep explicit memory."
        );
        assert_eq!(memories[0].status, crate::memory::MemoryStatus::Confirmed);
    }

    async fn collect_turn(
        handle: &CodexRuntimeHandle,
        conversation_id: String,
        input: String,
    ) -> Result<Vec<TurnTranscriptItem>, DaemonError> {
        let (item_tx, mut item_rx) = mpsc::unbounded_channel();
        handle.turn(conversation_id, input, item_tx).await?;
        let mut items = Vec::new();
        while let Ok(item) = item_rx.try_recv() {
            items.push(item);
        }
        Ok(items)
    }

    fn fake_codex_app_server_script_with_turn_error() -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("temp dir").keep();
        let path = dir.join("fake-codex-error");
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
        print(json.dumps({"id": msg["id"], "error": {"code": -32000, "message": "turn failed"}}), flush=True)
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

    fn assistant_text(items: &[TurnTranscriptItem]) -> &str {
        let Some(TurnTranscriptItem::AssistantText { text }) = items.first() else {
            panic!("expected assistant text item, got {items:?}");
        };
        text
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
        if "model" in msg.get("params", {}):
            print(json.dumps({"id": msg["id"], "error": {"code": -32602, "message": "model should be omitted by default"}}), flush=True)
            continue
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

    fn fake_codex_app_server_script_with_memory_extraction() -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("temp dir").keep();
        let path = dir.join("fake-codex-memory-extraction");
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
        input_text = "".join(
            part.get("text", "")
            for part in msg.get("params", {}).get("input", [])
            if part.get("type") == "text"
        )
        if "ordinary-chat memory proposal extractor" in input_text:
            if "Alice prefers decaf." in input_text:
                proposal = {
                    "content": "Alice prefers decaf.",
                    "memory_type": "preference",
                    "title": "Alice decaf preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [
                        {
                            "id": None,
                            "kind": "human",
                            "name": "Alice",
                            "role": "about"
                        }
                    ],
                    "retrieval_hints": {
                        "topics": ["people"],
                        "keywords": ["Alice", "decaf"],
                        "summary": "Alice prefers decaf."
                    },
                    "risk_flags": [],
                    "evidence_excerpt": "Alice prefers decaf."
                }
            else:
                proposal = {
                    "content": "Kevin prefers automatic memory extraction in chat.",
                    "memory_type": "preference",
                    "title": "Automatic memory extraction preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [
                        {
                            "id": "human:local",
                            "kind": "human",
                            "name": "Kevin",
                            "role": "about"
                        }
                    ],
                    "retrieval_hints": {
                        "topics": ["memory"],
                        "keywords": ["automatic memory extraction", "chat"],
                        "summary": "Kevin prefers automatic memory extraction in chat."
                    },
                    "risk_flags": [],
                    "evidence_excerpt": "I prefer automatic memory extraction in chat."
                }
            text = json.dumps({"proposals": [proposal]})
        else:
            text = "fake answer"
        print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
        print(json.dumps({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": text}}}), flush=True)
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
