use std::{collections::HashMap, sync::Arc};

use noema_home::SystemErrorLogger;
#[cfg(test)]
use noema_providers::ProviderError;
use noema_providers::{
    CodexProviderConfig, DEFAULT_FOUNDATION_LOCAL_PROFILE, FoundationLocalProviderConfig,
    GenerateRequest, GenerateResponse, ProviderConfig, ProviderCredentialAccessHandle,
    ProviderHandle, hosted_provider_from_config, provider_bootstrap_from_config,
};
#[cfg(test)]
use noema_store::NoemaStore;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::{CodexRuntimeSpawnConfig, TaskCompletionDeliveryRequest, actor::CodexRuntimeActor};
use crate::daemon::protocol::{DaemonError, StartedConversation, TurnStreamEvent};

pub(crate) type RuntimeProviderMap = HashMap<String, ProviderHandle>;
type ConfiguredRuntimeProviderMap = (String, Option<String>, RuntimeProviderMap);

#[derive(Debug, Clone)]
pub(crate) struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
    cancellation: Arc<RuntimeCancellation>,
}

#[derive(Debug)]
struct RuntimeCancellation(CancellationToken);

impl Drop for RuntimeCancellation {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl CodexRuntimeHandle {
    pub(crate) fn provider_map_from_config(
        provider_config: ProviderConfig,
        system_errors: SystemErrorLogger,
        provider_credentials: ProviderCredentialAccessHandle,
    ) -> Result<ConfiguredRuntimeProviderMap, DaemonError> {
        let bootstrap = provider_bootstrap_from_config(
            provider_config,
            provider_credentials.clone(),
            system_errors.clone(),
        )?;
        let default_provider_kind = bootstrap.default_provider_kind().to_string();
        let default_model_profile = bootstrap.default_model_profile().map(str::to_string);
        let mut providers = HashMap::new();
        if let Some((provider_kind, provider)) = bootstrap.into_hosted_provider() {
            providers.insert(provider_kind, provider);
        }
        if !providers.contains_key("codex") {
            let (provider_kind, provider) = hosted_provider_from_config(
                ProviderConfig::Codex(CodexProviderConfig::default()),
                provider_credentials.clone(),
                system_errors.clone(),
            )?;
            providers.insert(provider_kind, provider);
        }
        if !providers.contains_key("foundation_local") {
            let (provider_kind, provider) = hosted_provider_from_config(
                ProviderConfig::FoundationLocal(default_foundation_local_config()),
                provider_credentials,
                system_errors,
            )?;
            providers.insert(provider_kind, provider);
        }
        Ok((default_provider_kind, default_model_profile, providers))
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_map_and_memory(
        default_provider_kind: String,
        providers: RuntimeProviderMap,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        Self::spawn_with_provider_map_inner(
            default_provider_kind,
            providers,
            store,
            artifact_operations,
            system_errors,
            memory_operations,
            task_subscriptions,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_registry_and_memory(
        provider_registry: noema_providers::ProviderRegistryHandle,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        use noema_providers::RegistryProviderRouteResolver;

        let bind = |loader| -> noema_providers::ProviderRouteResolverHandle {
            Arc::new(RegistryProviderRouteResolver::new(
                loader,
                Arc::clone(&provider_registry),
            ))
        };
        let provider_accounts = super::actor::test_provider_account_access(&store)?;
        let (capability_bindings, capability_invokers) = super::actor::test_capability_handles();
        Self::spawn(CodexRuntimeSpawnConfig {
            primary_provider: bind(store.agent_provider_selection_loader("agent:primary")),
            default_provider: bind(store.default_provider_selection_loader()),
            progress_audit_provider: bind(
                store.auxiliary_provider_selection_loader(noema_store::TOOL_PROGRESS_AUDIT_TASK_ID),
            ),
            web_summary_provider: bind(
                store
                    .auxiliary_provider_selection_loader(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID),
            ),
            provider_registry,
            store,
            artifact_operations,
            system_errors,
            memory_operations,
            task_subscriptions,
            provider_accounts,
            capability_bindings,
            capability_invokers,
        })
        .await
    }

    pub(crate) async fn spawn(config: CodexRuntimeSpawnConfig) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::from_spawn_config(config).await?;
        let cancellation = Arc::new(RuntimeCancellation(actor.tasks.cancellation_token()));
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
        })
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider(
        provider: ProviderHandle,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        Self::spawn_with_provider_kind(provider, store, "codex").await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_and_memory(
        provider: ProviderHandle,
        store: NoemaStore,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
    ) -> Result<Self, DaemonError> {
        let provider_kind = "codex".to_string();
        let system_errors = crate::test_support::system_error_logger();
        let artifact_operations =
            crate::test_support::artifact_operations(&store).map_err(DaemonError::Protocol)?;
        Self::spawn_with_provider_map_inner(
            provider_kind.clone(),
            HashMap::from([(provider_kind, provider)]),
            store,
            artifact_operations,
            system_errors,
            memory_operations,
            crate::graphql::ConversationSubscriptionRegistry::default(),
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_kind(
        provider: ProviderHandle,
        store: NoemaStore,
        provider_kind: impl Into<String>,
    ) -> Result<Self, DaemonError> {
        let provider_kind = provider_kind.into();
        Self::spawn_with_provider_map(
            provider_kind.clone(),
            HashMap::from([(provider_kind, provider)]),
            store,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_and_search_provider(
        provider: ProviderHandle,
        store: NoemaStore,
        search_provider: crate::search::types::SearchRuntimeProvider,
    ) -> Result<Self, DaemonError> {
        let provider_kind = "codex".to_string();
        let system_errors = crate::test_support::system_error_logger();
        let providers = HashMap::from([(provider_kind.clone(), provider)]);
        if !providers.contains_key(&provider_kind) {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new_with_search_provider(
            provider_kind.clone(),
            providers,
            store,
            system_errors,
            search_provider,
        )
        .await?;
        let cancellation = Arc::new(RuntimeCancellation(actor.tasks.cancellation_token()));
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
        })
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_and_search_fetch_providers(
        provider: ProviderHandle,
        store: NoemaStore,
        search_provider: crate::search::types::SearchRuntimeProvider,
        web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider,
    ) -> Result<Self, DaemonError> {
        let provider_kind = "codex".to_string();
        let system_errors = crate::test_support::system_error_logger();
        let providers = HashMap::from([(provider_kind.clone(), provider)]);
        if !providers.contains_key(&provider_kind) {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new_with_search_and_fetch_provider(
            provider_kind.clone(),
            providers,
            store,
            system_errors,
            search_provider,
            web_fetch_provider,
        )
        .await?;
        let cancellation = Arc::new(RuntimeCancellation(actor.tasks.cancellation_token()));
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
        })
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_map<I>(
        default_provider_kind: impl Into<String>,
        providers: I,
        store: NoemaStore,
    ) -> Result<Self, DaemonError>
    where
        I: IntoIterator<Item = (String, ProviderHandle)>,
    {
        let system_errors = crate::test_support::system_error_logger();
        let artifact_operations =
            crate::test_support::artifact_operations(&store).map_err(DaemonError::Protocol)?;
        Self::spawn_with_provider_map_inner(
            default_provider_kind.into(),
            providers.into_iter().collect(),
            store,
            artifact_operations,
            system_errors,
            None,
            crate::graphql::ConversationSubscriptionRegistry::default(),
        )
        .await
    }

    #[cfg(test)]
    async fn spawn_with_provider_map_inner(
        default_provider_kind: String,
        providers: RuntimeProviderMap,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        if !providers.contains_key(&default_provider_kind) {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: default_provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let routing =
            super::actor::test_provider_routing(&store, &default_provider_kind, providers).await?;
        let provider_accounts = super::actor::test_provider_account_access(&store)?;
        let (capability_bindings, capability_invokers) = super::actor::test_capability_handles();
        Self::spawn(CodexRuntimeSpawnConfig {
            primary_provider: routing.primary,
            default_provider: routing.default,
            progress_audit_provider: routing.progress_audit,
            web_summary_provider: routing.web_summary,
            provider_registry: routing.registry,
            store,
            artifact_operations,
            system_errors,
            memory_operations,
            task_subscriptions,
            provider_accounts,
            capability_bindings,
            capability_invokers,
        })
        .await
    }

    #[cfg(test)]
    pub(crate) async fn start_conversation(
        &self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::StartConversation { cwd, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn start_primary_conversation(
        &self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::StartPrimaryConversation { cwd, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    #[cfg(test)]
    pub(crate) async fn turn(
        &self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.turn_with_client_message_id(conversation_id, input, item_tx, None)
            .await
    }

    pub(crate) async fn turn_with_client_message_id(
        &self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::Turn {
                conversation_id,
                input,
                item_tx,
                client_message_id,
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn select_multiple_choice_with_client_message_id(
        &self,
        conversation_id: String,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::SelectMultipleChoice {
                conversation_id,
                prompt_item_id,
                selected_option_ids,
                item_tx,
                client_message_id,
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    #[cfg(test)]
    pub(crate) async fn generate_once(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        self.send_generate_once(
            GenerateOnceRoute::Default,
            request,
            GenerateOnceModelPolicy::Selection,
        )
        .await
    }

    pub(crate) async fn generate_once_with_memory_provider(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        self.send_generate_once(
            GenerateOnceRoute::Memory,
            request,
            GenerateOnceModelPolicy::Selection,
        )
        .await
    }

    pub(crate) async fn generate_once_with_tool_classification_model(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        self.send_generate_once(
            GenerateOnceRoute::Default,
            request,
            GenerateOnceModelPolicy::ProviderToolClassification,
        )
        .await
    }

    async fn send_generate_once(
        &self,
        route: GenerateOnceRoute,
        request: GenerateRequest,
        model_policy: GenerateOnceModelPolicy,
    ) -> Result<GenerateResponse, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::GenerateOnce {
                route,
                request,
                model_policy,
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn generate_background_task(
        &self,
        request: super::BackgroundTaskGenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::BackgroundTask { request, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    /// Queue a primary-agent completion report behind any active foreground
    /// turn for the originating conversation.
    pub(crate) async fn deliver_task_completion(
        &self,
        request: TaskCompletionDeliveryRequest,
    ) -> Result<(), DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::TaskCompletionDelivery { request, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn shutdown(&self) {
        self.cancellation.0.cancel();
        let (reply, reply_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(CodexRuntimeCommand::Shutdown { reply })
            .await;
        let _ = reply_rx.await;
    }
}

fn default_foundation_local_config() -> FoundationLocalProviderConfig {
    FoundationLocalProviderConfig {
        default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
        bridge_path: None,
        system_errors: None,
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum GenerateOnceModelPolicy {
    Selection,
    ProviderToolClassification,
}

#[derive(Debug)]
pub(super) enum GenerateOnceRoute {
    Default,
    Memory,
}

#[derive(Debug)]
pub(super) enum CodexRuntimeCommand {
    #[cfg(test)]
    StartConversation {
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
    },
    StartPrimaryConversation {
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
    },
    Turn {
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    SelectMultipleChoice {
        conversation_id: String,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    GenerateOnce {
        route: GenerateOnceRoute,
        request: GenerateRequest,
        model_policy: GenerateOnceModelPolicy,
        reply: oneshot::Sender<Result<GenerateResponse, DaemonError>>,
    },
    BackgroundTask {
        request: super::BackgroundTaskGenerateRequest,
        reply: oneshot::Sender<Result<GenerateResponse, DaemonError>>,
    },
    TaskCompletionDelivery {
        request: TaskCompletionDeliveryRequest,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}
