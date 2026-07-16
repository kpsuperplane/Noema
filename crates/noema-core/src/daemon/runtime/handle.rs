use std::{collections::HashMap, future::Future, path::PathBuf, pin::Pin, sync::Arc};

use crate::{
    FoundationLocalProvider, FoundationLocalProviderConfig, LocalModelsProvider,
    LocalModelsProviderConfig, NoemaStore, OpenAiProvider, ProviderConfig, ProviderKind,
    config::DEFAULT_FOUNDATION_LOCAL_PROFILE,
    provider::adapters::codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    provider::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
        ProviderContextMetadata, ProviderError, ProviderResponseContinuation,
        ProviderToolCapabilities,
    },
};
use noema_home::{NoemaPaths, SystemErrorLogger};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::{TaskCompletionDeliveryRequest, actor::CodexRuntimeActor};
use crate::daemon::protocol::{DaemonError, StartedConversation, TurnStreamEvent};

pub(crate) type RuntimeProviderMap = HashMap<String, Arc<dyn RuntimeModelProvider>>;
type ConfiguredRuntimeProvider = (
    String,
    Arc<dyn RuntimeModelProvider>,
    Option<crate::LlamaServerSupervisor>,
);
type ConfiguredRuntimeProviderMap = (
    String,
    RuntimeProviderMap,
    Option<crate::LlamaServerSupervisor>,
);

/// Model provider interface used by the daemon runtime and auxiliary tools.
pub trait RuntimeModelProvider: std::fmt::Debug + Send + Sync {
    /// Return the provider's default model for tool-classification style tasks.
    fn default_tool_classification_model(&self) -> Option<String> {
        None
    }

    /// Return context metadata for an optional model override.
    fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata::default()
    }

    /// Return the provider's response-continuation strategy.
    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::default()
    }

    /// Return provider tool capabilities for an optional model override.
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities::default()
    }

    /// Count provider tokens for optional instructions and input.
    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        model: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
        let _ = (instructions, input, model);
        Box::pin(async { Ok(None) })
    }

    /// Generate a response while optionally emitting stream events.
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>;
}

