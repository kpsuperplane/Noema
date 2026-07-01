use std::{collections::HashMap, sync::Arc};

use crate::NoemaStore;
use tokio::sync::mpsc;

use super::handle::{CodexRuntimeCommand, RuntimeModelProvider};
use crate::daemon::protocol::DaemonError;

#[derive(Debug)]
pub(super) struct CodexRuntimeActor {
    pub(super) default_provider_kind: String,
    pub(super) providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
    pub(super) store: NoemaStore,
    pub(super) conversations: HashMap<String, ActiveConversation>,
}

impl CodexRuntimeActor {
    pub(super) async fn new(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            default_provider_kind,
            providers,
            store,
            conversations: HashMap::new(),
        })
    }

    pub(super) fn provider_for_kind(
        &self,
        provider_kind: &str,
    ) -> Result<Arc<dyn RuntimeModelProvider>, DaemonError> {
        self.providers.get(provider_kind).cloned().ok_or_else(|| {
            DaemonError::Provider(crate::ProviderError::ProviderUnavailable {
                provider: provider_kind.to_string(),
                message: "provider is not available in this daemon".to_string(),
            })
        })
    }

    pub(super) fn default_provider(&self) -> Result<Arc<dyn RuntimeModelProvider>, DaemonError> {
        self.provider_for_kind(&self.default_provider_kind)
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
                    let provider = match self.default_provider() {
                        Ok(provider) => provider,
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            continue;
                        }
                    };
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
    pub(super) provider_kind: String,
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
