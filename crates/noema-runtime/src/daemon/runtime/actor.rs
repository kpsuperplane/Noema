use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use noema_capabilities::{CapabilityBindingSourceHandle, CapabilityInvokerRegistration};
use noema_home::SystemErrorLogger;
#[cfg(test)]
use noema_providers::{ProviderHandle, ProviderRegistry, provider_account_instance_key};
use noema_providers::{
    ProviderRegistryHandle, ProviderRouteLease, ProviderRouteResolver, ProviderRouteResolverHandle,
    ProviderSelectionSnapshot, RegistryProviderRouteResolver, provider_selection_loader,
};
use noema_store::NoemaStore;
use tokio::sync::{mpsc, oneshot};

use super::RuntimeSpawnConfig;
use super::handle::{GenerateOnceModelPolicy, GenerateOnceRoute, RuntimeCommand};
use super::tasks::RuntimeTaskGroup;
use crate::daemon::protocol::RuntimeError;

type PendingTaskCompletion = BoxFuture<
    'static,
    (
        super::task_completion::GeneratedTaskCompletion,
        oneshot::Sender<Result<(), RuntimeError>>,
    ),
>;

pub(in crate::daemon) struct RuntimeActor {
    pub(in crate::daemon) primary_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) default_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) progress_audit_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) web_summary_provider: ProviderRouteResolverHandle,
    pub(in crate::daemon) provider_registry: ProviderRegistryHandle,
    pub(in crate::daemon) store: NoemaStore,
    pub(in crate::daemon) artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    pub(in crate::daemon) system_errors: SystemErrorLogger,
    pub(in crate::daemon) memory_operations: Option<noema_memory::MemoryOperationsHandle>,
    pub(in crate::daemon) web_backends: crate::WebBackendResolverHandle,
    pub(in crate::daemon) capability_bindings: CapabilityBindingSourceHandle,
    pub(in crate::daemon) capability_invokers: Arc<[CapabilityInvokerRegistration]>,
    pub(in crate::daemon) conversations: HashMap<String, ActiveConversation>,
    pub(super) tasks: RuntimeTaskGroup,
    pub(super) runtime_events: crate::daemon::RuntimeEventRegistry,
}

impl std::fmt::Debug for RuntimeActor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeActor")
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
            .field("web_backends", &"[CONFIGURED]")
            .field("capability_bindings", &"[CONFIGURED]")
            .field("capability_invokers", &self.capability_invokers.len())
            .field("conversation_count", &self.conversations.len())
            .finish_non_exhaustive()
    }
}

impl RuntimeActor {
    #[cfg(test)]
    pub(in crate::daemon) async fn new(
        default_provider_kind: String,
        providers: HashMap<String, ProviderHandle>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
    ) -> Result<Self, RuntimeError> {
        let artifact_operations =
            crate::test_support::artifact_operations(&store).map_err(RuntimeError::Protocol)?;
        let routing = test_provider_routing(&store, &default_provider_kind, providers).await?;
        let (capability_bindings, capability_invokers) =
            crate::contract_test_support::empty_capability_handles();
        let web_backends = crate::test_support::web_backends_for_store(&store);
        Self::from_spawn_config(RuntimeSpawnConfig {
            primary_provider: routing.primary,
            default_provider: routing.default,
            progress_audit_provider: routing.progress_audit,
            web_summary_provider: routing.web_summary,
            provider_registry: routing.registry,
            store,
            artifact_operations,
            system_errors,
            memory_operations: None,
            runtime_events: crate::daemon::RuntimeEventRegistry::default(),
            web_backends,
            capability_bindings,
            capability_invokers,
        })
        .await
    }

    pub(in crate::daemon) async fn from_spawn_config(
        config: RuntimeSpawnConfig,
    ) -> Result<Self, RuntimeError> {
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
            web_backends: config.web_backends,
            capability_bindings: config.capability_bindings,
            capability_invokers: config.capability_invokers,
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            runtime_events: config.runtime_events,
        })
    }

    pub(in crate::daemon) async fn resolve_static_provider_route(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, RuntimeError> {
        let loader = provider_selection_loader(move || {
            let selection = selection.clone();
            Box::pin(async move { Ok(selection) })
        });
        RegistryProviderRouteResolver::new(loader, Arc::clone(&self.provider_registry))
            .resolve_route()
            .await
            .map_err(RuntimeError::from)
    }

    pub(in crate::daemon) async fn resolve_primary_provider(
        &self,
    ) -> Result<ProviderRouteLease, RuntimeError> {
        self.primary_provider
            .resolve_route()
            .await
            .map_err(RuntimeError::from)
    }

    pub(in crate::daemon) async fn resolve_memory_provider(
        &self,
    ) -> Result<ProviderRouteLease, RuntimeError> {
        RegistryProviderRouteResolver::new(
            noema_memory::memory_provider_selection_loader(Arc::new(self.store.clone())),
            Arc::clone(&self.provider_registry),
        )
        .resolve_route()
        .await
        .map_err(RuntimeError::from)
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
            web_backends: self.web_backends.clone(),
            capability_bindings: self.capability_bindings.clone(),
            capability_invokers: self.capability_invokers.clone(),
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            runtime_events: self.runtime_events.clone(),
        }
    }

    pub(super) async fn run(mut self, mut receiver: mpsc::Receiver<RuntimeCommand>) {
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
                RuntimeCommand::StartConversation { cwd, reply } => {
                    let _ = reply.send(self.start_conversation(cwd).await);
                }
                RuntimeCommand::StartPrimaryConversation { cwd, reply } => {
                    let _ = reply.send(self.start_primary_conversation(cwd).await);
                }
                RuntimeCommand::Turn {
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
                RuntimeCommand::SelectMultipleChoice {
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
                RuntimeCommand::GenerateOnce {
                    route,
                    mut request,
                    model_policy,
                    reply,
                } => {
                    let provider = match route {
                        GenerateOnceRoute::Default => self
                            .default_provider
                            .resolve_route()
                            .await
                            .map_err(RuntimeError::from),
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
                                let _ = reply.send(Err(RuntimeError::Provider(
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
                            .map_err(RuntimeError::Provider);
                        let _ = reply.send(result);
                    });
                }
                RuntimeCommand::BackgroundTask { request, reply } => {
                    let actor = self.clone_for_background();
                    self.tasks.spawn(async move {
                        let result = actor.generate_background_task(request).await;
                        let _ = reply.send(result);
                    });
                }
                RuntimeCommand::TaskCompletionDelivery { request, reply } => {
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
                RuntimeCommand::Shutdown { reply } => {
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
) -> Result<TestProviderRouting, RuntimeError> {
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
            .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
        registry
            .register(key, provider)
            .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
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
            .map_err(|error| RuntimeError::Protocol(error.to_string()))?,
    );
    let ready_selection = registry
        .prove_ready_selection(configured_default.clone())
        .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
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

fn runtime_stopped() -> RuntimeError {
    RuntimeError::Protocol("daemon runtime stopped".to_string())
}

#[derive(Debug, Clone)]
pub(in crate::daemon) struct ActiveConversation {
    pub(in crate::daemon) cwd: Option<String>,
    pub(in crate::daemon) next_turn_index: u64,
}
