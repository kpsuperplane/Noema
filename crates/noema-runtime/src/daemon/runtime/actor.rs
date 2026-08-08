use std::collections::HashMap;
use std::sync::{Arc, RwLock, atomic::AtomicBool};

use noema_capabilities::{CapabilityBindingSourceHandle, CapabilityInvokerRegistration};
use noema_conversations::{ConversationItemKind, ConversationItemRecord};
use noema_home::{SystemErrorEvent, SystemErrorLogger};
#[cfg(test)]
use noema_providers::{ProviderHandle, ProviderRegistry, provider_account_instance_key};
use noema_providers::{
    ProviderRegistryHandle, ProviderRouteLease, ProviderSelectionSnapshot,
    RegistryProviderRouteResolver, provider_selection_loader,
};
use noema_store::NoemaStore;
use tokio::sync::mpsc;

use super::RuntimeSpawnConfig;
use super::capability_auth_arguments::CapabilityAuthArgumentStore;
use super::handle::{GenerateOnceModelPolicy, GenerateOnceRoute, RuntimeCommand};
use super::tasks::RuntimeTaskGroup;
use crate::daemon::{
    ConversationRuntimeEvent, TurnStreamEvent, TurnTranscriptItem, protocol::RuntimeError,
};

pub(in crate::daemon) struct RuntimeActor {
    pub(super) capability_auth_arguments: CapabilityAuthArgumentStore,
    pub(super) browser_snapshot_contexts:
        Arc<std::sync::Mutex<HashMap<String, BrowserSnapshotContext>>>,
    pub(in crate::daemon) primary_provider: RegistryProviderRouteResolver,
    pub(in crate::daemon) default_provider: RegistryProviderRouteResolver,
    pub(in crate::daemon) progress_audit_provider: RegistryProviderRouteResolver,
    pub(in crate::daemon) action_reviewer_provider: RegistryProviderRouteResolver,
    pub(in crate::daemon) web_summary_provider: RegistryProviderRouteResolver,
    pub(in crate::daemon) provider_registry: ProviderRegistryHandle,
    pub(in crate::daemon) store: NoemaStore,
    pub(in crate::daemon) artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    pub(in crate::daemon) system_errors: SystemErrorLogger,
    pub(in crate::daemon) native_memory: Option<noema_memory::NativeMemory>,
    pub(in crate::daemon) native_memory_update_active: Arc<AtomicBool>,
    pub(in crate::daemon) native_memory_update_error: Arc<RwLock<Option<String>>>,
    pub(in crate::daemon) web_backends: crate::WebBackendResolverHandle,
    pub(in crate::daemon) capability_bindings: CapabilityBindingSourceHandle,
    pub(in crate::daemon) capability_invokers: Arc<[CapabilityInvokerRegistration]>,
    pub(in crate::daemon) conversations: HashMap<String, ActiveConversation>,
    pub(super) tasks: RuntimeTaskGroup,
    pub(super) runtime_events: crate::daemon::RuntimeEventRegistry,
}

#[derive(Clone)]
pub(super) struct BrowserSnapshotContext {
    pub(super) url: String,
    pub(super) title: String,
    pub(super) revision: u64,
    pub(super) elements: HashMap<String, noema_capabilities::web::browse::BrowseInteractiveElement>,
}

impl std::fmt::Debug for RuntimeActor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeActor")
            .field("primary_provider", &"[CONFIGURED]")
            .field("default_provider", &"[CONFIGURED]")
            .field("progress_audit_provider", &"[CONFIGURED]")
            .field("action_reviewer_provider", &"[CONFIGURED]")
            .field("web_summary_provider", &"[CONFIGURED]")
            .field("provider_registry", &self.provider_registry)
            .field("store", &self.store)
            .field("artifact_operations", &"[CONFIGURED]")
            .field("system_errors", &self.system_errors)
            .field("web_backends", &"[CONFIGURED]")
            .field("capability_bindings", &"[CONFIGURED]")
            .field("capability_invokers", &self.capability_invokers.len())
            .field("conversation_count", &self.conversations.len())
            .finish_non_exhaustive()
    }
}

