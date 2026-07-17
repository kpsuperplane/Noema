use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::LocalModelsProvider;
#[cfg(test)]
use crate::NoemaStore;
use noema_home::SystemErrorLogger;
#[cfg(test)]
use noema_providers::ProviderError;
use noema_providers::{
    CodexProviderConfig, DEFAULT_FOUNDATION_LOCAL_PROFILE, FoundationLocalProviderConfig,
    GenerateRequest, GenerateResponse, LocalModelsProviderConfig, ProviderConfig,
    ProviderCredentialAccessHandle, ProviderHandle, ProviderKind, ProviderSelectionSnapshot,
    erase_model_provider, hosted_provider_from_config,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::{
    CodexRuntimeSpawnConfig, TaskCompletionDeliveryRequest, actor::CodexRuntimeActor,
    provider_routes::LegacyProviderRoutes,
};
use crate::daemon::protocol::{DaemonError, StartedConversation, TurnStreamEvent};

mod local_models;

pub(crate) type RuntimeProviderMap = HashMap<String, ProviderHandle>;
type ConfiguredRuntimeProvider = (String, ProviderHandle, Option<crate::LlamaServerSupervisor>);
type ConfiguredRuntimeProviderMap = (
    String,
    RuntimeProviderMap,
    Option<crate::LlamaServerSupervisor>,
);

#[derive(Debug, Clone)]
pub(crate) struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
    cancellation: Arc<RuntimeCancellation>,
    default_provider_kind: String,
    provider_routes: LegacyProviderRoutes,
    local_models_runtime: Arc<tokio::sync::RwLock<Option<crate::LlamaServerSupervisor>>>,
    local_models_runtime_root: Arc<tokio::sync::RwLock<Option<PathBuf>>>,
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
        let (default_provider_kind, default_provider, local_models_runtime) = provider_from_config(
            provider_config,
            system_errors.clone(),
            provider_credentials.clone(),
        )?;
        let mut providers = HashMap::new();
        providers.insert(default_provider_kind.clone(), default_provider);
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
        Ok((default_provider_kind, providers, local_models_runtime))
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_routes_and_memory(
        default_provider_kind: String,
        provider_routes: LegacyProviderRoutes,
        store: NoemaStore,
        artifact_operations: noema_artifacts::ArtifactOperationsHandle,
        system_errors: SystemErrorLogger,
        memory_connection: Option<crate::MnemosyneConnection>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        let provider_accounts = super::actor::test_provider_account_access(&store)?;
        let (capability_bindings, capability_invokers) = super::actor::test_capability_handles();
        Self::spawn(CodexRuntimeSpawnConfig {
            default_provider_kind,
            provider_routes,
            store,
            artifact_operations,
            system_errors,
            memory_connection,
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
        let default_provider_kind = actor.default_provider_kind.clone();
        let provider_routes = actor.provider_routes.clone();
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
            default_provider_kind,
            provider_routes,
            local_models_runtime: Arc::new(tokio::sync::RwLock::new(None)),
            local_models_runtime_root: Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    pub(crate) fn provider_kind(&self) -> &str {
        &self.default_provider_kind
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
        memory_connection: Option<crate::MnemosyneConnection>,
    ) -> Result<Self, DaemonError> {
        let provider_kind = "codex".to_string();
        let system_errors = store.system_error_logger();
        let artifact_operations =
            crate::test_support::artifact_operations(&store).map_err(DaemonError::Protocol)?;
        Self::spawn_with_provider_map_inner(
            provider_kind.clone(),
            HashMap::from([(provider_kind, provider)]),
            store,
            artifact_operations,
            system_errors,
            memory_connection,
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
        let system_errors = store.system_error_logger();
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
        let provider_routes = actor.provider_routes.clone();
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
            default_provider_kind: provider_kind,
            provider_routes,
            local_models_runtime: Arc::new(tokio::sync::RwLock::new(None)),
            local_models_runtime_root: Arc::new(tokio::sync::RwLock::new(None)),
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
        let system_errors = store.system_error_logger();
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
        let provider_routes = actor.provider_routes.clone();
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
            default_provider_kind: provider_kind,
            provider_routes,
            local_models_runtime: Arc::new(tokio::sync::RwLock::new(None)),
            local_models_runtime_root: Arc::new(tokio::sync::RwLock::new(None)),
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
        let system_errors = store.system_error_logger();
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
        memory_connection: Option<crate::MnemosyneConnection>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        if !providers.contains_key(&default_provider_kind) {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: default_provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        }
        let provider_routes = LegacyProviderRoutes::new(providers)?;
        let provider_accounts = super::actor::test_provider_account_access(&store)?;
        let (capability_bindings, capability_invokers) = super::actor::test_capability_handles();
        Self::spawn(CodexRuntimeSpawnConfig {
            default_provider_kind,
            provider_routes,
            store,
            artifact_operations,
            system_errors,
            memory_connection,
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
        let selection = provider_selection_for_generate_request(
            &self.default_provider_kind,
            &request,
            "runtime_generate_once",
        );
        self.generate_once_with_provider_selection(selection, request)
            .await
    }

    pub(crate) async fn generate_once_with_provider_selection(
        &self,
        selection: ProviderSelectionSnapshot,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        self.send_generate_once(selection, request, GenerateOnceModelPolicy::Selection)
            .await
    }

    pub(crate) async fn generate_once_with_tool_classification_model(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        let selection = ProviderSelectionSnapshot::provider_default(
            self.default_provider_kind.clone(),
            format!("provider_account:{}:default", self.default_provider_kind),
            request.options.reasoning_effort,
            Some("tool_classification_model".to_string()),
        );
        self.send_generate_once(
            selection,
            request,
            GenerateOnceModelPolicy::ProviderToolClassification,
        )
        .await
    }

    async fn send_generate_once(
        &self,
        selection: ProviderSelectionSnapshot,
        request: GenerateRequest,
        model_policy: GenerateOnceModelPolicy,
    ) -> Result<GenerateResponse, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::GenerateOnce {
                selection,
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
        if let Some(runtime) = self.local_models_runtime.write().await.take() {
            runtime.shutdown().await;
        }
    }
}

fn provider_from_config(
    provider_config: ProviderConfig,
    system_errors: SystemErrorLogger,
    provider_credentials: ProviderCredentialAccessHandle,
) -> Result<ConfiguredRuntimeProvider, DaemonError> {
    match provider_config {
        ProviderConfig::LocalModels(config) => {
            let provider = LocalModelsProvider::new(local_models_config(config, system_errors))?;
            let runtime = provider.runtime().clone();
            Ok((
                ProviderKind::LocalModels.as_str().to_string(),
                erase_model_provider(provider),
                Some(runtime),
            ))
        }
        hosted => {
            let (provider_kind, provider) =
                hosted_provider_from_config(hosted, provider_credentials, system_errors)?;
            Ok((provider_kind, provider, None))
        }
    }
}

fn default_foundation_local_config() -> FoundationLocalProviderConfig {
    FoundationLocalProviderConfig {
        default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
        bridge_path: None,
        system_errors: None,
    }
}

fn local_models_config(
    mut config: LocalModelsProviderConfig,
    system_errors: SystemErrorLogger,
) -> LocalModelsProviderConfig {
    config.system_errors = Some(system_errors);
    config
}

#[cfg(test)]
fn provider_selection_for_generate_request(
    provider_kind: &str,
    request: &GenerateRequest,
    selection_source: &str,
) -> ProviderSelectionSnapshot {
    let provider_account_id = format!("provider_account:{provider_kind}:default");
    match request.model.clone() {
        Some(model_profile) => ProviderSelectionSnapshot::explicit(
            provider_kind,
            provider_account_id,
            model_profile,
            request.options.reasoning_effort,
            Some(selection_source.to_string()),
        ),
        None => ProviderSelectionSnapshot::provider_default(
            provider_kind,
            provider_account_id,
            request.options.reasoning_effort,
            Some(selection_source.to_string()),
        ),
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum GenerateOnceModelPolicy {
    Selection,
    ProviderToolClassification,
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
        selection: ProviderSelectionSnapshot,
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
