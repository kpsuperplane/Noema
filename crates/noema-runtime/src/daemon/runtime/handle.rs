use std::sync::Arc;

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use noema_home::SystemErrorLogger;
#[cfg(test)]
use noema_providers::ProviderError;
#[cfg(test)]
use noema_providers::ProviderHandle;
use noema_providers::{GenerateRequest, GenerateResponse};
#[cfg(test)]
use noema_store::NoemaStore;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::{RuntimeSpawnConfig, TaskCompletionDeliveryRequest, actor::RuntimeActor};
use crate::daemon::protocol::{RuntimeError, StartedConversation, TurnStreamEvent};

#[derive(Debug, Clone)]
/// Command handle for the governed foreground and background execution runtime.
pub struct RuntimeHandle {
    sender: mpsc::Sender<RuntimeCommand>,
    cancellation: Arc<RuntimeCancellation>,
}

#[derive(Debug)]
struct RuntimeCancellation(CancellationToken);

impl Drop for RuntimeCancellation {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl RuntimeHandle {
    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_map_and_memory(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
        runtime_events: crate::daemon::RuntimeEventRegistry,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_with_provider_map_inner(
            default_provider_kind,
            providers,
            store,
            artifact_operations,
            system_errors,
            memory_operations,
            runtime_events,
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
        runtime_events: crate::daemon::RuntimeEventRegistry,
    ) -> Result<Self, RuntimeError> {
        use noema_providers::RegistryProviderRouteResolver;

        let bind = |loader| -> noema_providers::ProviderRouteResolverHandle {
            Arc::new(RegistryProviderRouteResolver::new(
                loader,
                Arc::clone(&provider_registry),
            ))
        };
        let (capability_bindings, capability_invokers) = super::actor::test_capability_handles();
        Self::spawn(RuntimeSpawnConfig {
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
            runtime_events,
            web_backends: crate::test_support::web_backends(),
            capability_bindings,
            capability_invokers,
        })
        .await
    }

    /// Start the governed execution runtime.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured runtime dependencies cannot be initialized.
    pub async fn spawn(config: RuntimeSpawnConfig) -> Result<Self, RuntimeError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = RuntimeActor::from_spawn_config(config).await?;
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
    ) -> Result<Self, RuntimeError> {
        Self::spawn_with_provider_kind(provider, store, "codex").await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_and_memory(
        provider: ProviderHandle,
        store: NoemaStore,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
    ) -> Result<Self, RuntimeError> {
        let provider_kind = "codex".to_string();
        let system_errors = crate::test_support::system_error_logger();
        let artifact_operations =
            crate::test_support::artifact_operations(&store).map_err(RuntimeError::Protocol)?;
        Self::spawn_with_provider_map_inner(
            provider_kind.clone(),
            HashMap::from([(provider_kind, provider)]),
            store,
            artifact_operations,
            system_errors,
            memory_operations,
            crate::daemon::RuntimeEventRegistry::default(),
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_kind(
        provider: ProviderHandle,
        store: NoemaStore,
        provider_kind: impl Into<String>,
    ) -> Result<Self, RuntimeError> {
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
    ) -> Result<Self, RuntimeError> {
        let provider_kind = "codex".to_string();
        let system_errors = crate::test_support::system_error_logger();
        let providers = HashMap::from([(provider_kind.clone(), provider)]);
        if !providers.contains_key(&provider_kind) {
            return Err(RuntimeError::Provider(ProviderError::ProviderUnavailable {
                provider: provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let (sender, receiver) = mpsc::channel(16);
        let actor = RuntimeActor::new_with_search_provider(
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
    ) -> Result<Self, RuntimeError> {
        let provider_kind = "codex".to_string();
        let system_errors = crate::test_support::system_error_logger();
        let providers = HashMap::from([(provider_kind.clone(), provider)]);
        if !providers.contains_key(&provider_kind) {
            return Err(RuntimeError::Provider(ProviderError::ProviderUnavailable {
                provider: provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let (sender, receiver) = mpsc::channel(16);
        let actor = RuntimeActor::new_with_search_and_fetch_provider(
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
    ) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = (String, ProviderHandle)>,
    {
        let system_errors = crate::test_support::system_error_logger();
        let artifact_operations =
            crate::test_support::artifact_operations(&store).map_err(RuntimeError::Protocol)?;
        Self::spawn_with_provider_map_inner(
            default_provider_kind.into(),
            providers.into_iter().collect(),
            store,
            artifact_operations,
            system_errors,
            None,
            crate::daemon::RuntimeEventRegistry::default(),
        )
        .await
    }

    #[cfg(test)]
    async fn spawn_with_provider_map_inner(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
        runtime_events: crate::daemon::RuntimeEventRegistry,
    ) -> Result<Self, RuntimeError> {
        if !providers.contains_key(&default_provider_kind) {
            return Err(RuntimeError::Provider(ProviderError::ProviderUnavailable {
                provider: default_provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let routing =
            super::actor::test_provider_routing(&store, &default_provider_kind, providers).await?;
        let (capability_bindings, capability_invokers) = super::actor::test_capability_handles();
        Self::spawn(RuntimeSpawnConfig {
            primary_provider: routing.primary,
            default_provider: routing.default,
            progress_audit_provider: routing.progress_audit,
            web_summary_provider: routing.web_summary,
            provider_registry: routing.registry,
            store,
            artifact_operations,
            system_errors,
            memory_operations,
            runtime_events,
            web_backends: crate::test_support::web_backends(),
            capability_bindings,
            capability_invokers,
        })
        .await
    }

    #[cfg(test)]
    pub(crate) async fn start_conversation(
        &self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(RuntimeCommand::StartConversation { cwd, reply })
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?
    }

    /// Start or hydrate the primary agent's durable conversation.
    ///
    /// # Errors
    ///
    /// Returns an error when the runtime is stopped or conversation startup fails.
    pub async fn start_primary_conversation(
        &self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(RuntimeCommand::StartPrimaryConversation { cwd, reply })
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?
    }

    #[cfg(test)]
    pub(crate) async fn turn(
        &self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        self.turn_with_client_message_id(conversation_id, input, item_tx, None)
            .await
    }

    /// Execute one primary-conversation turn with an optional idempotency key.
    ///
    /// # Errors
    ///
    /// Returns an error when the runtime is stopped or turn execution fails.
    pub async fn turn_with_client_message_id(
        &self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(RuntimeCommand::Turn {
                conversation_id,
                input,
                item_tx,
                client_message_id,
                reply,
            })
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?
    }

    /// Submit a response to a durable multiple-choice prompt.
    ///
    /// # Errors
    ///
    /// Returns an error when the runtime is stopped, the prompt is invalid, or
    /// turn continuation fails.
    pub async fn select_multiple_choice_with_client_message_id(
        &self,
        conversation_id: String,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(RuntimeCommand::SelectMultipleChoice {
                conversation_id,
                prompt_item_id,
                selected_option_ids,
                item_tx,
                client_message_id,
                reply,
            })
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?
    }

    #[cfg(test)]
    pub(crate) async fn generate_once(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, RuntimeError> {
        self.send_generate_once(
            GenerateOnceRoute::Default,
            request,
            GenerateOnceModelPolicy::Selection,
        )
        .await
    }

    /// Generate one response through the current memory-provider route.
    ///
    /// # Errors
    ///
    /// Returns an error when route resolution or provider generation fails.
    pub async fn generate_once_with_memory_provider(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, RuntimeError> {
        self.send_generate_once(
            GenerateOnceRoute::Memory,
            request,
            GenerateOnceModelPolicy::Selection,
        )
        .await
    }

    /// Generate one response through the provider's classification-model policy.
    ///
    /// # Errors
    ///
    /// Returns an error when route resolution or provider generation fails.
    pub async fn generate_once_with_tool_classification_model(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, RuntimeError> {
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
    ) -> Result<GenerateResponse, RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(RuntimeCommand::GenerateOnce {
                route,
                request,
                model_policy,
                reply,
            })
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn generate_background_task(
        &self,
        request: super::BackgroundTaskGenerateRequest,
    ) -> Result<GenerateResponse, RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(RuntimeCommand::BackgroundTask { request, reply })
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?
    }

    /// Queue a primary-agent completion report behind any active foreground
    /// turn for the originating conversation.
    pub(crate) async fn deliver_task_completion(
        &self,
        request: TaskCompletionDeliveryRequest,
    ) -> Result<(), RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(RuntimeCommand::TaskCompletionDelivery { request, reply })
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Protocol("daemon runtime stopped".to_string()))?
    }

    /// Cancel active work and wait for the runtime actor to stop.
    pub async fn shutdown(&self) {
        self.cancellation.0.cancel();
        let (reply, reply_rx) = oneshot::channel();
        let _ = self.sender.send(RuntimeCommand::Shutdown { reply }).await;
        let _ = reply_rx.await;
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
pub(super) enum RuntimeCommand {
    #[cfg(test)]
    StartConversation {
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, RuntimeError>>,
    },
    StartPrimaryConversation {
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, RuntimeError>>,
    },
    Turn {
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
    SelectMultipleChoice {
        conversation_id: String,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
    GenerateOnce {
        route: GenerateOnceRoute,
        request: GenerateRequest,
        model_policy: GenerateOnceModelPolicy,
        reply: oneshot::Sender<Result<GenerateResponse, RuntimeError>>,
    },
    BackgroundTask {
        request: super::BackgroundTaskGenerateRequest,
        reply: oneshot::Sender<Result<GenerateResponse, RuntimeError>>,
    },
    TaskCompletionDelivery {
        request: TaskCompletionDeliveryRequest,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}
