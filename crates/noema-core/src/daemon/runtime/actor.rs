use std::collections::HashMap;
use std::sync::Arc;

use crate::NoemaStore;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use noema_capabilities::{CapabilityBindingSourceHandle, CapabilityInvokerRegistration};
use noema_home::SystemErrorLogger;
use noema_providers::{
    ProviderAccountOperationsHandle, ProviderCredentialAccessHandle, ProviderRouteLease,
    ProviderSelectionSnapshot,
};
#[cfg(test)]
use noema_providers::{ProviderAccountService, ProviderHandle};
use tokio::sync::{mpsc, oneshot};

use super::CodexRuntimeSpawnConfig;
use super::handle::{CodexRuntimeCommand, GenerateOnceModelPolicy};
use super::provider_routes::LegacyProviderRoutes;
use super::tasks::RuntimeTaskGroup;
use crate::daemon::protocol::DaemonError;

type PendingTaskCompletion = BoxFuture<
    'static,
    (
        super::task_completion::GeneratedTaskCompletion,
        oneshot::Sender<Result<(), DaemonError>>,
    ),
>;

#[derive(Clone)]
pub(crate) struct ProviderAccountRuntimeAccess {
    operations: ProviderAccountOperationsHandle,
    credentials: ProviderCredentialAccessHandle,
}

impl ProviderAccountRuntimeAccess {
    pub(crate) fn new(
        operations: ProviderAccountOperationsHandle,
        credentials: ProviderCredentialAccessHandle,
    ) -> Self {
        Self {
            operations,
            credentials,
        }
    }

    pub(in crate::daemon) fn operations(&self) -> &ProviderAccountOperationsHandle {
        &self.operations
    }

    pub(in crate::daemon) fn credentials(&self) -> &ProviderCredentialAccessHandle {
        &self.credentials
    }
}

impl std::fmt::Debug for ProviderAccountRuntimeAccess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderAccountRuntimeAccess")
            .field("operations", &"[CONFIGURED]")
            .field("credentials", &"[REDACTED]")
            .finish()
    }
}

pub(in crate::daemon) struct CodexRuntimeActor {
    pub(in crate::daemon) default_provider_kind: String,
    pub(in crate::daemon) provider_routes: LegacyProviderRoutes,
    pub(in crate::daemon) store: NoemaStore,
    pub(in crate::daemon) artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    pub(in crate::daemon) system_errors: SystemErrorLogger,
    pub(in crate::daemon) memory_operations: Option<noema_memory::MemoryOperationsHandle>,
    pub(in crate::daemon) search_provider: crate::search::types::SearchRuntimeProvider,
    pub(in crate::daemon) web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider,
    pub(in crate::daemon) provider_accounts: ProviderAccountRuntimeAccess,
    pub(in crate::daemon) capability_bindings: CapabilityBindingSourceHandle,
    pub(in crate::daemon) capability_invokers: Arc<[CapabilityInvokerRegistration]>,
    pub(in crate::daemon) conversations: HashMap<String, ActiveConversation>,
    pub(super) tasks: RuntimeTaskGroup,
    pub(super) task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
}

impl std::fmt::Debug for CodexRuntimeActor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexRuntimeActor")
            .field("default_provider_kind", &self.default_provider_kind)
            .field("provider_routes", &self.provider_routes)
            .field("store", &self.store)
            .field("artifact_operations", &"[CONFIGURED]")
            .field("system_errors", &self.system_errors)
            .field(
                "memory_operations_configured",
                &self.memory_operations.is_some(),
            )
            .field("search_provider", &self.search_provider)
            .field("web_fetch_provider", &self.web_fetch_provider)
            .field("provider_accounts", &self.provider_accounts)
            .field("capability_bindings", &"[CONFIGURED]")
            .field("capability_invokers", &self.capability_invokers.len())
            .field("conversation_count", &self.conversations.len())
            .finish_non_exhaustive()
    }
}

impl CodexRuntimeActor {
    #[cfg(test)]
    pub(in crate::daemon) async fn new(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
    ) -> Result<Self, DaemonError> {
        let artifact_operations =
            crate::test_support::artifact_operations(&store).map_err(DaemonError::Protocol)?;
        Self::new_with_memory(
            default_provider_kind,
            providers,
            store,
            artifact_operations,
            system_errors,
            None,
            crate::graphql::ConversationSubscriptionRegistry::default(),
        )
        .await
    }

