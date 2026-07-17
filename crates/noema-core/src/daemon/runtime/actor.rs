use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use noema_capabilities::{CapabilityBindingSourceHandle, CapabilityInvokerRegistration};
use noema_home::SystemErrorLogger;
use noema_providers::{
    ProviderAccountOperationsHandle, ProviderCredentialAccessHandle, ProviderRegistryHandle,
    ProviderRouteLease, ProviderRouteResolver, ProviderRouteResolverHandle,
    ProviderSelectionSnapshot, RegistryProviderRouteResolver, provider_selection_loader,
};
#[cfg(test)]
use noema_providers::{
    ProviderAccountService, ProviderHandle, ProviderRegistry, provider_account_instance_key,
};
use noema_store::NoemaStore;
use tokio::sync::{mpsc, oneshot};

use super::CodexRuntimeSpawnConfig;
use super::handle::{CodexRuntimeCommand, GenerateOnceModelPolicy, GenerateOnceRoute};
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
    pub(in crate::daemon) primary_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) default_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) progress_audit_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) web_summary_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) provider_registry: ProviderRegistryHandle,
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
            .field("primary_provider", &"[CONFIGURED]")
            .field("default_provider", &"[CONFIGURED]")
            .field("progress_audit_provider", &"[CONFIGURED]")
            .field("web_summary_provider", &"[CONFIGURED]")
            .field("provider_registry", &self.provider_registry)
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
        let routing = test_provider_routing(&store, &default_provider_kind, providers).await?;
        let provider_accounts = test_provider_account_access(&store)?;
        let (capability_bindings, capability_invokers) = test_capability_handles();
        Self::from_spawn_config(CodexRuntimeSpawnConfig {
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

    pub(in crate::daemon) async fn from_spawn_config(
        config: CodexRuntimeSpawnConfig,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            primary_provider: config.primary_provider,
            default_provider: config.default_provider,
            progress_audit_provider: config.progress_audit_provider,
            web_summary_provider: config.web_summary_provider,
            provider_registry: config.provider_registry,
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

    pub(in crate::daemon) async fn resolve_static_provider_route(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, DaemonError> {
        let loader = provider_selection_loader(move || {
            let selection = selection.clone();
            Box::pin(async move { Ok(selection) })
        });
        RegistryProviderRouteResolver::new(loader, Arc::clone(&self.provider_registry))
            .resolve_route()
            .await
            .map_err(DaemonError::from)
    }

    pub(in crate::daemon) async fn resolve_primary_provider(
        &self,
    ) -> Result<ProviderRouteLease, DaemonError> {
        self.primary_provider
            .resolve_route()
            .await
            .map_err(DaemonError::from)
    }

    pub(in crate::daemon) async fn resolve_default_provider(
        &self,
    ) -> Result<ProviderRouteLease, DaemonError> {
        self.default_provider
            .resolve_route()
            .await
            .map_err(DaemonError::from)
    }

    pub(in crate::daemon) async fn resolve_memory_provider(
        &self,
    ) -> Result<ProviderRouteLease, DaemonError> {
        RegistryProviderRouteResolver::new(
            noema_memory::memory_provider_selection_loader(Arc::new(self.store.clone())),
            Arc::clone(&self.provider_registry),
        )
        .resolve_route()
        .await
        .map_err(DaemonError::from)
    }

    pub(super) fn clone_for_background(&self) -> Self {
        Self {
            primary_provider: Arc::clone(&self.primary_provider),
            default_provider: Arc::clone(&self.default_provider),
            progress_audit_provider: Arc::clone(&self.progress_audit_provider),
            web_summary_provider: Arc::clone(&self.web_summary_provider),
            provider_registry: Arc::clone(&self.provider_registry),
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
                    route,
                    mut request,
                    model_policy,
                    reply,
                } => {
                    let provider = match route {
                        GenerateOnceRoute::Default => self.resolve_default_provider().await,
                        GenerateOnceRoute::Memory => self.resolve_memory_provider().await,
                    };
                    let provider = match provider {
                        Ok(provider) => provider,
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            continue;
                        }
                    };
                    let selection = provider.selection();
                    match model_policy {
                        GenerateOnceModelPolicy::Selection => {
                            request.model = selection.model_profile.clone();
                            request.options.reasoning_effort = selection.reasoning_effort;
                        }
                        GenerateOnceModelPolicy::ProviderToolClassification => {
                            let Some(model) =
                                provider.operations().default_tool_classification_model()
                            else {
                                let _ = reply.send(Err(DaemonError::Provider(
                                    noema_providers::ProviderError::ProviderUnavailable {
                                        provider: selection.provider_kind.clone(),
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
pub(super) struct TestProviderRouting {
    pub(super) primary: ProviderRouteResolverHandle,
    pub(super) default: ProviderRouteResolverHandle,
    pub(super) progress_audit: ProviderRouteResolverHandle,
    pub(super) web_summary: ProviderRouteResolverHandle,
    pub(super) registry: ProviderRegistryHandle,
}

#[cfg(test)]
pub(super) async fn test_provider_routing(
    store: &NoemaStore,
    default_provider_kind: &str,
    providers: HashMap<String, ProviderHandle>,
) -> Result<TestProviderRouting, DaemonError> {
    let default_account = match default_provider_kind {
        "foundation_local" => {
            store
                .ensure_default_foundation_local_provider_account()
                .await?
        }
        _ => store.ensure_default_provider_account().await?,
    };
    store
        .update_provider_account_status(
            &default_account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await?;
    store.ensure_default_actors().await?;

    let default_kind = if default_provider_kind == "foundation_local" {
        "foundation_local"
    } else {
        "codex"
    };
    let default_model = if default_kind == "foundation_local" {
        noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE
    } else {
        "gpt-5.6-luna"
    };
    let registry = Arc::new(ProviderRegistry::new());
    for (provider_kind, provider) in providers {
        let account_id = format!("provider_account:{provider_kind}:default");
        let key = provider_account_instance_key(&account_id)
            .map_err(|error| DaemonError::Protocol(error.to_string()))?;
        registry
            .register(key, provider)
            .map_err(|error| DaemonError::Protocol(error.to_string()))?;
    }
    let mut configured_default = ProviderSelectionSnapshot::explicit(
        default_kind,
        &default_account.provider_account_id,
        default_model,
        None,
        Some("test_runtime_default".to_string()),
    );
    configured_default.provider_instance_key = Some(
        provider_account_instance_key(&default_account.provider_account_id)
            .map_err(|error| DaemonError::Protocol(error.to_string()))?,
    );
    let ready_selection = registry
        .prove_ready_selection(configured_default.clone())
        .map_err(|error| DaemonError::Protocol(error.to_string()))?;
    store
        .initialize_missing_provider_selections(&configured_default, Some(&ready_selection))
        .await?;

    let bind = |loader| -> ProviderRouteResolverHandle {
        Arc::new(RegistryProviderRouteResolver::new(
            loader,
            Arc::clone(&registry),
        ))
    };
    Ok(TestProviderRouting {
        primary: bind(store.agent_provider_selection_loader("agent:primary")),
        default: bind(store.default_provider_selection_loader()),
        progress_audit: bind(
            store.auxiliary_provider_selection_loader(noema_store::TOOL_PROGRESS_AUDIT_TASK_ID),
        ),
        web_summary: bind(
            store.auxiliary_provider_selection_loader(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID),
        ),
        registry,
    })
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
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) next_turn_index: u64,
}
