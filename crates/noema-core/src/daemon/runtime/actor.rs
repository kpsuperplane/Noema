use std::{collections::HashMap, sync::Arc};

use crate::NoemaStore;
use tokio::sync::mpsc;

use super::handle::{CodexRuntimeCommand, RuntimeModelProvider};
use crate::daemon::protocol::DaemonError;

#[derive(Debug)]
pub(super) struct CodexRuntimeActor {
    pub(super) provider: Arc<dyn RuntimeModelProvider>,
    pub(super) provider_kind: String,
    pub(super) store: NoemaStore,
    pub(super) conversations: HashMap<String, ActiveConversation>,
}

impl CodexRuntimeActor {
    pub(super) async fn new(
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
        provider_kind: String,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            provider,
            provider_kind,
            store,
            conversations: HashMap::new(),
        })
    }

    pub(super) async fn run(mut self, mut receiver: mpsc::Receiver<CodexRuntimeCommand>) {
        while let Some(command) = receiver.recv().await {
            match command {
                CodexRuntimeCommand::StartConversation { model, cwd, reply } => {
                    let _ = reply.send(self.start_conversation(model, cwd).await);
                }
                CodexRuntimeCommand::StartPrimaryConversation { model, cwd, reply } => {
                    let _ = reply.send(self.start_primary_conversation(model, cwd).await);
                }
                CodexRuntimeCommand::Turn {
                    conversation_id,
                    input,
                    item_tx,
                    reply,
                } => {
                    let _ = reply.send(self.turn(conversation_id, input, item_tx).await);
                }
                CodexRuntimeCommand::GenerateOnce { request, reply } => {
                    let provider = Arc::clone(&self.provider);
                    tokio::spawn(async move {
                        let mut ignore_event = |_| {};
                        let result = provider
                            .generate_streaming(request, &mut ignore_event)
                            .await
                            .map_err(DaemonError::Provider);
                        let _ = reply.send(result);
                    });
                }
                CodexRuntimeCommand::EndConversation {
                    conversation_id,
                    reply,
                } => {
                    self.conversations.remove(&conversation_id);
                    let _ = reply.send(Ok(()));
                }
                CodexRuntimeCommand::Shutdown { reply } => {
                    let _ = reply.send(());
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ActiveConversation {
    pub(super) model: Option<String>,
    pub(super) cwd: Option<String>,
    pub(super) next_turn_index: u64,
    pub(super) tool_snapshot: Option<CachedToolSnapshot>,
}

#[derive(Debug, Clone)]
pub(super) struct CachedToolSnapshot {
    pub(super) hash: u64,
    pub(super) rendered_tools: String,
}