impl<T> RuntimeModelProvider for T
where
    T: ModelProvider + std::fmt::Debug + Send + Sync,
{
    fn default_tool_classification_model(&self) -> Option<String> {
        ModelProvider::default_tool_classification_model(self)
    }

    fn context_metadata(&self, model: Option<&str>) -> ProviderContextMetadata {
        ModelProvider::context_metadata(self, model)
    }

    fn response_continuation(&self, model: Option<&str>) -> ProviderResponseContinuation {
        ModelProvider::response_continuation(self, model)
    }

    fn tool_capabilities(&self, model: Option<&str>) -> ProviderToolCapabilities {
        ModelProvider::tool_capabilities(self, model)
    }

    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        model: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u32>, ProviderError>> + Send + 'a>> {
        Box::pin(async move { ModelProvider::count_tokens(self, instructions, input, model).await })
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move { ModelProvider::generate_streaming(self, request, on_event).await })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
    cancellation: Arc<RuntimeCancellation>,
    default_provider_kind: String,
    tool_classification_model: Option<String>,
    local_models_runtime: Arc<tokio::sync::RwLock<Option<crate::LlamaServerSupervisor>>>,
    local_models_runtime_root: Arc<tokio::sync::RwLock<Option<PathBuf>>>,
    memory_model_route:
        Arc<tokio::sync::RwLock<Option<crate::memory_model_proxy::MemoryModelRoute>>>,
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
    ) -> Result<ConfiguredRuntimeProviderMap, DaemonError> {
        let (default_provider_kind, default_provider, local_models_runtime) =
            provider_from_config(provider_config, system_errors.clone())?;
        let mut providers = HashMap::new();
        providers.insert(default_provider_kind.clone(), default_provider);
        if !providers.contains_key("codex") {
            providers.insert(
                "codex".to_string(),
                Arc::new(CodexResponsesProvider::new(default_codex_provider_config(
                    system_errors.clone(),
                )?)?),
            );
        }
        if !providers.contains_key("foundation_local") {
            providers.insert(
                "foundation_local".to_string(),
                Arc::new(FoundationLocalProvider::new(
                    default_foundation_local_config(system_errors),
                )?),
            );
        }
        Ok((default_provider_kind, providers, local_models_runtime))
    }

    pub(crate) async fn spawn_with_provider_map_and_memory(
        default_provider_kind: String,
        providers: RuntimeProviderMap,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
        memory_connection: Option<crate::MnemosyneConnection>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        Self::spawn_with_provider_map_inner(
            default_provider_kind,
            providers,
            store,
            system_errors,
            memory_connection,
            task_subscriptions,
        )
        .await
    }

    pub(crate) fn provider_kind(&self) -> &str {
        &self.default_provider_kind
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider(
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        Self::spawn_with_provider_kind(provider, store, "codex").await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_and_memory(
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
        memory_connection: Option<crate::MnemosyneConnection>,
    ) -> Result<Self, DaemonError> {
        let provider_kind = "codex".to_string();
        let system_errors = store.system_error_logger();
        Self::spawn_with_provider_map_inner(
            provider_kind.clone(),
            HashMap::from([(provider_kind, provider)]),
            store,
            system_errors,
            memory_connection,
            crate::graphql::ConversationSubscriptionRegistry::default(),
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_kind(
        provider: Arc<dyn RuntimeModelProvider>,
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
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
        search_provider: crate::search::types::SearchRuntimeProvider,
    ) -> Result<Self, DaemonError> {
        let provider_kind = "codex".to_string();
        let system_errors = store.system_error_logger();
        let providers = HashMap::from([(provider_kind.clone(), provider)]);
        let Some(default_provider) = providers.get(&provider_kind) else {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        };
        let tool_classification_model = default_provider.default_tool_classification_model();
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
            default_provider_kind: provider_kind,
            tool_classification_model,
            local_models_runtime: Arc::new(tokio::sync::RwLock::new(None)),
            local_models_runtime_root: Arc::new(tokio::sync::RwLock::new(None)),
            memory_model_route: Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_and_search_fetch_providers(
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
        search_provider: crate::search::types::SearchRuntimeProvider,
        web_fetch_provider: crate::web_fetch::types::WebFetchRuntimeProvider,
    ) -> Result<Self, DaemonError> {
        let provider_kind = "codex".to_string();
        let system_errors = store.system_error_logger();
        let providers = HashMap::from([(provider_kind.clone(), provider)]);
        let Some(default_provider) = providers.get(&provider_kind) else {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        };
        let tool_classification_model = default_provider.default_tool_classification_model();
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
            default_provider_kind: provider_kind,
            tool_classification_model,
            local_models_runtime: Arc::new(tokio::sync::RwLock::new(None)),
            local_models_runtime_root: Arc::new(tokio::sync::RwLock::new(None)),
            memory_model_route: Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    #[cfg(test)]
    pub(crate) async fn spawn_with_provider_map<I>(
        default_provider_kind: impl Into<String>,
        providers: I,
        store: NoemaStore,
    ) -> Result<Self, DaemonError>
    where
        I: IntoIterator<Item = (String, Arc<dyn RuntimeModelProvider>)>,
    {
        let system_errors = store.system_error_logger();
        Self::spawn_with_provider_map_inner(
            default_provider_kind.into(),
            providers.into_iter().collect(),
            store,
            system_errors,
            None,
            crate::graphql::ConversationSubscriptionRegistry::default(),
        )
        .await
    }

    async fn spawn_with_provider_map_inner(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
        memory_connection: Option<crate::MnemosyneConnection>,
        task_subscriptions: crate::graphql::ConversationSubscriptionRegistry,
    ) -> Result<Self, DaemonError> {
        let Some(default_provider) = providers.get(&default_provider_kind) else {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: default_provider_kind,
                message: "default provider is not available in this daemon".to_string(),
            }));
        };
        let tool_classification_model = default_provider.default_tool_classification_model();
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new_with_memory(
            default_provider_kind.clone(),
            providers,
            store,
            system_errors,
            memory_connection,
            task_subscriptions,
        )
        .await?;
        let cancellation = Arc::new(RuntimeCancellation(actor.tasks.cancellation_token()));
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
            default_provider_kind,
            tool_classification_model,
            local_models_runtime: Arc::new(tokio::sync::RwLock::new(None)),
            local_models_runtime_root: Arc::new(tokio::sync::RwLock::new(None)),
            memory_model_route: Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    pub(crate) fn tool_classification_model(&self) -> Option<&str> {
        self.tool_classification_model.as_deref()
    }

    /// Set the packaged llama.cpp resource root used for dynamic registrations.
    pub(crate) async fn set_local_models_runtime_root(&self, runtime_root: Option<PathBuf>) {
        *self.local_models_runtime_root.write().await = runtime_root;
    }

    pub(crate) async fn attach_memory_model_route(
        &self,
        route: Option<crate::memory_model_proxy::MemoryModelRoute>,
    ) {
        *self.memory_model_route.write().await = route;
    }

    /// Attach the supervisor owned by an already-registered local provider.
    pub(crate) async fn attach_local_models_runtime(&self, runtime: crate::LlamaServerSupervisor) {
        let previous = self.local_models_runtime.write().await.replace(runtime);
        if let Some(previous) = previous {
            previous.shutdown().await;
        }
    }

    /// Register or replace the active installed local model without restarting Noema.
    pub(crate) async fn register_installed_local_model(
        &self,
        installation: &crate::LocalModelInstallationRecord,
        paths: &NoemaPaths,
    ) -> Result<(), DaemonError> {
        if installation.status != crate::LocalModelInstallationStatus::Installed {
            return Err(DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: ProviderKind::LocalModels.as_str().to_string(),
                message: format!("local model `{}` is not installed", installation.model_id),
            }));
        }
        let sha256 = installation.sha256.as_deref().ok_or_else(|| {
            DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: ProviderKind::LocalModels.as_str().to_string(),
                message: format!(
                    "installed local model `{}` has no verified digest",
                    installation.model_id
                ),
            })
        })?;
        let model_path = paths.local_model_blob_path(sha256)?;
        let runtime_root = self.local_models_runtime_root.read().await.clone();
        let provider = LocalModelsProvider::new(LocalModelsProviderConfig {
            default_model: installation.model_id.clone(),
            model_path: Some(model_path),
            preferred_backend: Some(installation.backend),
            runtime_root,
            context_window_tokens: crate::config::DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: crate::config::DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: crate::config::DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
            system_errors: Some(SystemErrorLogger::from_paths(paths)),
        })?;
        self.register_local_models_provider(provider).await
    }

    async fn register_local_models_provider(
        &self,
        provider: LocalModelsProvider,
    ) -> Result<(), DaemonError> {
        let runtime = provider.runtime().clone();
        let model_profile = ModelProvider::default_tool_classification_model(&provider)
            .ok_or_else(|| {
                DaemonError::Protocol(
                    "local models provider has no default model profile".to_string(),
                )
            })?;
        let provider: Arc<dyn RuntimeModelProvider> = Arc::new(provider);
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::RegisterProvider {
                provider_kind: ProviderKind::LocalModels.as_str().to_string(),
                provider: provider.clone(),
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;

        self.attach_local_models_runtime(runtime).await;
        if let Some(route) = self.memory_model_route.read().await.clone() {
            route.update(provider, model_profile, None).await;
        }
        Ok(())
    }

    /// Return the current local inference process status, when a model is registered.
    pub(crate) async fn local_model_runtime_status(
        &self,
    ) -> Option<crate::LocalModelRuntimeStatus> {
        self.local_models_runtime
            .read()
            .await
            .as_ref()
            .map(crate::LlamaServerSupervisor::status)
    }

    /// Subscribe to local inference process transitions, when a model is registered.
    pub(crate) async fn subscribe_local_model_runtime_status(
        &self,
    ) -> Option<tokio::sync::watch::Receiver<crate::LocalModelRuntimeStatus>> {
        self.local_models_runtime
            .read()
            .await
            .as_ref()
            .map(crate::LlamaServerSupervisor::subscribe_status)
    }

    /// Retry the registered local inference runtime from its first backend candidate.
    pub(crate) async fn retry_local_model_runtime(
        &self,
    ) -> Result<crate::LocalModelRuntimeStatus, DaemonError> {
        let runtime = self
            .local_models_runtime
            .read()
            .await
            .clone()
            .ok_or_else(|| {
                DaemonError::Provider(ProviderError::ProviderUnavailable {
                    provider: ProviderKind::LocalModels.as_str().to_string(),
                    message: "no installed local model is registered".to_string(),
                })
            })?;
        runtime.retry().await.map_err(|error| {
            DaemonError::Provider(ProviderError::ProviderUnavailable {
                provider: ProviderKind::LocalModels.as_str().to_string(),
                message: error.to_string(),
            })
        })?;
        Ok(runtime.status())
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

    pub(crate) async fn generate_once(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        self.generate_once_with_provider_kind(None, request).await
    }

    pub(crate) async fn generate_once_with_provider_kind(
        &self,
        provider_kind: Option<String>,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::GenerateOnce {
                provider_kind,
                request,
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
        if let Some(runtime) = self.local_models_runtime.write().await.take() {
            runtime.shutdown().await;
        }
        let (reply, reply_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(CodexRuntimeCommand::Shutdown { reply })
            .await;
        let _ = reply_rx.await;
    }
}

fn provider_from_config(
    provider_config: ProviderConfig,
    system_errors: SystemErrorLogger,
) -> Result<ConfiguredRuntimeProvider, DaemonError> {
    match provider_config {
        ProviderConfig::Codex(codex_config) => Ok((
            ProviderKind::Codex.as_str().to_string(),
            Arc::new(CodexResponsesProvider::new(codex_provider_config(
                codex_config,
                system_errors,
            )?)?),
            None,
        )),
        ProviderConfig::OpenAi(mut openai_config) => {
            openai_config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::OpenAi.as_str().to_string(),
                Arc::new(OpenAiProvider::new(openai_config)?),
                None,
            ))
        }
        ProviderConfig::FoundationLocal(config) => Ok((
            ProviderKind::FoundationLocal.as_str().to_string(),
            Arc::new(FoundationLocalProvider::new(foundation_local_config(
                config,
                system_errors,
            ))?),
            None,
        )),
        ProviderConfig::LocalModels(config) => {
            let provider = LocalModelsProvider::new(local_models_config(config, system_errors))?;
            let runtime = provider.runtime().clone();
            Ok((
                ProviderKind::LocalModels.as_str().to_string(),
                Arc::new(provider),
                Some(runtime),
            ))
        }
    }
}

fn default_codex_provider_config(
    system_errors: SystemErrorLogger,
) -> Result<CodexProviderConfig, DaemonError> {
    codex_provider_config(CodexProviderConfig::default(), system_errors)
}

fn codex_provider_config(
    mut codex_config: CodexProviderConfig,
    system_errors: SystemErrorLogger,
) -> Result<CodexProviderConfig, DaemonError> {
    let paths = NoemaPaths::from_process_env()?;
    let account_home = paths.provider_account_home("codex", "default");
    crate::provider::auth::ensure_provider_account_home(&account_home)?;
    apply_provider_account_home(&mut codex_config, &account_home);
    codex_config.system_errors = Some(system_errors);
    Ok(codex_config)
}

fn default_foundation_local_config(
    system_errors: SystemErrorLogger,
) -> FoundationLocalProviderConfig {
    FoundationLocalProviderConfig {
        default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
        bridge_path: None,
        system_errors: Some(system_errors),
    }
}

fn foundation_local_config(
    mut config: FoundationLocalProviderConfig,
    system_errors: SystemErrorLogger,
) -> FoundationLocalProviderConfig {
    config.system_errors = Some(system_errors);
    config
}

fn local_models_config(
    mut config: LocalModelsProviderConfig,
    system_errors: SystemErrorLogger,
) -> LocalModelsProviderConfig {
    config.system_errors = Some(system_errors);
    config
}

fn apply_provider_account_home(
    config: &mut crate::CodexProviderConfig,
    account_home: &std::path::Path,
) {
    config.account_home = Some(account_home.to_path_buf());
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
        provider_kind: Option<String>,
        request: GenerateRequest,
        reply: oneshot::Sender<Result<GenerateResponse, DaemonError>>,
    },
    BackgroundTask {
        request: super::BackgroundTaskGenerateRequest,
        reply: oneshot::Sender<Result<GenerateResponse, DaemonError>>,
    },
    RegisterProvider {
        provider_kind: String,
        provider: Arc<dyn RuntimeModelProvider>,
        reply: oneshot::Sender<()>,
    },
    TaskCompletionDelivery {
        request: TaskCompletionDeliveryRequest,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_config_for_provider_account_uses_account_home() {
        let account_home = std::path::PathBuf::from("/noema/providers/codex/default");
        let mut config = CodexProviderConfig::default();

        apply_provider_account_home(&mut config, &account_home);

        assert_eq!(config.account_home.as_deref(), Some(account_home.as_path()));
    }
}
