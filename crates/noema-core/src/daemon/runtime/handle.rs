use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use crate::{
    FoundationLocalProvider, FoundationLocalProviderConfig, NoemaStore, OpenAiProvider,
    ProviderConfig, ProviderKind, SystemErrorLogger,
    config::DEFAULT_FOUNDATION_LOCAL_PROFILE,
    provider::adapters::codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    provider::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
        ProviderContextMetadata, ProviderError, ProviderToolCapabilities,
    },
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::actor::CodexRuntimeActor;
use crate::daemon::protocol::{DaemonError, StartedConversation, TurnStreamEvent};

pub(crate) type RuntimeProviderMap = HashMap<String, Arc<dyn RuntimeModelProvider>>;

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
    ) -> Result<(String, RuntimeProviderMap), DaemonError> {
        let (default_provider_kind, default_provider) =
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
        Ok((default_provider_kind, providers))
    }

    pub(crate) async fn spawn_with_provider_map_and_memory(
        default_provider_kind: String,
        providers: RuntimeProviderMap,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
        memory_connection: Option<crate::MnemosyneConnection>,
    ) -> Result<Self, DaemonError> {
        Self::spawn_with_provider_map_inner(
            default_provider_kind,
            providers,
            store,
            system_errors,
            memory_connection,
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
        )
        .await
    }

    async fn spawn_with_provider_map_inner(
        default_provider_kind: String,
        providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
        store: NoemaStore,
        system_errors: SystemErrorLogger,
        memory_connection: Option<crate::MnemosyneConnection>,
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
        )
        .await?;
        let cancellation = Arc::new(RuntimeCancellation(actor.tasks.cancellation_token()));
        tokio::spawn(actor.run(receiver));
        Ok(Self {
            sender,
            cancellation,
            default_provider_kind,
            tool_classification_model,
        })
    }

    pub(crate) fn tool_classification_model(&self) -> Option<&str> {
        self.tool_classification_model.as_deref()
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

fn provider_from_config(
    provider_config: ProviderConfig,
    system_errors: SystemErrorLogger,
) -> Result<(String, Arc<dyn RuntimeModelProvider>), DaemonError> {
    match provider_config {
        ProviderConfig::Codex(codex_config) => Ok((
            ProviderKind::Codex.as_str().to_string(),
            Arc::new(CodexResponsesProvider::new(codex_provider_config(
                codex_config,
                system_errors,
            )?)?),
        )),
        ProviderConfig::OpenAi(mut openai_config) => {
            openai_config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::OpenAi.as_str().to_string(),
                Arc::new(OpenAiProvider::new(openai_config)?),
            ))
        }
        ProviderConfig::FoundationLocal(config) => Ok((
            ProviderKind::FoundationLocal.as_str().to_string(),
            Arc::new(FoundationLocalProvider::new(foundation_local_config(
                config,
                system_errors,
            ))?),
        )),
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
    let paths = crate::NoemaPaths::from_process_env()?;
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