    #[cfg(test)]
    pub(in crate::daemon) async fn new_with_memory(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        memory_operations: Option<noema_memory::MemoryOperationsHandle>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        let provider_routes = LegacyProviderRoutes::new(providers)?;
        let provider_accounts = test_provider_account_access(&store)?;
        let (capability_bindings, capability_invokers) = test_capability_handles();
        Self::from_spawn_config(CodexRuntimeSpawnConfig {
            default_provider_kind,
            provider_routes,
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

    pub(in crate::daemon) async fn from_spawn_config(
        config: CodexRuntimeSpawnConfig,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            default_provider_kind: config.default_provider_kind,
            provider_routes: config.provider_routes,
            store: config.store,
            artifact_operations: config.artifact_operations,
            system_errors: config.system_errors,
            memory_operations: config.memory_operations,
            search_provider: noema_providers::default_web_search_backend(),
            web_fetch_provider: noema_providers::default_web_fetch_backend(),
            provider_accounts: config.provider_accounts,
            capability_bindings: config.capability_bindings,
            capability_invokers: config.capability_invokers,
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            task_subscriptions: config.task_subscriptions,
        })
    }

    #[cfg(test)]
    pub(in crate::daemon) async fn new_with_search_provider(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
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
        providers: HashMap<String, ProviderHandle>,
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

    pub(in crate::daemon) async fn resolve_provider_route(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, DaemonError> {
        self.provider_routes
            .read()
            .await
            .resolve_snapshot(selection)
            .map_err(DaemonError::from)
    }

    pub(in crate::daemon) async fn provider_for_kind(
        &self,
        provider_kind: &str,
    ) -> Result<ProviderRouteLease, DaemonError> {
        self.resolve_provider_route(ProviderSelectionSnapshot::provider_default(
            provider_kind,
            format!("provider_account:{provider_kind}:default"),
            None,
            Some("legacy_runtime_route".to_string()),
        ))
        .await
    }

    pub(in crate::daemon) async fn default_provider(
        &self,
    ) -> Result<ProviderRouteLease, DaemonError> {
        self.provider_for_kind(&self.default_provider_kind).await
    }

    pub(super) fn clone_for_background(&self) -> Self {
        Self {
            default_provider_kind: self.default_provider_kind.clone(),
            provider_routes: self.provider_routes.clone(),
            store: self.store.clone(),
            artifact_operations: self.artifact_operations.clone(),
            system_errors: self.system_errors.clone(),
            memory_operations: self.memory_operations.clone(),
            search_provider: self.search_provider.clone(),
            web_fetch_provider: self.web_fetch_provider.clone(),
            provider_accounts: self.provider_accounts.clone(),
            capability_bindings: self.capability_bindings.clone(),
            capability_invokers: self.capability_invokers.clone(),
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            task_subscriptions: self.task_subscriptions.clone(),
        }
    }

    #[allow(dead_code)]
    pub(in crate::daemon) async fn resolved_web_search_provider(
        &self,
    ) -> Result<super::web_tools::ResolvedWebProvider, noema_providers::ProviderPersistenceError>
    {
        super::web_tools::resolve_web_search_provider(&self.store).await
    }

    #[allow(dead_code)]
    pub(in crate::daemon) async fn resolved_web_fetch_provider(
        &self,
    ) -> Result<super::web_tools::ResolvedWebProvider, noema_providers::ProviderPersistenceError>
    {
        super::web_tools::resolve_web_fetch_provider(&self.store).await
    }

    pub(super) async fn run(mut self, mut receiver: mpsc::Receiver<CodexRuntimeCommand>) {
        let mut shutdown_reply = None;
        let mut task_completions: FuturesUnordered<PendingTaskCompletion> = FuturesUnordered::new();
        loop {
            let command = if task_completions.is_empty() {
                receiver.recv().await
            } else {
                tokio::select! {
                    command = receiver.recv() => command,
                    Some((completion, reply)) = task_completions.next() => {
                        let result = self.finish_task_completion_delivery(completion).await;
                        let _ = reply.send(result);
                        continue;
                    }
                }
            };
            let Some(command) = command else {
                break;
            };
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
                    selection,
                    mut request,
                    model_policy,
                    reply,
                } => {
                    let provider = match self.resolve_provider_route(selection.clone()).await {
                        Ok(provider) => provider,
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            continue;
                        }
                    };
                    match model_policy {
                        GenerateOnceModelPolicy::Selection => {
                            request.model = selection.model_profile;
                        }
                        GenerateOnceModelPolicy::ProviderToolClassification => {
                            let Some(model) =
                                provider.operations().default_tool_classification_model()
                            else {
                                let _ = reply.send(Err(DaemonError::Provider(
                                    noema_providers::ProviderError::ProviderUnavailable {
                                        provider: selection.provider_kind,
                                        message: "provider has no tool-classification model"
                                            .to_string(),
                                    },
                                )));
                                continue;
                            };
                            request.model = Some(model);
                        }
                    }
                    request.options.reasoning_effort = selection.reasoning_effort;
                    self.tasks.spawn(async move {
                        let mut ignore_event = |_| {};
                        let result = provider
                            .operations()
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
                CodexRuntimeCommand::TaskCompletionDelivery { request, reply } => {
                    match self.start_task_completion_delivery(request).await {
                        Ok(super::task_completion::TaskCompletionDeliveryStart::Delivered) => {
                            let _ = reply.send(Ok(()));
                        }
                        Ok(super::task_completion::TaskCompletionDeliveryStart::Generate(
                            generation,
                        )) => {
                            task_completions
                                .push(async move { (generation.generate().await, reply) }.boxed());
                        }
                        Err(error) => {
                            let _ = reply.send(Err(error));
                        }
                    }
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

#[cfg(test)]
pub(super) fn test_provider_account_access(
    store: &NoemaStore,
) -> Result<ProviderAccountRuntimeAccess, DaemonError> {
    let paths = crate::test_support::test_paths();
    let service = ProviderAccountService::new(
        paths,
        Arc::new(store.clone()),
        Arc::new(store.clone()),
        crate::test_support::system_error_logger(),
    )
    .map_err(DaemonError::from)?;
    Ok(ProviderAccountRuntimeAccess::new(
        service.operations(),
        service.credentials(),
    ))
}

#[cfg(test)]
#[derive(Debug)]
struct EmptyCapabilityBindingSource;

#[cfg(test)]
impl noema_capabilities::CapabilityBindingSource for EmptyCapabilityBindingSource {
    fn catalog(
        &self,
    ) -> noema_capabilities::CapabilityFuture<
        '_,
        Result<
            noema_capabilities::CapabilityCatalogResult,
            noema_capabilities::CapabilityBindingSourceError,
        >,
    > {
        Box::pin(async { Ok(noema_capabilities::CapabilityCatalogResult::default()) })
    }
}

#[cfg(test)]
#[derive(Debug)]
struct EmptyCapabilityInvoker;

#[cfg(test)]
impl noema_capabilities::CapabilityInvoker for EmptyCapabilityInvoker {
    fn invoke(
        &self,
        _invocation: noema_capabilities::CapabilityInvocation,
    ) -> noema_capabilities::CapabilityFuture<
        '_,
        Result<noema_capabilities::CapabilityOutput, noema_capabilities::CapabilityError>,
    > {
        Box::pin(async { Err(noema_capabilities::CapabilityError::UnknownOperation) })
    }
}

#[cfg(test)]
pub(super) fn test_capability_handles() -> (
    CapabilityBindingSourceHandle,
    Arc<[CapabilityInvokerRegistration]>,
) {
    (
        Arc::new(EmptyCapabilityBindingSource),
        Arc::from([CapabilityInvokerRegistration::new(
            noema_capabilities::InvokerKey::new("test-external"),
            Arc::new(EmptyCapabilityInvoker),
        )]),
    )
}

fn runtime_stopped() -> DaemonError {
    DaemonError::Protocol("daemon runtime stopped".to_string())
}

#[derive(Debug, Clone)]
pub(in crate::daemon) struct ActiveConversation {
    pub(in crate::daemon) provider_selection: noema_providers::ProviderSelectionSnapshot,
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) next_turn_index: u64,
}

impl ActiveConversation {
    pub(in crate::daemon) fn provider_kind(&self) -> &str {
        &self.provider_selection.provider_kind
    }

    pub(in crate::daemon) fn model(&self) -> Option<&str> {
        self.provider_selection.model_profile.as_deref()
    }

    pub(in crate::daemon) fn reasoning_effort(&self) -> Option<noema_providers::ReasoningEffort> {
        self.provider_selection.reasoning_effort
    }
}
