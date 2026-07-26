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

use super::{RuntimeSpawnConfig, actor::RuntimeActor};
use crate::daemon::protocol::{RuntimeError, StartedConversation, TurnStreamEvent};

#[derive(Debug, Clone)]
/// Command handle for the governed foreground and background execution runtime.
pub struct RuntimeHandle {
    sender: mpsc::Sender<RuntimeCommand>,
    cancellation: Arc<RuntimeCancellation>,
}

#[derive(Debug)]
struct RuntimeCancellation(CancellationToken);

#[cfg(test)]
struct TestRuntimeServices {
    artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    system_errors: SystemErrorLogger,
    runtime_events: crate::daemon::RuntimeEventRegistry,
    web_backends: crate::WebBackendResolverHandle,
}

impl Drop for RuntimeCancellation {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl RuntimeHandle {
    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_map_and_events(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        runtime_events: crate::daemon::RuntimeEventRegistry,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_with_provider_map_inner(
            default_provider_kind,
            providers,
            store,
            TestRuntimeServices {
                artifact_operations,
                system_errors,
                runtime_events,
                web_backends: crate::test_support::web_backends(),
            },
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_registry(
        provider_registry: noema_providers::ProviderRegistryHandle,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        runtime_events: crate::daemon::RuntimeEventRegistry,
    ) -> Result<Self, RuntimeError> {
        use noema_providers::RegistryProviderRouteResolver;

        let bind = |loader| -> noema_providers::ProviderRouteResolverHandle {
            Arc::new(RegistryProviderRouteResolver::new(
                loader,
                Arc::clone(&provider_registry),
            ))
        };
        let (capability_bindings, capability_invokers) =
            crate::contract_test_support::empty_capability_handles();
        Self::spawn(RuntimeSpawnConfig {
            primary_provider: bind(store.agent_provider_selection_loader("agent:primary")),
            default_provider: bind(store.default_provider_selection_loader()),
            progress_audit_provider: bind(store.auxiliary_provider_selection_loader(
                noema_store::AuxiliaryModelTask::ToolProgressAudit,
            )),
            action_reviewer_provider: bind(store.auxiliary_provider_selection_loader(
                noema_store::AuxiliaryModelTask::ActionReviewer,
            )),
            web_summary_provider: bind(store.auxiliary_provider_selection_loader(
                noema_store::AuxiliaryModelTask::WebFetchSummarizer,
            )),
            provider_registry,
            store,
            artifact_operations,
            system_errors,
            native_memory: None,
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
        let actor = RuntimeActor::from_spawn_config(config);
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
            TestRuntimeServices {
                artifact_operations,
                system_errors,
                runtime_events: crate::daemon::RuntimeEventRegistry::default(),
                web_backends: crate::test_support::web_backends(),
            },
        )
        .await
    }

    #[cfg(test)]
    async fn spawn_with_provider_map_inner(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
        store: NoemaStore,
        services: TestRuntimeServices,
    ) -> Result<Self, RuntimeError> {
        if !providers.contains_key(&default_provider_kind) {
            return Err(RuntimeError::Provider(ProviderError::ProviderUnavailable {
                provider: default_provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let routing =
            super::actor::test_provider_routing(&store, &default_provider_kind, providers).await?;
        let (capability_bindings, capability_invokers) =
            crate::contract_test_support::empty_capability_handles();
        Self::spawn(RuntimeSpawnConfig {
            primary_provider: routing.primary,
            default_provider: routing.default,
            progress_audit_provider: routing.progress_audit,
            action_reviewer_provider: routing.action_reviewer,
            web_summary_provider: routing.web_summary,
            provider_registry: routing.registry,
            store,
            artifact_operations: services.artifact_operations,
            system_errors: services.system_errors,
            native_memory: None,
            runtime_events: services.runtime_events,
            web_backends: services.web_backends,
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
        self.request(|reply| RuntimeCommand::StartConversation { cwd, reply })
            .await
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
        self.request(|reply| RuntimeCommand::StartPrimaryConversation { cwd, reply })
            .await
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
        self.request(|reply| RuntimeCommand::Turn {
            conversation_id,
            input,
            item_tx,
            client_message_id,
            reply,
        })
        .await
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
        self.request(|reply| RuntimeCommand::SelectMultipleChoice {
            conversation_id,
            prompt_item_id,
            selected_option_ids,
            item_tx,
            client_message_id,
            reply,
        })
        .await
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

    /// Queue one native-memory update for the local primary conversation.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError`] if the runtime is stopped or rejects the
    /// requested conversation.
    pub async fn trigger_native_memory_update(
        &self,
        conversation_id: String,
    ) -> Result<bool, RuntimeError> {
        self.request(|reply| RuntimeCommand::UpdateNativeMemory {
            conversation_id,
            reply,
        })
        .await
    }

    /// Return whether a native memory update is currently active.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError`] if the runtime is stopped before answering.
    pub async fn native_memory_update_active(&self) -> Result<bool, RuntimeError> {
        self.request(|reply| RuntimeCommand::NativeMemoryStatus { reply })
            .await
    }

    /// Return the last native-memory update error, if the previous run failed.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError`] if the runtime is stopped before answering.
    pub async fn native_memory_update_error(&self) -> Result<Option<String>, RuntimeError> {
        self.request(|reply| RuntimeCommand::NativeMemoryError { reply })
            .await
    }

    async fn send_generate_once(
        &self,
        route: GenerateOnceRoute,
        request: GenerateRequest,
        model_policy: GenerateOnceModelPolicy,
    ) -> Result<GenerateResponse, RuntimeError> {
        self.request(|reply| RuntimeCommand::GenerateOnce {
            route,
            request,
            model_policy,
            reply,
        })
        .await
    }

    pub(crate) async fn generate_background_task(
        &self,
        request: super::BackgroundTaskGenerateRequest,
    ) -> Result<GenerateResponse, RuntimeError> {
        self.request(|reply| RuntimeCommand::BackgroundTask { request, reply })
            .await
    }

    pub(crate) async fn deliver_work_notification(
        &self,
        notification: noema_store::ClaimedWorkNotification,
        conversation_id: String,
        work_event: crate::daemon::WorkRuntimeEvent,
    ) -> Result<(), RuntimeError> {
        self.request(|reply| RuntimeCommand::DeliverWorkNotification {
            notification,
            conversation_id,
            work_event,
            reply,
        })
        .await
    }

    /// Resolve one immutable governed-action revision for the local human.
    ///
    /// # Errors
    ///
    /// Returns an error when the revision is stale, unauthorized, cannot be
    /// revalidated, or its exact saved payload cannot be executed.
    pub async fn resolve_governed_action(
        &self,
        action_id: String,
        revision: u64,
        human_id: String,
        decision: noema_store::GovernedActionDecision,
    ) -> Result<noema_store::GovernedActionRecord, RuntimeError> {
        self.request(|reply| RuntimeCommand::ResolveGovernedAction {
            action_id,
            revision,
            human_id,
            decision,
            reply,
        })
        .await
    }

    /// Resume every exact MCP call covered by one completed OAuth attempt.
    ///
    /// # Errors
    /// Returns [`RuntimeError`] when the runtime cannot resume an eligible request.
    pub async fn resume_mcp_authentication_attempt(
        &self,
        attempt_id: String,
    ) -> Result<(), RuntimeError> {
        self.request(|reply| RuntimeCommand::ResumeMcpAuthenticationAttempt { attempt_id, reply })
            .await
    }

    /// Skip one exact MCP call waiting for authentication.
    ///
    /// # Errors
    /// Returns [`RuntimeError`] when the request is stale or cannot be resumed safely.
    pub async fn skip_mcp_authentication_request(
        &self,
        request_id: String,
        revision: u64,
        human_id: String,
    ) -> Result<noema_store::McpAuthenticationRequestRecord, RuntimeError> {
        self.request(|reply| RuntimeCommand::SkipMcpAuthenticationRequest {
            request_id,
            revision,
            human_id,
            reply,
        })
        .await
    }

    async fn request<T>(
        &self,
        command: impl FnOnce(oneshot::Sender<Result<T, RuntimeError>>) -> RuntimeCommand,
    ) -> Result<T, RuntimeError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(command(reply))
            .await
            .map_err(|_| runtime_stopped())?;
        reply_rx.await.map_err(|_| runtime_stopped())?
    }

    /// Cancel active work and wait for the runtime actor to stop.
    pub async fn shutdown(&self) {
        self.cancellation.0.cancel();
        let (reply, reply_rx) = oneshot::channel();
        let _ = self.sender.send(RuntimeCommand::Shutdown { reply }).await;
        let _ = reply_rx.await;
    }
}

fn runtime_stopped() -> RuntimeError {
    RuntimeError::Protocol("daemon runtime stopped".to_string())
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
    DeliverWorkNotification {
        notification: noema_store::ClaimedWorkNotification,
        conversation_id: String,
        work_event: crate::daemon::WorkRuntimeEvent,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
    ResolveGovernedAction {
        action_id: String,
        revision: u64,
        human_id: String,
        decision: noema_store::GovernedActionDecision,
        reply: oneshot::Sender<Result<noema_store::GovernedActionRecord, RuntimeError>>,
    },
    ResumeMcpAuthenticationAttempt {
        attempt_id: String,
        reply: oneshot::Sender<Result<(), RuntimeError>>,
    },
    SkipMcpAuthenticationRequest {
        request_id: String,
        revision: u64,
        human_id: String,
        reply: oneshot::Sender<Result<noema_store::McpAuthenticationRequestRecord, RuntimeError>>,
    },
    UpdateNativeMemory {
        conversation_id: String,
        reply: oneshot::Sender<Result<bool, RuntimeError>>,
    },
    NativeMemoryStatus {
        reply: oneshot::Sender<Result<bool, RuntimeError>>,
    },
    NativeMemoryError {
        reply: oneshot::Sender<Result<Option<String>, RuntimeError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}
