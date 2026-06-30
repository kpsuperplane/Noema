use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    NoemaStore,
    provider::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider, ProviderError,
    },
    providers::codex_responses::{CodexProviderConfig, CodexResponsesProvider},
};
use tokio::sync::{mpsc, oneshot};

use super::actor::CodexRuntimeActor;
use crate::daemon::protocol::{DaemonError, StartedConversation, TurnStreamEvent};

pub(crate) trait RuntimeModelProvider: std::fmt::Debug + Send + Sync {
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>;
}

impl<T> RuntimeModelProvider for T
where
    T: ModelProvider + std::fmt::Debug + Send + Sync,
{
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move { ModelProvider::generate_streaming(self, request, on_event).await })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
}

impl CodexRuntimeHandle {
    pub(crate) async fn spawn(
        mut codex_config: CodexProviderConfig,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        let paths = crate::NoemaPaths::from_process_env()?;
        let account_home = paths.provider_account_home("codex", "default");
        crate::provider_auth::ensure_provider_account_home(&account_home)?;
        apply_provider_account_home(&mut codex_config, &account_home);

        let provider = Arc::new(CodexResponsesProvider::new(codex_config)?);
        Self::spawn_with_provider(provider, store).await
    }

    pub(crate) async fn spawn_with_provider(
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new(provider, store).await?;
        tokio::spawn(actor.run(receiver));
        Ok(Self { sender })
    }

    pub(crate) async fn start_conversation(
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

    pub(crate) async fn start_primary_conversation(
        &self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::StartPrimaryConversation { model, cwd, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn turn(
        &self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
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

    pub(crate) async fn end_conversation(
        &self,
        conversation_id: String,
    ) -> Result<(), DaemonError> {
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

    pub(crate) async fn shutdown(&self) {
        let (reply, reply_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(CodexRuntimeCommand::Shutdown { reply })
            .await;
        let _ = reply_rx.await;
    }
}

fn apply_provider_account_home(
    config: &mut crate::CodexProviderConfig,
    account_home: &std::path::Path,
) {
    config.account_home = Some(account_home.to_path_buf());
}

#[derive(Debug)]
pub(super) enum CodexRuntimeCommand {
    StartConversation {
        model: Option<String>,
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
    },
    StartPrimaryConversation {
        model: Option<String>,
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
    },
    Turn {
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_config_for_provider_account_uses_account_home() {
        let account_home = std::path::PathBuf::from("/noema/providers/codex/default");
        let mut config = CodexProviderConfig::default();

        apply_provider_account_home(&mut config, &account_home);

        assert_eq!(config.account_home.as_deref(), Some(account_home.as_path()));
    }
}
