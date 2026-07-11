use std::{collections::HashMap, sync::Arc};

use crate::{NoemaStore, SystemErrorLogger};
use tokio::sync::mpsc;

use super::handle::{CodexRuntimeCommand, RuntimeModelProvider};
use super::tasks::RuntimeTaskGroup;
use crate::daemon::protocol::DaemonError;

#[derive(Debug)]
pub(in crate::daemon) struct CodexRuntimeActor {
    pub(in crate::daemon) default_provider_kind: String,
    pub(in crate::daemon) providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
    pub(in crate::daemon) store: NoemaStore,
    pub(in crate::daemon) system_errors: SystemErrorLogger,
    pub(in crate::daemon) memory_connection: Option<crate::MnemosyneConnection>,
    pub(in crate::daemon) search_provider: crate::search::types::SearchRuntimeProvider,
    pub(in crate::daemon) web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider,
    pub(in crate::daemon) conversations: HashMap<String, ActiveConversation>,
    pub(super) tasks: RuntimeTaskGroup,
    pub(super) task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
}

impl CodexRuntimeActor {
    #[cfg(test)]
    pub(in crate::daemon) async fn new(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
    ) -> Result<Self, DaemonError> {
        Self::new_with_memory(
            default_provider_kind,
            providers,
            store,
            system_errors,
            None,
            crate::graphql::ConversationSubscriptionRegistry::default(),
        )
        .await
    }

    pub(in crate::daemon) async fn new_with_memory(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
        memory_connection: Option<crate::MnemosyneConnection>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            default_provider_kind,
            providers,
            store,
            system_errors,
            memory_connection,
            search_provider: crate::search::types::SearchRuntimeProvider::default(),
            web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider::default(),
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            task_subscriptions,
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

    pub(super) fn clone_for_background(&self) -> Self {
        Self {
            default_provider_kind: self.default_provider_kind.clone(),
            providers: self.providers.clone(),
            store: self.store.clone(),
            system_errors: self.system_errors.clone(),
            memory_connection: self.memory_connection.clone(),
            search_provider: self.search_provider.clone(),
            web_fetch_provider: self.web_fetch_provider.clone(),
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            task_subscriptions: self.task_subscriptions.clone(),
        }
    }

    pub(in crate::daemon) fn memory_client(&self) -> Option<crate::MnemosyneClient> {
        self.memory_connection.as_ref().map(|connection| {
            crate::MnemosyneClient::new(connection.base_url.clone(), connection.api_key.clone())
        })
    }

    #[allow(dead_code)]
    pub(in crate::daemon) async fn resolved_web_search_provider(
        &self,
    ) -> Result<super::web_tools::ResolvedWebProvider, crate::StoreError> {
        super::web_tools::resolve_web_search_provider(&self.store).await
    }

    #[allow(dead_code)]
    pub(in crate::daemon) async fn resolved_web_fetch_provider(
        &self,
    ) -> Result<super::web_tools::ResolvedWebProvider, crate::StoreError> {
        super::web_tools::resolve_web_fetch_provider(&self.store).await
    }

    pub(super) async fn run(mut self, mut receiver: mpsc::Receiver<CodexRuntimeCommand>) {
        let mut shutdown_reply = None;
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
                    let cancellation = self.tasks.cancellation_token();
                    let result = tokio::select! {
                        biased;
                        () = cancellation.cancelled() => match self
                            .store
                            .recover_shutdown_cancelled_work(&conversation_id)
                            .await
                        {
                            Ok(()) => Err(runtime_stopped()),
                            Err(error) => Err(error.into()),
                        },
                        result = self.turn(conversation_id.clone(), input, item_tx, client_message_id) => result,
                    };
                    let _ = reply.send(result);
                }
                CodexRuntimeCommand::SelectMultipleChoice {
                    conversation_id,
                    prompt_item_id,
                    selected_option_ids,
                    item_tx,
                    client_message_id,
                    reply,
                } => {
                    let cancellation = self.tasks.cancellation_token();
                    let result = tokio::select! {
                        biased;
                        () = cancellation.cancelled() => match self
                            .store
                            .recover_shutdown_cancelled_work(&conversation_id)
                            .await
                        {
                            Ok(()) => Err(runtime_stopped()),
                            Err(error) => Err(error.into()),
                        },
                        result = self.select_multiple_choice(
                            conversation_id.clone(),
                            prompt_item_id,
                            selected_option_ids,
                            item_tx,
                            client_message_id,
                        ) => result,
                    };
                    let _ = reply.send(result);
                }
                CodexRuntimeCommand::GenerateOnce {
                    provider_kind,
                    request,
                    reply,
                } => {
                    let provider = match provider_kind {
                        Some(provider_kind) => self.provider_for_kind(&provider_kind),
                        None => self.default_provider(),
                    };
                    let provider = match provider {
                        Ok(provider) => provider,
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            continue;
                        }
                    };
                    self.tasks.spawn(async move {
                        let mut ignore_event = |_| {};
                        let result = provider
                            .generate_streaming(request, &mut ignore_event)
                            .await
                            .map_err(DaemonError::Provider);
                        let _ = reply.send(result);
                    });
                }
                CodexRuntimeCommand::BackgroundTask { request, reply } => {
                    let actor = self.clone_for_background();
                    self.tasks.spawn(async move {
                        let result = actor.generate_background_task(request).await;
                        let _ = reply.send(result);
                    });
                }
                CodexRuntimeCommand::Shutdown { reply } => {
                    shutdown_reply = Some(reply);
                    break;
                }
            }
        }
        self.tasks.shutdown().await;
        if let Some(reply) = shutdown_reply {
            let _ = reply.send(());
        }
    }
}

fn runtime_stopped() -> DaemonError {
    DaemonError::Protocol("daemon runtime stopped".to_string())
}

#[derive(Debug, Clone)]
pub(in crate::daemon) struct ActiveConversation {
    pub(in crate::daemon) provider_kind: String,
    pub(in crate::daemon) model: Option<String>,
    pub(in crate::daemon) reasoning_effort: Option<crate::provider::ReasoningEffort>,
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) next_turn_index: u64,
}
