use std::path::Path;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
};

use super::protocol::{
    DaemonError, DaemonRequest, DaemonResponse, StartedConversation, TurnTranscriptItem,
};

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
