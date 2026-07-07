use std::{collections::HashMap, sync::Arc};

use crate::{NoemaStore, SystemErrorLogger};
use tokio::sync::mpsc;

use super::handle::{CodexRuntimeCommand, RuntimeModelProvider};
use crate::daemon::protocol::DaemonError;

#[derive(Debug)]
pub(in crate::daemon) struct CodexRuntimeActor {
    pub(in crate::daemon) default_provider_kind: String,
    pub(in crate::daemon) providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
    pub(in crate::daemon) store: NoemaStore,
    pub(in crate::daemon) system_errors: SystemErrorLogger,
    pub(in crate::daemon) search_provider: crate::search::types::SearchRuntimeProvider,
    pub(in crate::daemon) web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider,
    pub(in crate::daemon) conversations: HashMap<String, ActiveConversation>,
}

impl CodexRuntimeActor {
    pub(in crate::daemon) async fn new(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            default_provider_kind,
            providers,
            store,
            system_errors,
            search_provider: crate::search::types::SearchRuntimeProvider::default(),
            web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider::default(),
            conversations: HashMap::new(),
        })
    }

    #[cfg(test)]
    pub(in crate::daemon) async fn new_with_search_provider(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
        search_provider: crate::search::types::SearchRuntimeProvider,
    ) -> Result<Self, DaemonError> {
        let mut actor = Self::new(default_provider_kind, providers, store, system_errors).await?;
        actor.search_provider = search_provider;
        Ok(actor)
    }

    #[cfg(test)]
    pub(in crate::daemon) async fn new_with_search_and_fetch_provider(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
        search_provider: crate::search::types::SearchRuntimeProvider,
        web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider,
    ) -> Result<Self, DaemonError> {
        let mut actor = Self::new_with_search_provider(
            default_provider_kind,
            providers,
            store,
            system_errors,
            search_provider,
        )
        .await?;
        actor.web_fetch_provider = web_fetch_provider;
        Ok(actor)
    }

    pub(in crate::daemon) fn provider_for_kind(
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

    pub(in crate::daemon) fn default_provider(
        &self,
    ) -> Result<Arc<dyn RuntimeModelProvider>, DaemonError> {
        self.provider_for_kind(&self.default_provider_kind)
    }

    #[allow(dead_code)]
    pub(in crate::daemon) async fn resolved_web_search_provider(
        &self,
    ) -> Result<super::web_tools::ResolvedWebSearchProvider, crate::StoreError> {
        super::web_tools::resolve_web_search_provider(&self.store).await
    }

    #[allow(dead_code)]
    pub(in crate::daemon) async fn resolved_web_fetch_provider(
        &self,
    ) -> Result<super::web_tools::ResolvedWebFetchProvider, crate::StoreError> {
        super::web_tools::resolve_web_fetch_provider(&self.store).await
    }

    pub(super) async fn run(mut self, mut receiver: mpsc::Receiver<CodexRuntimeCommand>) {
        while let Some(command) = receiver.recv().await {
            match command {
                #[cfg(test)]
                CodexRuntimeCommand::StartConversation { cwd, reply } => {
                    let _ = reply.send(self.start_conversation(cwd).await);
                }
                CodexRuntimeCommand::StartPrimaryConversation { cwd, reply } => {
                    let _ = reply.send(self.start_primary_conversation(cwd).await);
                }
                CodexRuntimeCommand::Turn {
                    conversation_id,
                    input,
                    item_tx,
                    client_message_id,
                    reply,
                } => {
                    let _ = reply.send(
                        self.turn(conversation_id, input, item_tx, client_message_id)
                            .await,
                    );
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
                CodexRuntimeCommand::Shutdown { reply } => {
                    let _ = reply.send(());
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::daemon) struct ActiveConversation {
    pub(in crate::daemon) provider_kind: String,
    pub(in crate::daemon) model: Option<String>,
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) next_turn_index: u64,
}