impl RuntimeActor {
    pub(super) fn native_memory_context(&self) -> Option<String> {
        let root = self.native_memory.as_ref()?.read_root().ok()?;
        let mut rendered = root.body;
        if !root.children.is_empty() {
            rendered.push_str("\n\nDirect child pages:\n");
            for child in root.children {
                rendered.push_str(&format!(
                    "- {} ({}, {})\n",
                    child.title, child.path, child.id
                ));
            }
        }
        Some(rendered)
    }

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
        Ok(Self::from_spawn_config(RuntimeSpawnConfig {
            noema_paths: crate::test_support::test_paths(),
            primary_provider: routing.primary,
            default_provider: routing.default,
            progress_audit_provider: routing.progress_audit,
            action_reviewer_provider: routing.action_reviewer,
            web_summary_provider: routing.web_summary,
            provider_registry: routing.registry,
            store,
            artifact_operations,
            system_errors,
            native_memory: None,
            runtime_events: crate::daemon::RuntimeEventRegistry::default(),
            web_backends,
            capability_bindings,
            capability_invokers,
        }))
    }

    pub(in crate::daemon) fn from_spawn_config(config: RuntimeSpawnConfig) -> Self {
        Self {
            capability_auth_arguments: CapabilityAuthArgumentStore::new(config.noema_paths),
            browser_snapshot_contexts: Arc::new(std::sync::Mutex::new(HashMap::new())),
            primary_provider: config.primary_provider,
            default_provider: config.default_provider,
            progress_audit_provider: config.progress_audit_provider,
            action_reviewer_provider: config.action_reviewer_provider,
            web_summary_provider: config.web_summary_provider,
            provider_registry: config.provider_registry,
            store: config.store,
            artifact_operations: config.artifact_operations,
            system_errors: config.system_errors,
            native_memory: config.native_memory,
            native_memory_update_active: Arc::new(AtomicBool::new(false)),
            native_memory_update_error: Arc::new(RwLock::new(None)),
            web_backends: config.web_backends,
            capability_bindings: config.capability_bindings,
            capability_invokers: config.capability_invokers,
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            runtime_events: config.runtime_events,
        }
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
            self.store.auxiliary_provider_selection_loader(
                noema_store::AuxiliaryModelTask::MemoryConsolidation,
            ),
            Arc::clone(&self.provider_registry),
        )
        .resolve_route()
        .await
        .map_err(RuntimeError::from)
    }

    pub(super) fn clone_for_background(&self) -> Self {
        Self {
            capability_auth_arguments: self.capability_auth_arguments.clone(),
            browser_snapshot_contexts: Arc::clone(&self.browser_snapshot_contexts),
            primary_provider: self.primary_provider.clone(),
            default_provider: self.default_provider.clone(),
            progress_audit_provider: self.progress_audit_provider.clone(),
            action_reviewer_provider: self.action_reviewer_provider.clone(),
            web_summary_provider: self.web_summary_provider.clone(),
            provider_registry: Arc::clone(&self.provider_registry),
            store: self.store.clone(),
            artifact_operations: self.artifact_operations.clone(),
            system_errors: self.system_errors.clone(),
            native_memory: self.native_memory.clone(),
            native_memory_update_active: Arc::clone(&self.native_memory_update_active),
            native_memory_update_error: Arc::clone(&self.native_memory_update_error),
            web_backends: self.web_backends.clone(),
            capability_bindings: self.capability_bindings.clone(),
            capability_invokers: self.capability_invokers.clone(),
            conversations: HashMap::new(),
            tasks: RuntimeTaskGroup::default(),
            runtime_events: self.runtime_events.clone(),
        }
    }

    pub(super) async fn run(mut self, mut receiver: mpsc::Receiver<RuntimeCommand>) {
        if let Err(error) = self.recover_governed_action_origins().await {
            self.system_errors.try_append(
                SystemErrorEvent::new(
                    crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT,
                    "failed to recover interrupted reviewed action",
                )
                .with_error_chain([error.to_string()]),
            );
        }
        if let Err(error) = self.recover_capability_authentication_origins().await {
            let message = error.to_string();
            self.system_errors.try_append(
                SystemErrorEvent::new(
                    crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT,
                    "failed to recover interrupted MCP authentication",
                )
                .with_error_chain([message]),
            );
        }
        if let Err(error) = self.recover_conversation_interactions().await {
            self.system_errors.try_append(
                SystemErrorEvent::new(
                    crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT,
                    "failed to recover answered conversation interaction",
                )
                .with_error_chain([error.to_string()]),
            );
        }
        let mut shutdown_reply = None;
        loop {
            let command = receiver.recv().await;
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
                    client_time_zone,
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
                        result = self.turn(conversation_id.clone(), input, item_tx, client_message_id, client_time_zone) => result,
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
                RuntimeCommand::SubmitA2UIAction {
                    conversation_id,
                    interaction_id,
                    expected_revision,
                    surface_id,
                    source_component_id,
                    action_name,
                    context,
                    data_model,
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
                        result = self.submit_a2ui_action(
                            conversation_id.clone(),
                            interaction_id,
                            expected_revision,
                            surface_id,
                            source_component_id,
                            action_name,
                            context,
                            data_model,
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
                RuntimeCommand::DeliverWorkNotification {
                    notification,
                    conversation_id,
                    work_event,
                    reply,
                } => {
                    self.runtime_events.publish_work(work_event);
                    let result = match self
                        .narrate_work_notification(&notification, &conversation_id)
                        .await
                    {
                        Ok(()) => {
                            let completion = noema_store::CompleteWorkNotification {
                                notification_id: notification.notification_id,
                                lease_token: notification.lease_token,
                                conversation_id,
                            };
                            match self.store.complete_work_notification(completion).await {
                                Ok(Some(item)) => {
                                    notification_conversation_event(item).map(|event| {
                                        self.runtime_events.publish_conversation(event);
                                    })
                                }
                                Ok(None) => Ok(()),
                                Err(error) => Err(RuntimeError::from(error)),
                            }
                        }
                        Err(error) => Err(error),
                    };
                    let _ = reply.send(result);
                }
                RuntimeCommand::NarrateCapabilitySetupCompletion { completion, reply } => {
                    let result = self.narrate_capability_setup_completion(completion).await;
                    if let Err(error) = &result {
                        self.system_errors.try_append(
                            SystemErrorEvent::new(
                                crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT,
                                "failed to narrate completed capability setup",
                            )
                            .with_error_chain([error.to_string()]),
                        );
                    }
                    let _ = reply.send(result);
                }
                RuntimeCommand::ResolveGovernedAction {
                    action_id,
                    revision,
                    human_id,
                    decision,
                    reply,
                } => {
                    let mut actor = self.clone_for_background();
                    self.tasks.spawn(async move {
                        let result = actor
                            .resolve_governed_action(&action_id, revision, &human_id, decision)
                            .await;
                        let _ = reply.send(result);
                    });
                }
                RuntimeCommand::BrowserActionSessionAvailability {
                    actions,
                    human_id,
                    reply,
                } => {
                    let actor = self.clone_for_background();
                    self.tasks.spawn(async move {
                        let result = actor
                            .browser_action_session_availability(actions, &human_id)
                            .await;
                        let _ = reply.send(result);
                    });
                }
                RuntimeCommand::ResumeMcpAuthenticationAttempt { attempt_id, reply } => {
                    let mut actor = self.clone_for_background();
                    self.tasks.spawn(async move {
                        let result = actor.resume_mcp_authentication_attempt(&attempt_id).await;
                        let _ = reply.send(result);
                    });
                }
                RuntimeCommand::PublishCapabilityAuthenticationOrigins { reply } => {
                    let mut actor = self.clone_for_background();
                    self.tasks.spawn(async move {
                        let result = actor.publish_capability_authentication_origins().await;
                        let _ = reply.send(result);
                    });
                }
                RuntimeCommand::SkipMcpAuthenticationRequest {
                    request_id,
                    revision,
                    human_id,
                    reply,
                } => {
                    let mut actor = self.clone_for_background();
                    self.tasks.spawn(async move {
                        let result = actor
                            .skip_mcp_authentication_request(&request_id, revision, &human_id)
                            .await;
                        let _ = reply.send(result);
                    });
                }
                RuntimeCommand::UpdateNativeMemory {
                    conversation_id,
                    reply,
                } => {
                    let result = match self
                        .store
                        .primary_conversation_for_human("human:local")
                        .await
                    {
                        Ok(Some(primary)) if primary.conversation_id == conversation_id => {
                            Ok(self.schedule_background_native_memory_update(conversation_id))
                        }
                        Ok(_) => Err(RuntimeError::Protocol(
                            "native memory updates are limited to the primary conversation"
                                .to_string(),
                        )),
                        Err(error) => Err(error.into()),
                    };
                    let _ = reply.send(result);
                }
                RuntimeCommand::NativeMemoryStatus { reply } => {
                    let _ = reply.send(Ok(self
                        .native_memory_update_active
                        .load(std::sync::atomic::Ordering::Acquire)));
                }
                RuntimeCommand::NativeMemoryError { reply } => {
                    let error = self
                        .native_memory_update_error
                        .read()
                        .ok()
                        .and_then(|value| value.clone());
                    let _ = reply.send(Ok(error));
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

fn notification_conversation_event(
    item: ConversationItemRecord,
) -> Result<ConversationRuntimeEvent, RuntimeError> {
    if item.kind != ConversationItemKind::TaskReference {
        return Err(RuntimeError::Protocol(format!(
            "notification item {} is not a task reference",
            item.item_id
        )));
    }
    let task_id = notification_field(&item, "task_id")?;
    Ok(ConversationRuntimeEvent::Turn {
        client_message_id: None,
        event: Box::new(TurnStreamEvent::ConversationItem {
            conversation_id: item.conversation_id,
            item_id: item.item_id,
            cursor: Some(item.cursor),
            turn_id: item.turn_id,
            metadata: item.metadata,
            item: Box::new(TurnTranscriptItem::TaskReference { task_id }),
        }),
    })
}

fn notification_field(
    item: &ConversationItemRecord,
    field: &'static str,
) -> Result<String, RuntimeError> {
    item.payload_json
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            RuntimeError::Protocol(format!("notification item {} has no {field}", item.item_id))
        })
}

#[cfg(test)]
pub(super) struct TestProviderRouting {
    pub(super) primary: RegistryProviderRouteResolver,
    pub(super) default: RegistryProviderRouteResolver,
    pub(super) progress_audit: RegistryProviderRouteResolver,
    pub(super) action_reviewer: RegistryProviderRouteResolver,
    pub(super) web_summary: RegistryProviderRouteResolver,
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

    let bind = |loader| RegistryProviderRouteResolver::new(loader, Arc::clone(&registry));
    Ok(TestProviderRouting {
        primary: bind(store.agent_provider_selection_loader("agent:primary")),
        default: bind(store.default_provider_selection_loader()),
        progress_audit: bind(store.auxiliary_provider_selection_loader(
            noema_store::AuxiliaryModelTask::ToolProgressAudit,
        )),
        action_reviewer: bind(
            store.auxiliary_provider_selection_loader(
                noema_store::AuxiliaryModelTask::ActionReviewer,
            ),
        ),
        web_summary: bind(store.auxiliary_provider_selection_loader(
            noema_store::AuxiliaryModelTask::WebFetchSummarizer,
        )),
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
