use async_graphql::{Context, Object, Result, Schema, Subscription};
use futures_util::Stream;
#[cfg(test)]
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use super::{
    ConversationSubscriptionRegistry, GraphqlRuntimeState,
    agents::{
        self, GraphqlAgent, GraphqlAgentModelPreference, GraphqlSaveAgentModelPreferenceInput,
    },
    chat::{
        self, GraphqlConversationEvent, GraphqlConversationTranscriptPage,
        GraphqlConversationTranscriptPageInput, GraphqlPrimaryConversation,
        GraphqlSendConversationTurnInput, GraphqlSendMultipleChoiceSelectionInput,
        GraphqlTurnAccepted,
    },
    local_status::{self, GraphqlLocalStatus, GraphqlMemoryStorageStatus},
    mcp::{
        self, GraphqlAutofillToolCalibrationsResult, GraphqlContinueMcpServerSetupInput,
        GraphqlCreateMcpServerInput, GraphqlMcpApprovalRequest, GraphqlMcpOAuthSetupAttempt,
        GraphqlMcpServer, GraphqlMcpServerSetupResult, GraphqlMcpTool,
        GraphqlSaveToolCalibrationInput, GraphqlStartMcpServerOAuthSetupInput,
        GraphqlStartMcpServerReauthenticationOAuthSetupInput, GraphqlToolCalibration,
        GraphqlTrustedIdentitySelector,
    },
    memory::{
        self, GraphqlMemoryArticle, GraphqlMemoryGraph, GraphqlMemoryGraphInput,
        GraphqlMemoryServiceStatus, GraphqlMemorySettings, GraphqlSaveMemoryServiceSettingsInput,
    },
    onboarding::{
        self, GraphqlOnboardingStatus, GraphqlProviderAuthAttempt,
        GraphqlStartProviderAuthAttemptInput,
    },
    provider_accounts::{
        self, GraphqlCapabilityFeatures, GraphqlClearProviderSecretInput,
        GraphqlCreateProviderAccountInput, GraphqlDeleteProviderAccountInput,
        GraphqlProviderAccount, GraphqlProviderAccountCatalogEntry, GraphqlProviderCapability,
        GraphqlProviderSecretInput,
    },
    usage_settings::{self, GraphqlSaveToolProgressAuditPreferenceInput, GraphqlUsageSettings},
    web_fetch_settings::{
        self, GraphqlSaveWebFetchSummarizerPreferenceInput, GraphqlWebFetchSettings,
    },
    web_tool_settings::{
        self, GraphqlSaveWebToolProviderBindingInput, GraphqlWebToolBindingSettings,
        GraphqlWebToolSettings,
    },
};

const _: fn(GraphqlProviderCapability, GraphqlCapabilityFeatures) = |_, _| {};

/// Concrete GraphQL schema type used by the web server.
pub type GraphqlSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Shared state available to GraphQL resolvers.
#[derive(Clone)]
pub struct GraphqlState {
    runtime_state: GraphqlRuntimeState,
    #[cfg(test)]
    mcp_setup_outcomes: Option<Arc<Mutex<VecDeque<TestMcpSetupOutcome>>>>,
    #[cfg(test)]
    mcp_browser_oauth_supported: bool,
}

impl GraphqlState {
    /// Build test state with ready memory storage.
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests(),
            #[cfg(test)]
            mcp_setup_outcomes: None,
            #[cfg(test)]
            mcp_browser_oauth_supported: false,
        }
    }

    /// Build test state backed by a real embedded store.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: crate::NoemaStore) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store(store),
            mcp_setup_outcomes: None,
            mcp_browser_oauth_supported: false,
        }
    }

    /// Build test state backed by a real embedded store and path root.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store_and_paths(
        store: crate::NoemaStore,
        paths: crate::NoemaPaths,
    ) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store_and_paths(store, paths),
            mcp_setup_outcomes: None,
            mcp_browser_oauth_supported: false,
        }
    }

    /// Build test state backed by a store and runtime handle.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_store_and_runtime(
        store: crate::NoemaStore,
        runtime: crate::daemon::CodexRuntimeHandle,
    ) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store_and_runtime(store, runtime),
            mcp_setup_outcomes: None,
            mcp_browser_oauth_supported: false,
        }
    }

    /// Build test state with store, paths, and fake MCP setup outcomes.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_tests_with_store_paths_and_mcp_setup(
        store: crate::NoemaStore,
        paths: crate::NoemaPaths,
        outcomes: Vec<TestMcpSetupOutcome>,
    ) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store_and_paths(store, paths),
            mcp_setup_outcomes: Some(Arc::new(Mutex::new(VecDeque::from(outcomes)))),
            mcp_browser_oauth_supported: false,
        }
    }

    /// Override browser OAuth autodetection for MCP setup tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_mcp_browser_oauth_supported(mut self, supported: bool) -> Self {
        self.mcp_browser_oauth_supported = supported;
        self
    }

    /// Build state backed by the shared Noema runtime host.
    #[must_use]
    pub fn from_runtime_host(host: &crate::NoemaRuntimeHost) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::from_host(host),
            #[cfg(test)]
            mcp_setup_outcomes: None,
            #[cfg(test)]
            mcp_browser_oauth_supported: false,
        }
    }

    pub(crate) fn runtime(&self) -> Result<&crate::daemon::CodexRuntimeHandle> {
        self.runtime_state.runtime()
    }

    pub(crate) fn store(&self) -> Result<&crate::NoemaStore> {
        self.runtime_state.store()
    }

    pub(crate) fn optional_store(&self) -> Option<&crate::NoemaStore> {
        self.runtime_state.optional_store()
    }

    pub(crate) fn provider_auth(&self) -> Result<&crate::provider::auth::ProviderAuthManager> {
        self.runtime_state.provider_auth()
    }

    pub(crate) fn mcp_oauth(&self) -> Result<&crate::mcp::McpOAuthSetupManager> {
        self.runtime_state.mcp_oauth()
    }

    pub(crate) fn paths(&self) -> Result<&crate::NoemaPaths> {
        self.runtime_state.paths()
    }

    pub(crate) fn memory_connection(&self) -> Option<&crate::MnemosyneConnection> {
        self.runtime_state.memory_connection()
    }

    pub(crate) fn memory_startup_error(&self) -> Option<&str> {
        self.runtime_state.memory_startup_error()
    }

    pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
        self.runtime_state.subscriptions()
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.runtime_state.memory_storage()
    }

    pub(crate) fn mcp_setup_transport(
        &self,
        server: &crate::McpServerRecord,
        secrets_override: Option<&crate::mcp::secrets::McpSecretMaterial>,
    ) -> GraphqlMcpSetupTransport {
        #[cfg(test)]
        if let Some(outcomes) = &self.mcp_setup_outcomes {
            let outcome = outcomes
                .lock()
                .expect("MCP setup outcomes lock")
                .pop_front()
                .unwrap_or_else(|| TestMcpSetupOutcome::Ok(Vec::new()));
            return GraphqlMcpSetupTransport::Test(outcome);
        }

        let paths = match self.paths() {
            Ok(paths) => paths,
            Err(error) => {
                return GraphqlMcpSetupTransport::Unavailable(format!("{error:?}"));
            }
        };
        let secrets_path = paths.mcp_server_home(&server.mcp_server_id);
        let disk_secrets;
        let secrets = match secrets_override {
            Some(secrets) => secrets,
            None => {
                disk_secrets =
                    crate::mcp::secrets::read_mcp_secrets(&secrets_path).unwrap_or_default();
                &disk_secrets
            }
        };
        let system_errors = Some(crate::SystemErrorLogger::from_paths(paths));
        let diagnostic_server_id = Some(server.mcp_server_id.clone());
        match server.transport_kind {
            crate::McpTransportKind::Stdio => {
                match crate::mcp::StdioMcpTransport::from_server_config(server, secrets) {
                    Ok(transport) => GraphqlMcpSetupTransport::Stdio(Box::new(
                        transport
                            .with_diagnostics(system_errors.clone(), diagnostic_server_id.clone()),
                    )),
                    Err(message) => GraphqlMcpSetupTransport::Unavailable(message),
                }
            }
            crate::McpTransportKind::Sse => {
                match crate::mcp::SseMcpTransport::from_server_config(server, secrets) {
                    Ok(transport) => GraphqlMcpSetupTransport::Sse(Box::new(
                        transport
                            .with_diagnostics(system_errors.clone(), diagnostic_server_id.clone()),
                    )),
                    Err(message) => GraphqlMcpSetupTransport::Unavailable(message),
                }
            }
            crate::McpTransportKind::StreamableHttp => {
                match crate::mcp::StreamableHttpMcpTransport::from_server_config(server, secrets) {
                    Ok(transport) => GraphqlMcpSetupTransport::StreamableHttp(Box::new(
                        transport
                            .with_diagnostics(system_errors.clone(), diagnostic_server_id.clone()),
                    )),
                    Err(message) => GraphqlMcpSetupTransport::Unavailable(message),
                }
            }
        }
    }

    pub(crate) async fn mcp_browser_oauth_supported(
        &self,
        setup: &crate::mcp::setup::NewMcpServerSetup,
    ) -> bool {
        #[cfg(test)]
        {
            let _ = setup;
            self.mcp_browser_oauth_supported
        }

        #[cfg(not(test))]
        {
            if !matches!(
                setup.transport_kind,
                crate::McpTransportKind::Sse | crate::McpTransportKind::StreamableHttp
            ) || setup.secrets.has_secret_material()
            {
                return false;
            }
            let Some(url) = setup
                .safe_config
                .get("url")
                .and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            crate::mcp::oauth::oauth_authorization_supported(url).await
        }
    }
}

pub(crate) enum GraphqlMcpSetupTransport {
    Unavailable(String),
    Sse(Box<crate::mcp::SseMcpTransport>),
    StreamableHttp(Box<crate::mcp::StreamableHttpMcpTransport>),
    Stdio(Box<crate::mcp::StdioMcpTransport>),
    #[cfg(test)]
    Test(TestMcpSetupOutcome),
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) enum TestMcpSetupOutcome {
    Ok(Vec<crate::mcp::DiscoveredMcpTool>),
    AuthRequired(String),
}

impl crate::mcp::McpTransport for GraphqlMcpSetupTransport {
    async fn initialize(&mut self) -> std::result::Result<(), crate::mcp::McpClientError> {
        match self {
            Self::Unavailable(message) => {
                Err(crate::mcp::McpClientError::Transport(message.clone()))
            }
            Self::Sse(transport) => transport.initialize().await,
            Self::StreamableHttp(transport) => transport.initialize().await,
            Self::Stdio(transport) => transport.initialize().await,
            #[cfg(test)]
            Self::Test(TestMcpSetupOutcome::Ok(_)) => Ok(()),
            #[cfg(test)]
            Self::Test(TestMcpSetupOutcome::AuthRequired(message)) => {
                Err(crate::mcp::McpClientError::AuthRequired(message.clone()))
            }
        }
    }

    async fn list_tools(
        &mut self,
    ) -> std::result::Result<Vec<crate::mcp::DiscoveredMcpTool>, crate::mcp::McpClientError> {
        match self {
            Self::Unavailable(message) => {
                Err(crate::mcp::McpClientError::Transport(message.clone()))
            }
            Self::Sse(transport) => transport.list_tools().await,
            Self::StreamableHttp(transport) => transport.list_tools().await,
            Self::Stdio(transport) => transport.list_tools().await,
            #[cfg(test)]
            Self::Test(TestMcpSetupOutcome::Ok(tools)) => Ok(tools.clone()),
            #[cfg(test)]
            Self::Test(TestMcpSetupOutcome::AuthRequired(message)) => {
                Err(crate::mcp::McpClientError::AuthRequired(message.clone()))
            }
        }
    }

    async fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, crate::mcp::McpClientError> {
        match self {
            Self::Unavailable(message) => {
                Err(crate::mcp::McpClientError::Transport(message.clone()))
            }
            Self::Sse(transport) => transport.call_tool(name, arguments).await,
            Self::StreamableHttp(transport) => transport.call_tool(name, arguments).await,
            Self::Stdio(transport) => transport.call_tool(name, arguments).await,
            #[cfg(test)]
            Self::Test(_) => Err(crate::mcp::McpClientError::Transport(
                "test MCP setup transport does not execute tools".to_string(),
            )),
        }
    }
}

/// Build the Noema GraphQL schema.
#[must_use]
pub fn build_schema(state: GraphqlState) -> GraphqlSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(state)
        .finish()
}

/// Root GraphQL query object.
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Return local Noema status.
    async fn local_status(&self, ctx: &Context<'_>) -> Result<GraphqlLocalStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_status::local_status(state).await
    }

    /// Return onboarding status.
    async fn onboarding_status(&self, ctx: &Context<'_>) -> Result<GraphqlOnboardingStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        onboarding::onboarding_status(state).await
    }

    /// Return a short-lived provider auth attempt.
    async fn provider_auth_attempt(
        &self,
        ctx: &Context<'_>,
        attempt_id: String,
    ) -> Result<Option<GraphqlProviderAuthAttempt>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        onboarding::provider_auth_attempt(state, attempt_id).await
    }

    /// List provider account metadata safe to show in Settings.
    async fn provider_accounts(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlProviderAccount>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::provider_accounts(state).await
    }

    /// List provider account types that can be added in Settings.
    async fn provider_account_catalog(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<GraphqlProviderAccountCatalogEntry>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::provider_account_catalog(state).await
    }

    /// List agent metadata safe to show in Settings.
    async fn agents(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlAgent>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        agents::agents(state).await
    }

    /// Return web fetch settings safe to show in Settings.
    async fn web_fetch_settings(&self, ctx: &Context<'_>) -> Result<GraphqlWebFetchSettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        web_fetch_settings::web_fetch_settings(state).await
    }

    /// Return web tool provider bindings safe to show in Settings.
    async fn web_tool_settings(&self, ctx: &Context<'_>) -> Result<GraphqlWebToolSettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        web_tool_settings::web_tool_settings(state).await
    }

    /// Return Safety usage settings safe to show in Settings.
    async fn usage_settings(&self, ctx: &Context<'_>) -> Result<GraphqlUsageSettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        usage_settings::usage_settings(state).await
    }

    /// List MCP server metadata safe to show in Settings.
    async fn mcp_servers(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlMcpServer>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::mcp_servers(state).await
    }

    /// List MCP tools discovered for one server.
    async fn mcp_tools(
        &self,
        ctx: &Context<'_>,
        mcp_server_id: String,
    ) -> Result<Vec<GraphqlMcpTool>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::mcp_tools(state, mcp_server_id).await
    }

    /// Return a short-lived MCP OAuth setup attempt.
    async fn mcp_oauth_setup_attempt(
        &self,
        ctx: &Context<'_>,
        attempt_id: String,
    ) -> Result<Option<GraphqlMcpOAuthSetupAttempt>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::mcp_oauth_setup_attempt(state, attempt_id).await
    }

    /// List trusted identity selectors for one owner scope.
    async fn trusted_identity_selectors(
        &self,
        ctx: &Context<'_>,
        owner_scope_id: String,
    ) -> Result<Vec<GraphqlTrustedIdentitySelector>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::trusted_identity_selectors(state, owner_scope_id).await
    }

    /// List MCP approval requests safe to show in Settings.
    async fn mcp_approval_requests(
        &self,
        ctx: &Context<'_>,
        status: Option<String>,
    ) -> Result<Vec<GraphqlMcpApprovalRequest>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::mcp_approval_requests(state, status).await
    }

    /// Return memory service settings and readiness status.
    async fn memory_settings(&self, ctx: &Context<'_>) -> Result<GraphqlMemorySettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::memory_settings(state).await
    }

    /// Return graph documents for the local human memory scope.
    async fn memory_graph(
        &self,
        ctx: &Context<'_>,
        input: Option<GraphqlMemoryGraphInput>,
    ) -> Result<GraphqlMemoryGraph> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::memory_graph(state, input.unwrap_or_default()).await
    }

    /// Return the primary conversation identity without creating it or replaying transcript.
    async fn primary_conversation(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GraphqlPrimaryConversation>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::primary_conversation(state).await
    }

    /// Return a cursor-based page of visible conversation transcript items.
    async fn conversation_transcript_page(
        &self,
        ctx: &Context<'_>,
        input: GraphqlConversationTranscriptPageInput,
    ) -> Result<GraphqlConversationTranscriptPage> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::conversation_transcript_page(state, input).await
    }
}

/// Root GraphQL mutation object.
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Start a provider auth attempt.
    async fn start_provider_auth_attempt(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartProviderAuthAttemptInput,
    ) -> Result<GraphqlProviderAuthAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        onboarding::start_provider_auth_attempt(state, input).await
    }

    /// Create a user-managed provider account.
    async fn create_provider_account(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCreateProviderAccountInput,
    ) -> Result<GraphqlProviderAccount> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::create_provider_account(state, input).await
    }

    /// Save a write-only provider account secret.
    async fn save_provider_secret_input(
        &self,
        ctx: &Context<'_>,
        input: GraphqlProviderSecretInput,
    ) -> Result<GraphqlProviderAccount> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::save_provider_secret_input(state, input).await
    }

    /// Clear a write-only provider account secret.
    async fn clear_provider_secret(
        &self,
        ctx: &Context<'_>,
        input: GraphqlClearProviderSecretInput,
    ) -> Result<GraphqlProviderAccount> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::clear_provider_secret(state, input).await
    }

    /// Hard-delete a user-managed provider account.
    async fn delete_provider_account(
        &self,
        ctx: &Context<'_>,
        input: GraphqlDeleteProviderAccountInput,
    ) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::delete_provider_account(state, input).await
    }

    /// Save one agent's model/provider preference.
    async fn save_agent_model_preference(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveAgentModelPreferenceInput,
    ) -> Result<GraphqlAgentModelPreference> {
        let state = ctx.data_unchecked::<GraphqlState>();
        agents::save_agent_model_preference(state, input).await
    }

    /// Save the web fetch summarizer model/provider preference.
    async fn save_web_fetch_summarizer_preference(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveWebFetchSummarizerPreferenceInput,
    ) -> Result<GraphqlAgentModelPreference> {
        let state = ctx.data_unchecked::<GraphqlState>();
        web_fetch_settings::save_web_fetch_summarizer_preference(state, input).await
    }

    /// Save the provider binding for one web tool capability.
    async fn save_web_tool_provider_binding(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveWebToolProviderBindingInput,
    ) -> Result<GraphqlWebToolBindingSettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        web_tool_settings::save_web_tool_provider_binding(state, input).await
    }

    /// Save the tool progress audit model/provider preference.
    async fn save_tool_progress_audit_preference(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveToolProgressAuditPreferenceInput,
    ) -> Result<GraphqlAgentModelPreference> {
        let state = ctx.data_unchecked::<GraphqlState>();
        usage_settings::save_tool_progress_audit_preference(state, input).await
    }

    /// Save memory service settings.
    async fn save_memory_service_settings(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveMemoryServiceSettingsInput,
    ) -> Result<GraphqlMemorySettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::save_memory_service_settings(state, input).await
    }

    /// Force regeneration of the AI-written memory article.
    async fn regenerate_memory_article(&self, ctx: &Context<'_>) -> Result<GraphqlMemoryArticle> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::regenerate_memory_article(state).await
    }

    /// Check memory service readiness and persist the sanitized result.
    async fn check_memory_service(&self, ctx: &Context<'_>) -> Result<GraphqlMemoryServiceStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        memory::check_memory_service(state).await
    }

    /// Ensure the primary conversation exists.
    async fn ensure_primary_conversation(
        &self,
        ctx: &Context<'_>,
        cwd: Option<String>,
    ) -> Result<GraphqlPrimaryConversation> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::ensure_primary_conversation(state, cwd).await
    }

    /// Send a conversation turn.
    async fn send_conversation_turn(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSendConversationTurnInput,
    ) -> Result<GraphqlTurnAccepted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::send_conversation_turn(state, input).await
    }

    /// Send a multiple-choice selection as a user turn.
    async fn send_multiple_choice_selection(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSendMultipleChoiceSelectionInput,
    ) -> Result<GraphqlTurnAccepted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::send_multiple_choice_selection(state, input).await
    }

    /// Save reviewed MCP tool calibration.
    async fn save_tool_calibration(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveToolCalibrationInput,
    ) -> Result<GraphqlToolCalibration> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::save_tool_calibration(state, input).await
    }

    /// Save reviewed MCP tool calibrations in one request.
    async fn save_tool_calibrations(
        &self,
        ctx: &Context<'_>,
        inputs: Vec<GraphqlSaveToolCalibrationInput>,
    ) -> Result<Vec<GraphqlToolCalibration>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::save_tool_calibrations(state, inputs).await
    }

    /// Generate advisory MCP tool calibration suggestions from persisted metadata.
    async fn autofill_tool_calibrations(
        &self,
        ctx: &Context<'_>,
        mcp_server_id: String,
    ) -> Result<GraphqlAutofillToolCalibrationsResult> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::autofill_tool_calibrations(state, mcp_server_id).await
    }

    /// Add and verify an MCP server.
    async fn create_mcp_server(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCreateMcpServerInput,
    ) -> Result<GraphqlMcpServerSetupResult> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::create_mcp_server(state, input).await
    }

    /// Start browser OAuth setup for a hosted MCP server.
    async fn start_mcp_server_oauth_setup(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartMcpServerOAuthSetupInput,
    ) -> Result<GraphqlMcpOAuthSetupAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::start_mcp_server_oauth_setup(state, input).await
    }

    /// Start browser OAuth reauthentication for an existing hosted MCP server.
    async fn start_mcp_server_reauthentication_oauth_setup(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartMcpServerReauthenticationOAuthSetupInput,
    ) -> Result<GraphqlMcpOAuthSetupAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::start_mcp_server_reauthentication_oauth_setup(state, input).await
    }

    /// Continue MCP server setup after adding authentication material.
    async fn continue_mcp_server_setup(
        &self,
        ctx: &Context<'_>,
        input: GraphqlContinueMcpServerSetupInput,
    ) -> Result<GraphqlMcpServerSetupResult> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::continue_mcp_server_setup(state, input).await
    }

    /// Delete an MCP server and its setup secrets.
    async fn delete_mcp_server(&self, ctx: &Context<'_>, mcp_server_id: String) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::delete_mcp_server(state, mcp_server_id).await
    }
}

/// Root GraphQL subscription object.
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Stream conversation events.
    async fn conversation_events(
        &self,
        ctx: &Context<'_>,
        conversation_id: String,
    ) -> impl Stream<Item = GraphqlConversationEvent> {
        let state = ctx.data_unchecked::<GraphqlState>();
        chat::conversation_events(state.subscriptions().clone(), conversation_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{daemon::TurnStreamEvent, graphql::subscriptions::ConversationLiveEvent};
    use futures_util::StreamExt;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn schema_sdl_exposes_initial_noema_fields() {
        let schema = build_schema(GraphqlState::for_tests());
        let sdl = schema.sdl();

        assert!(sdl.contains("type Query"));
        assert!(sdl.contains("localStatus"));
        assert!(sdl.contains("onboardingStatus"));
        assert!(sdl.contains("type Mutation"));
        assert!(sdl.contains("startProviderAuthAttempt"));
        assert!(sdl.contains("primaryConversation"));
        assert!(sdl.contains("conversationTranscriptPage"));
        assert!(sdl.contains("ensurePrimaryConversation"));
        assert!(!sdl.contains("startPrimaryConversation"));
        assert!(sdl.contains("sendConversationTurn"));
        assert!(sdl.contains("saveToolCalibration"));
        assert!(sdl.contains("autofillToolCalibrations"));
        assert!(!sdl.contains("Graphql"));
        assert!(sdl.contains("type AutofillToolCalibrationsResult"));
        assert!(sdl.contains("type ToolCalibrationSuggestion"));
        assert!(sdl.contains("createMcpServer"));
        assert!(sdl.contains("continueMcpServerSetup"));
        assert!(sdl.contains("startMcpServerReauthenticationOauthSetup"));
        assert!(sdl.contains("SaveToolCalibrationInput"));
        assert!(sdl.contains("type ToolCalibration"));
        assert!(sdl.contains("type Subscription"));
        assert!(sdl.contains("conversationEvents"));
        assert!(sdl.contains("AssistantTextDeltaEvent"));
        assert!(sdl.contains("memorySettings"));
        assert!(sdl.contains("memoryGraph"));
        assert!(sdl.contains("saveMemoryServiceSettings"));
        assert!(sdl.contains("checkMemoryService"));
        assert!(sdl.contains("type MemorySettings"));
        assert!(sdl.contains("type MemoryGraph"));
        assert!(sdl.contains("MemoryGraphInput"));
        assert!(sdl.contains("type MemoryServiceStatus"));
        assert!(sdl.contains("SaveMemoryServiceSettingsInput"));
        assert!(!sdl.contains("memoryClaims"));
        assert!(!sdl.contains("memoryClaim("));
        assert!(!sdl.contains("memoryPredicateProposals"));
        assert!(!sdl.contains("memoryPredicateProposal"));
        assert!(sdl.contains("providerAccounts"));
        assert!(sdl.contains("type ProviderAccount"));
        assert!(sdl.contains("agents"));
        assert!(sdl.contains("type Agent"));
        assert!(sdl.contains("saveAgentModelPreference"));
        assert!(sdl.contains("type AgentModelPreference"));
        assert!(sdl.contains("type AgentModelProviderOption"));
        assert!(sdl.contains("type AgentModelProfileOption"));
        assert!(sdl.contains("webFetchSettings"));
        assert!(sdl.contains("type WebFetchSettings"));
        assert!(sdl.contains("type WebFetchSummarizerSettings"));
        assert!(sdl.contains("webToolSettings"));
        assert!(sdl.contains("type WebToolSettings"));
        assert!(sdl.contains("type WebToolBindingSettings"));
        assert!(sdl.contains("type WebToolProviderOption"));
        assert!(sdl.contains("saveWebFetchSummarizerPreference"));
        assert!(sdl.contains("SaveWebFetchSummarizerPreferenceInput"));
        assert!(sdl.contains("saveWebToolProviderBinding"));
        assert!(sdl.contains("SaveWebToolProviderBindingInput"));
        assert!(sdl.contains("agentId"));
        assert!(sdl.contains("displayName"));
        assert!(sdl.contains("isPrimary"));
        assert!(sdl.contains("mcpServers"));
        assert!(sdl.contains("type McpServer"));
        assert!(sdl.contains("mcpTools"));
        assert!(sdl.contains("type McpTool"));
        assert!(sdl.contains("trustedIdentitySelectors"));
        assert!(sdl.contains("type TrustedIdentitySelector"));
        assert!(sdl.contains("mcpApprovalRequests"));
        assert!(sdl.contains("type McpApprovalRequest"));
        assert!(!sdl.contains("type MemoryClaim"));
        assert!(!sdl.contains("type MemoryClaimEvidence"));
        assert!(!sdl.contains("type PredicateProposal"));
        assert!(sdl.contains("MemoryGraph"));
        assert!(sdl.contains("MemoryGraphInput"));
    }

    async fn schema_with_reasoning_openai_profile() -> (GraphqlSchema, String) {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let account_id = "provider_account:openai:reasoning";
        crate::store::tests::insert_provider_account_for_tests(
            &store,
            account_id,
            "openai",
            "reasoning",
            "OpenAI reasoning",
            crate::ProviderAuthMethod::SecretInput,
            true,
            crate::ProviderAccountStatus::Authenticated,
            json!({}),
        )
        .await;
        store
            .update_provider_account_metadata(
                account_id,
                serde_json::json!({
                    "profiles": [{
                        "id": "gpt-5.5",
                        "label": "GPT-5.5",
                        "reasoning_efforts": ["low", "medium", "high"],
                        "default_reasoning_effort": "medium"
                    }]
                }),
            )
            .await
            .expect("metadata");
        (
            build_schema(GraphqlState::for_tests_with_store(store)),
            account_id.to_string(),
        )
    }

    async fn spawn_memory_graph_mnemosyne_server() -> String {
        spawn_memory_graph_mnemosyne_server_with_limit(25).await
    }

    async fn spawn_memory_graph_mnemosyne_server_with_limit(limit: u16) -> String {
        let results = json!([
            {
                "id": "mem_1",
                "memory": "Kevin prefers local-first tools",
                "metadata": {"sourceKind": "user_message"},
                "created_at": "2026-07-08T00:00:30.000Z",
                "updated_at": "2026-07-08T00:01:00.000Z"
            },
            {
                "id": "mem_2",
                "memory": "Kevin likes tools that keep data local",
                "metadata": {
                    "noemaConversationId": "abc",
                    "sourceObservation": "I prefer local-first tools."
                },
                "created_at": "2026-07-08T00:00:40.000Z",
                "updated_at": "2026-07-08T00:01:10.000Z"
            }
        ]);
        spawn_memory_graph_mnemosyne_server_with_results(limit, results).await
    }

    async fn spawn_memory_graph_mnemosyne_server_with_results(
        limit: u16,
        results: serde_json::Value,
    ) -> String {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let base_url = format!(
            "http://{}",
            listener.local_addr().expect("listener address")
        );
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0_u8; 8192];
            let read = stream.read(&mut buffer).await.expect("read");
            let request = String::from_utf8_lossy(&buffer[..read]);
            let (head, _body) = request.split_once("\r\n\r\n").expect("request head");
            let mut lines = head.lines();
            let request_line = lines.next().expect("request line");
            assert_eq!(
                request_line,
                format!("GET /v1/memories?user_id=human%3Alocal&limit={limit} HTTP/1.1")
            );

            let response = json!({ "results": results });
            let response_body = serde_json::to_vec(&response).expect("response JSON");
            let response_head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                response_body.len()
            );
            stream
                .write_all(response_head.as_bytes())
                .await
                .expect("write head");
            stream.write_all(&response_body).await.expect("write body");
        });

        base_url
    }

    #[tokio::test]
    async fn memory_settings_query_returns_defaults() {
        let store = crate::store::tests::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                "{ memorySettings { mode baseUrl port status { status } } }",
            ))
            .await
            .into_result()
            .expect("query");

        assert_eq!(
            response.data,
            async_graphql::Value::from_json(serde_json::json!({
                "memorySettings": {
                    "mode": "MANAGED",
                    "baseUrl": null,
                    "port": null,
                    "status": {"status": "UNAVAILABLE"}
                }
            }))
            .expect("json")
        );
    }

    #[tokio::test]
    async fn memory_settings_query_reports_managed_mnemosyne_unavailable() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
            .await
            .expect("store");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_paths(store, paths));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memorySettings {
                    status {
                      status
                      lastErrorCode
                      lastErrorMessage
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memorySettings"]["status"]["status"], "UNAVAILABLE");
        assert_eq!(
            data["memorySettings"]["status"]["lastErrorCode"],
            "mnemosyne_unavailable"
        );
        assert_eq!(
            data["memorySettings"]["status"]["lastErrorMessage"],
            "Managed Mnemosyne is not running"
        );
    }

    #[tokio::test]
    async fn memory_graph_returns_unavailable_without_memory_connection() {
        let store = crate::store::tests::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    status {
                      status
                      lastErrorCode
                    }
                    documents {
                      id
                    }
                    article {
                      title
                      markdown
                      isGenerated
                    }
                    pageInfo {
                      page
                      limit
                      hasMore
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memoryGraph"]["status"]["status"], "UNAVAILABLE");
        assert_eq!(
            data["memoryGraph"]["status"]["lastErrorCode"],
            "mnemosyne_unavailable"
        );
        assert_eq!(data["memoryGraph"]["documents"], json!([]));
        assert_eq!(data["memoryGraph"]["article"]["title"], "Local human");
        assert_eq!(
            data["memoryGraph"]["article"]["markdown"],
            "# Local human\n\nLittle is currently known about Local human."
        );
        assert_eq!(data["memoryGraph"]["article"]["isGenerated"], false);
        assert_eq!(data["memoryGraph"]["pageInfo"]["page"], 1);
        assert_eq!(data["memoryGraph"]["pageInfo"]["limit"], 25);
        assert_eq!(data["memoryGraph"]["pageInfo"]["hasMore"], false);
    }

    #[tokio::test]
    async fn memory_graph_lists_external_mnemosyne_memories() {
        let server_base_url = spawn_memory_graph_mnemosyne_server().await;
        let store = crate::store::tests::test_store().await;
        store
            .save_memory_service_settings(crate::SaveMemoryServiceSettings {
                mode: crate::MemoryServiceMode::External,
                base_url: Some(server_base_url),
                port: None,
                provider_account_id: None,
                provider_kind: None,
                model_profile: None,
                reasoning_effort: None,
            })
            .await
            .expect("settings");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    status {
                      status
                      lastErrorCode
                    }
                    documents {
                      id
                      title
                      memoryEntries {
                        id
                        documentId
                        content
                        source {
                          kind
                          conversationId
                          turnId
                          itemId
                          messageText
                        }
                        metadata
                        spaceContainerTag
                        parentMemoryId
                        rootMemoryId
                        memoryRelations
                      }
                    }
                    article {
                      title
                      subtitle
                      markdown
                      isGenerated
                    }
                    pageInfo {
                      page
                      limit
                      hasMore
                      total
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memoryGraph"]["status"]["status"], "READY");
        assert_eq!(data["memoryGraph"]["article"]["title"], "Local human");
        assert_eq!(
            data["memoryGraph"]["article"]["markdown"],
            "# Local human\n\nLocal human is described by the currently available biographical facts.\n\nKevin prefers local-first tools\n\nKevin likes tools that keep data local"
        );
        assert_eq!(data["memoryGraph"]["article"]["isGenerated"], false);
        assert_eq!(
            data["memoryGraph"]["documents"][0]["id"],
            "conversation:abc"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["id"],
            "mem_2"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["documentId"],
            "conversation:abc"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["spaceContainerTag"],
            "human:local"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["metadata"]["sourceObservation"],
            "I prefer local-first tools."
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["source"]["conversationId"],
            "abc"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["source"]["messageText"],
            "I prefer local-first tools."
        );
        assert_eq!(
            data["memoryGraph"]["documents"][1]["id"],
            "mnemosyne:human:local"
        );
        assert_eq!(data["memoryGraph"]["documents"][1]["title"], "Human memory");
        assert_eq!(data["memoryGraph"]["pageInfo"]["hasMore"], false);
        assert_eq!(data["memoryGraph"]["pageInfo"]["total"], 2);
    }

    #[tokio::test]
    async fn memory_graph_source_resolves_exact_persisted_user_message() {
        let store = crate::store::tests::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                "codex",
                Some("gpt-test".to_string()),
                None,
            )
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(crate::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let user_item = store
            .append_conversation_item(crate::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: crate::ConversationItemKind::UserText,
                status: crate::ConversationItemStatus::Completed,
                author: crate::ActorRef::human("human:local"),
                content_text: Some("I like airplanes and local-first tools.".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("user item");
        let server_base_url = spawn_memory_graph_mnemosyne_server_with_results(
            25,
            json!([
                {
                    "id": "mem_exact",
                    "memory": "Kevin likes airplanes and local-first tools",
                    "metadata": {
                        "noemaConversationId": conversation.conversation_id,
                        "turnId": turn.turn_id,
                        "userItemId": user_item.item_id,
                        "sourceKind": "user_message",
                        "sourceObservation": "stale copied text"
                    },
                    "created_at": "2026-07-08T00:00:40.000Z",
                    "updated_at": "2026-07-08T00:01:10.000Z"
                }
            ]),
        )
        .await;
        store
            .save_memory_service_settings(crate::SaveMemoryServiceSettings {
                mode: crate::MemoryServiceMode::External,
                base_url: Some(server_base_url),
                port: None,
                provider_account_id: None,
                provider_kind: None,
                model_profile: None,
                reasoning_effort: None,
            })
            .await
            .expect("settings");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    documents {
                      memoryEntries {
                        source {
                          kind
                          conversationId
                          turnId
                          itemId
                          messageText
                        }
                      }
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");
        let source = &data["memoryGraph"]["documents"][0]["memoryEntries"][0]["source"];

        assert_eq!(source["kind"], "user_message");
        assert_eq!(
            source["messageText"],
            "I like airplanes and local-first tools."
        );
        assert_ne!(source["messageText"], "stale copied text");
    }

    #[tokio::test]
    async fn memory_graph_lazily_generates_and_caches_article() {
        let server_base_url = spawn_memory_graph_mnemosyne_server().await;
        let store = crate::store::tests::test_store().await;
        store
            .save_memory_service_settings(crate::SaveMemoryServiceSettings {
                mode: crate::MemoryServiceMode::External,
                base_url: Some(server_base_url),
                port: None,
                provider_account_id: None,
                provider_kind: None,
                model_profile: None,
                reasoning_effort: None,
            })
            .await
            .expect("settings");
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            "# Kevin\n\nKevin prefers local-first tools and tools that keep data local.",
            Some("test-memory-writer"),
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    article {
                      title
                      markdown
                      isGenerated
                      generatedAt
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memoryGraph"]["article"]["title"], "Kevin");
        assert_eq!(
            data["memoryGraph"]["article"]["markdown"],
            "# Kevin\n\nKevin prefers local-first tools and tools that keep data local."
        );
        assert_eq!(data["memoryGraph"]["article"]["isGenerated"], true);
        assert!(data["memoryGraph"]["article"]["generatedAt"].is_string());
        {
            let requests = requests.lock().expect("requests");
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].model.as_deref(), None);
            match &requests[0].input {
                crate::provider::GenerateInput::Text(prompt) => {
                    assert!(prompt.contains("Kevin prefers local-first tools"));
                    assert!(prompt.contains("User-authored source observation"));
                    assert!(prompt.contains("do not turn a preference about another speaker"));
                    assert!(prompt.contains("Return Markdown only"));
                }
                other => panic!("unexpected memory article input: {other:?}"),
            }
        }
        let cached = store
            .memory_article_cache("human:local")
            .await
            .expect("article cache")
            .expect("article cache row");
        assert_eq!(
            cached.article_markdown,
            "# Kevin\n\nKevin prefers local-first tools and tools that keep data local."
        );
    }

    #[tokio::test]
    async fn memory_article_uses_configured_memory_model_preference() {
        let server_base_url = spawn_memory_graph_mnemosyne_server().await;
        let store = crate::store::tests::test_store().await;
        store
            .save_memory_service_settings(crate::SaveMemoryServiceSettings {
                mode: crate::MemoryServiceMode::External,
                base_url: Some(server_base_url),
                port: None,
                provider_account_id: Some("provider_account:codex:memory".to_string()),
                provider_kind: Some("codex".to_string()),
                model_profile: Some("memory-writer".to_string()),
                reasoning_effort: Some(crate::provider::ReasoningEffort::High),
            })
            .await
            .expect("settings");
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            "# Kevin\n\nKevin prefers local-first tools and tools that keep data local.",
            Some("wrong-tool-classifier"),
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        schema
            .execute(async_graphql::Request::new(
                "{ memoryGraph(input: { page: 1, limit: 25 }) { article { title } } }",
            ))
            .await
            .into_result()
            .expect("query");

        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].model.as_deref(), Some("memory-writer"));
        assert_eq!(
            requests[0].options.reasoning_effort,
            Some(crate::provider::ReasoningEffort::High)
        );
    }

    #[tokio::test]
    async fn regenerate_memory_article_forces_generation() {
        let server_base_url = spawn_memory_graph_mnemosyne_server_with_limit(100).await;
        let store = crate::store::tests::test_store().await;
        store
            .save_memory_service_settings(crate::SaveMemoryServiceSettings {
                mode: crate::MemoryServiceMode::External,
                base_url: Some(server_base_url),
                port: None,
                provider_account_id: None,
                provider_kind: None,
                model_profile: None,
                reasoning_effort: None,
            })
            .await
            .expect("settings");
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            "# Kevin\n\nKevin is freshly regenerated.",
            Some("test-memory-writer"),
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  regenerateMemoryArticle {
                    title
                    markdown
                    isGenerated
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("mutation");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["regenerateMemoryArticle"]["title"], "Kevin");
        assert_eq!(
            data["regenerateMemoryArticle"]["markdown"],
            "# Kevin\n\nKevin is freshly regenerated."
        );
        assert_eq!(data["regenerateMemoryArticle"]["isGenerated"], true);
        assert_eq!(requests.lock().expect("requests").len(), 1);
    }

    #[tokio::test]
    async fn save_external_memory_service_settings_requires_base_url() {
        let store = crate::store::tests::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  saveMemoryServiceSettings(input: {
                    mode: EXTERNAL
                  }) {
                    mode
                  }
                }
                "#,
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("base URL"));
    }

    #[tokio::test]
    async fn old_memory_graph_nodes_field_is_not_in_schema() {
        let schema = build_schema(GraphqlState::for_tests());
        let response = schema
            .execute(async_graphql::Request::new(
                "{ memoryGraph { nodes { nodeId } } }",
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("nodes"));
    }

    #[tokio::test]
    async fn check_memory_service_times_out_when_socket_never_responds() {
        use tokio::{
            net::TcpListener,
            time::{Duration, timeout},
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("listener address");
        let _server = tokio::spawn(async move {
            if let Ok((_socket, _peer)) = listener.accept().await {
                futures_util::future::pending::<()>().await;
            }
        });

        let store = crate::store::tests::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let save_response = schema
            .execute(format!(
                r#"
                mutation {{
                  saveMemoryServiceSettings(input: {{
                    mode: EXTERNAL
                    baseUrl: "http://{address}"
                  }}) {{
                    baseUrl
                  }}
                }}
                "#
            ))
            .await;
        assert!(
            save_response.errors.is_empty(),
            "{:?}",
            save_response.errors
        );

        let response = timeout(
            Duration::from_secs(3),
            schema.execute(async_graphql::Request::new(
                r#"
                mutation {
                  checkMemoryService {
                    status
                    lastErrorCode
                  }
                }
                "#,
            )),
        )
        .await
        .expect("readiness check should not hang indefinitely");

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["checkMemoryService"]["status"], "UNAVAILABLE");
        assert_eq!(
            data["checkMemoryService"]["lastErrorCode"],
            "request_failed"
        );
    }

    #[tokio::test]
    async fn save_memory_service_settings_rejects_reasoning_effort_without_model() {
        let store = crate::store::tests::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  saveMemoryServiceSettings(input: {
                    mode: MANAGED
                    reasoningEffort: HIGH
                  }) {
                    mode
                  }
                }
                "#,
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("reasoning effort"));
    }

    #[test]
    fn graphql_state_for_tests_has_runtime_state_accessors() {
        let state = GraphqlState::for_tests();
        assert!(state.optional_store().is_none());
        assert_eq!(
            state.memory_storage(),
            crate::graphql::local_status::GraphqlMemoryStorageStatus::Ready
        );
    }

    #[tokio::test]
    async fn provider_accounts_query_returns_safe_metadata() {
        use crate::{ProviderAccountStatus, store::tests::test_store};

        let store = test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                ProviderAccountStatus::Authenticated,
                Some("codex_ok"),
                Some("Codex credentials are usable"),
            )
            .await
            .expect("status update");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccounts {
                    providerAccountId
                    providerKind
                    accountKey
                    displayName
                    authMethod
                    status
                    isActive
                    isDefault
                    lastCheckedAt
                    lastAuthenticatedAt
                    lastErrorCode
                    lastErrorMessage
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let account = &data["providerAccounts"][0];
        assert_eq!(
            account["providerAccountId"],
            "provider_account:codex:default"
        );
        assert_eq!(account["providerKind"], "codex");
        assert_eq!(account["accountKey"], "default");
        assert_eq!(account["displayName"], "Codex");
        assert_eq!(account["authMethod"], "oauth_device_code");
        assert_eq!(account["status"], "AUTHENTICATED");
        assert_eq!(account["isActive"], true);
        assert_eq!(account["isDefault"], true);
        assert_eq!(account["lastErrorCode"], "codex_ok");
        assert_eq!(account["lastErrorMessage"], "Codex credentials are usable");

        let json_text = serde_json::to_string(&data).expect("provider json");
        assert!(!json_text.contains("auth.json"));
        assert!(!json_text.contains("codex_tokens.json"));
        assert!(!json_text.contains("api_key"));
        assert!(!json_text.contains("token"));
    }

    #[tokio::test]
    async fn provider_account_catalog_lists_exa() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccountCatalog {
                    providerKind
                    displayName
                    authMethod
                    capabilities { capabilityId }
                  }
                  providerAccounts { providerKind }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["providerAccountCatalog"][0]["providerKind"], "exa");
        assert!(
            data["providerAccounts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|account| { account["providerKind"] != "exa" })
        );
    }

    #[tokio::test]
    async fn create_exa_provider_account_stores_secret_without_returning_it() {
        use crate::{NoemaPaths, store::tests::test_store};

        let dir = tempfile::tempdir().expect("tempdir");
        let store = test_store().await;
        let state = GraphqlState::for_tests_with_store_and_paths(
            store,
            NoemaPaths::from_noema_home(dir.path()).expect("paths"),
        );
        let schema = build_schema(state);

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  createProviderAccount(input: {
                    providerKind: "exa"
                    displayName: "Research"
                    secret: "secret-key"
                  }) {
                    providerAccountId
                    providerKind
                    accountKey
                    displayName
                    authMethod
                    status
                    lastErrorMessage
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let value = response.data.into_json().expect("json");
        let account = &value["createProviderAccount"];
        assert_eq!(account["providerKind"], "exa");
        assert_eq!(account["displayName"], "Research");
        assert_eq!(account["status"], "AUTHENTICATED");
        assert!(!value.to_string().contains("secret-key"));
        let account_key = account["accountKey"].as_str().expect("account key");
        assert!(
            dir.path()
                .join(format!("providers/exa/{account_key}/api_key.json"))
                .is_file()
        );
    }

    #[tokio::test]
    async fn provider_accounts_query_includes_created_exa_accounts() {
        use crate::{NoemaPaths, store::tests::test_store};

        let dir = tempfile::tempdir().expect("tempdir");
        let store = test_store().await;
        let state = GraphqlState::for_tests_with_store_and_paths(
            store,
            NoemaPaths::from_noema_home(dir.path()).expect("paths"),
        );
        let schema = build_schema(state);

        let create_response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  createProviderAccount(input: {
                    providerKind: "exa"
                    displayName: "Research"
                    secret: "secret-key"
                  }) {
                    providerAccountId
                  }
                }
                "#,
            ))
            .await;
        assert!(
            create_response.errors.is_empty(),
            "{:?}",
            create_response.errors
        );

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccounts {
                    providerKind
                    displayName
                    authMethod
                    isDefault
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let accounts = response.data.into_json().expect("json")["providerAccounts"]
            .as_array()
            .expect("accounts")
            .clone();
        assert!(accounts.iter().any(|account| {
            account["providerKind"] == "exa"
                && account["displayName"] == "Research"
                && account["authMethod"] == "secret_input"
                && account["isDefault"] == false
        }));
    }

    #[tokio::test]
    async fn delete_provider_account_removes_exa_account_secret_and_binding() {
        use crate::{NoemaPaths, store::tests::test_store};

        let dir = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let store = test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_paths(
            store.clone(),
            paths.clone(),
        ));

        let create_response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  createProviderAccount(input: {
                    providerKind: "exa"
                    displayName: "Research"
                    secret: "secret-key"
                  }) {
                    providerAccountId
                    accountKey
                  }
                }
                "#,
            ))
            .await;
        assert!(
            create_response.errors.is_empty(),
            "{:?}",
            create_response.errors
        );
        let create_data = create_response.data.into_json().expect("json");
        let created = &create_data["createProviderAccount"];
        let provider_account_id = created["providerAccountId"]
            .as_str()
            .expect("provider account id");
        let account_key = created["accountKey"].as_str().expect("account key");
        let account_home = paths.provider_account_home("exa", account_key);
        assert!(account_home.join("api_key.json").is_file());
        store
            .upsert_provider_capability_binding("web.search", "web.search", provider_account_id)
            .await
            .expect("save binding");

        let delete_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  deleteProviderAccount(input: {{
                    providerAccountId: "{provider_account_id}"
                  }})
                }}
                "#
            )))
            .await;

        assert!(
            delete_response.errors.is_empty(),
            "{:?}",
            delete_response.errors
        );
        let delete_data = delete_response.data.into_json().expect("json");
        assert_eq!(delete_data["deleteProviderAccount"], true);
        assert!(
            store
                .get_provider_account(provider_account_id)
                .await
                .expect("load provider")
                .is_none()
        );
        assert!(
            store
                .provider_capability_binding("web.search", "web.search")
                .await
                .expect("load binding")
                .is_none()
        );
        assert!(!account_home.exists());
    }

    #[tokio::test]
    async fn delete_provider_account_rejects_default_accounts() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  deleteProviderAccount(input: {
                    providerAccountId: "provider_account:codex:default"
                  })
                }
                "#,
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(
            response.errors[0]
                .message
                .contains("default provider accounts cannot be deleted")
        );
    }

    #[tokio::test]
    async fn provider_accounts_query_returns_all_active_default_accounts() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccounts {
                    providerKind
                    accountKey
                    displayName
                    authMethod
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let accounts = response.data.into_json().expect("json")["providerAccounts"]
            .as_array()
            .expect("accounts")
            .clone();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0]["providerKind"], "codex");
        assert_eq!(accounts[1]["providerKind"], "foundation_local");
        assert_eq!(accounts[1]["authMethod"], "none");
    }

    #[tokio::test]
    async fn agents_query_returns_safe_agent_metadata() {
        use crate::{NewAgent, store::tests::test_store};

        let store = test_store().await;
        store.ensure_default_actors().await.expect("default actors");
        store
            .update_agent_display_name("agent:primary", "Noema")
            .await
            .expect("name primary");
        store
            .create_agent(NewAgent {
                agent_id: "agent:unnamed".to_string(),
                display_name: None,
            })
            .await
            .expect("unnamed agent");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    agentId
                    displayName
                    isPrimary
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let agents = data["agents"].as_array().expect("agents array");
        assert_eq!(agents[0]["agentId"], "agent:primary");
        assert_eq!(agents[0]["displayName"], "Noema");
        assert_eq!(agents[0]["isPrimary"], true);
        assert_eq!(agents[1]["agentId"], "agent:unnamed");
        assert_eq!(agents[1]["displayName"], serde_json::Value::Null);
        assert_eq!(agents[1]["isPrimary"], false);

        let json_text = serde_json::to_string(&data).expect("agent json");
        assert!(!json_text.contains("prompt"));
        assert!(!json_text.contains("memory"));
        assert!(!json_text.contains("runtime"));
        assert!(!json_text.contains("credential"));
        assert!(!json_text.contains("conversation"));
    }

    #[tokio::test]
    async fn agents_query_exposes_model_preference_options() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.5", "label": "GPT-5.5" },
                        { "id": "gpt-5.4", "label": "GPT-5.4" },
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4-Mini" },
                        { "id": "gpt-5.3-codex-spark", "label": "GPT-5.3-Codex-Spark" }
                    ],
                    "models_source": "codex_models_endpoint"
                }),
            )
            .await
            .expect("codex metadata");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
                agent_id: "agent:primary".to_string(),
                provider_kind: "foundation_local".to_string(),
                provider_account_id: foundation.provider_account_id,
                model_profile: "default".to_string(),
                reasoning_effort: None,
            })
            .await
            .expect("preference");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    agentId
                    modelPreference {
                      providerKind
                      providerAccountId
                      modelProfile
                    }
                    modelOptions {
                      providerKind
                      providerAccountId
                      providerDisplayName
                      status
                      profiles {
                        id
                        label
                        disabledReason
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let agent = &response.data.into_json().expect("json")["agents"][0];
        assert_eq!(agent["modelPreference"]["providerKind"], "foundation_local");
        assert_eq!(agent["modelPreference"]["modelProfile"], "default");
        assert_eq!(agent["modelOptions"][0]["providerKind"], "codex");
        let codex_profile_ids: Vec<_> = agent["modelOptions"][0]["profiles"]
            .as_array()
            .expect("codex profiles")
            .iter()
            .map(|profile| profile["id"].as_str().expect("profile id"))
            .collect();
        assert_eq!(
            codex_profile_ids,
            ["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex-spark"]
        );
        assert_eq!(agent["modelOptions"][1]["providerKind"], "foundation_local");
        assert_eq!(
            agent["modelOptions"][1]["profiles"][0]["label"],
            "Default on-device"
        );
    }

    #[tokio::test]
    async fn agents_query_exposes_profile_reasoning_efforts() {
        let (schema, _) = schema_with_reasoning_openai_profile().await;
        let response = schema
            .execute(
                r#"
              query {
                agents {
                  modelOptions {
                    profiles {
                      id
                      reasoningEfforts
                      defaultReasoningEffort
                    }
                  }
                }
              }
            "#,
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let profile = &data["agents"][0]["modelOptions"][0]["profiles"][0];
        assert_eq!(
            profile["reasoningEfforts"],
            serde_json::json!(["LOW", "MEDIUM", "HIGH"])
        );
        assert_eq!(profile["defaultReasoningEffort"], "MEDIUM");
    }

    #[tokio::test]
    async fn codex_profile_metadata_reasoning_efforts_are_exposed() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("status");
        store
            .update_provider_account_metadata(
                &account.provider_account_id,
                serde_json::json!({
                    "profiles": [{
                        "id": "gpt-5.5",
                        "label": "GPT-5.5",
                        "reasoning_efforts": ["low", "medium", "high"],
                        "default_reasoning_effort": "medium"
                    }]
                }),
            )
            .await
            .expect("metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(
                r#"
              query {
                agents {
                  modelOptions {
                    providerKind
                    providerAccountId
                    profiles {
                      id
                      reasoningEfforts
                      defaultReasoningEffort
                    }
                  }
                }
              }
            "#,
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let codex_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("model options")
            .iter()
            .find(|option| option["providerKind"] == "codex")
            .expect("codex option");
        let profile = &codex_option["profiles"][0];
        assert_eq!(
            profile["reasoningEfforts"],
            serde_json::json!(["LOW", "MEDIUM", "HIGH"])
        );
        assert_eq!(profile["defaultReasoningEffort"], "MEDIUM");

        let account_id = codex_option["providerAccountId"]
            .as_str()
            .expect("provider account id");
        let save_without_reasoning = schema
            .execute(format!(
                r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5"
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(
            !save_without_reasoning.errors.is_empty(),
            "missing reasoning effort should be rejected"
        );
        assert_eq!(
            save_without_reasoning.errors[0].message,
            "reasoning effort is required for selected model profile"
        );

        let save_with_reasoning = schema
            .execute(format!(
                r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: HIGH
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(
            save_with_reasoning.errors.is_empty(),
            "{:?}",
            save_with_reasoning.errors
        );
        let data = save_with_reasoning.data.into_json().expect("json");
        assert_eq!(data["saveAgentModelPreference"]["reasoningEffort"], "HIGH");
    }

    #[tokio::test]
    async fn agents_query_does_not_invent_remote_model_profiles() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    modelOptions {
                      providerKind
                      profiles {
                        id
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let codex_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("options")
            .iter()
            .find(|option| option["providerKind"] == "codex")
            .expect("codex option");
        assert!(
            codex_option["profiles"]
                .as_array()
                .expect("codex profiles")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn web_fetch_settings_query_defaults_to_tool_model_and_returns_options() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" },
                        { "id": "gpt-5.5", "label": "GPT-5.5" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  webFetchSettings {
                    summarizer {
                      defaultModelProfile
                      modelPreference {
                        providerKind
                      }
                      modelOptions {
                        providerKind
                        providerAccountId
                        providerDisplayName
                        status
                        disabledReason
                        profiles {
                          id
                          label
                          disabledReason
                        }
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let summarizer = &data["webFetchSettings"]["summarizer"];
        assert_eq!(summarizer["defaultModelProfile"], "gpt-5.4-mini");
        assert_eq!(summarizer["modelPreference"], serde_json::Value::Null);
        assert_eq!(summarizer["modelOptions"][0]["providerKind"], "codex");
        assert_eq!(
            summarizer["modelOptions"][0]["profiles"][0]["id"],
            "gpt-5.4-mini"
        );
    }

    #[tokio::test]
    async fn web_tool_settings_query_returns_default_system_bindings() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  webToolSettings {
                    search {
                      toolName
                      capabilityId
                      activeProviderAccountId
                      providerOptions {
                        providerAccountId
                        providerKind
                      }
                    }
                    fetch {
                      toolName
                      capabilityId
                      activeProviderAccountId
                      providerOptions {
                        providerAccountId
                        providerKind
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["webToolSettings"]["search"]["activeProviderAccountId"],
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(
            data["webToolSettings"]["fetch"]["activeProviderAccountId"],
            "provider_account:direct_http:system"
        );
        assert_eq!(
            data["webToolSettings"]["search"]["providerOptions"][0]["providerKind"],
            "duckduckgo_public"
        );
        assert_eq!(
            data["webToolSettings"]["fetch"]["providerOptions"][0]["providerKind"],
            "direct_http"
        );
    }

    #[tokio::test]
    async fn save_web_tool_provider_binding_mutation_returns_saved_binding() {
        use crate::{ProviderAccountStatus, store::tests::test_store};

        let store = test_store().await;
        crate::store::tests::insert_provider_account_for_tests(
            &store,
            "provider_account:openai:default",
            "openai",
            "default",
            "OpenAI default",
            crate::ProviderAuthMethod::SecretInput,
            true,
            ProviderAccountStatus::Authenticated,
            json!({}),
        )
        .await;

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  saveWebToolProviderBinding(input: {
                    toolName: "web.search"
                    capabilityId: "web.search"
                    providerAccountId: "provider_account:openai:default"
                  }) {
                    toolName
                    capabilityId
                    activeProviderAccountId
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["saveWebToolProviderBinding"]["toolName"], "web.search");
        assert_eq!(
            data["saveWebToolProviderBinding"]["activeProviderAccountId"],
            "provider_account:openai:default"
        );
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_persists_valid_codex_profile() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &codex.provider_account_id,
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("codex authenticated");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveWebFetchSummarizerPreference(input: {{
                    providerAccountId: "{}"
                    modelProfile: "gpt-5.4-mini"
                  }}) {{
                    providerKind
                    providerAccountId
                    modelProfile
                  }}
                }}
                "#,
                codex.provider_account_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let saved = store
            .get_auxiliary_model_preference(crate::WEB_FETCH_SUMMARIZER_TASK_ID)
            .await
            .expect("preference read")
            .expect("preference saved");
        assert_eq!(saved.provider_kind, "codex");
        assert_eq!(saved.provider_account_id, codex.provider_account_id);
        assert_eq!(saved.model_profile, "gpt-5.4-mini");
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_persists_reasoning_effort() {
        let (schema, account_id) = schema_with_reasoning_openai_profile().await;
        let response = schema
            .execute(format!(
                r#"
            mutation {{
              saveWebFetchSummarizerPreference(input: {{
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: LOW
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["saveWebFetchSummarizerPreference"]["reasoningEffort"],
            "LOW"
        );
    }

    #[tokio::test]
    async fn usage_settings_query_exposes_progress_audit_default() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" },
                        { "id": "gpt-5.5", "label": "GPT-5.5" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  usageSettings {
                    progressAudit {
                      defaultModelProfile
                      modelPreference { providerKind }
                      modelOptions { providerKind profiles { id } }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let audit = &data["usageSettings"]["progressAudit"];
        assert_eq!(audit["defaultModelProfile"], "gpt-5.4-mini");
        assert_eq!(audit["modelPreference"], serde_json::Value::Null);
        assert_eq!(
            audit["modelOptions"][0]["profiles"][0]["id"],
            "gpt-5.4-mini"
        );
    }

    #[tokio::test]
    async fn usage_settings_query_exposes_provider_specific_progress_audit_defaults() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" },
                        { "id": "gpt-5.5", "label": "GPT-5.5" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  usageSettings {
                    progressAudit {
                      modelOptions {
                        providerKind
                        defaultModelProfile
                        profiles { id }
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let options = data["usageSettings"]["progressAudit"]["modelOptions"]
            .as_array()
            .expect("model options");
        let codex = options
            .iter()
            .find(|option| option["providerKind"] == "codex")
            .expect("codex option");
        let foundation = options
            .iter()
            .find(|option| option["providerKind"] == "foundation_local")
            .expect("foundation option");

        assert_eq!(codex["defaultModelProfile"], "gpt-5.4-mini");
        assert_eq!(foundation["defaultModelProfile"], "default");
        assert_ne!(foundation["defaultModelProfile"], "gpt-5.4-mini");
    }

    #[tokio::test]
    async fn save_tool_progress_audit_preference_persists_valid_codex_profile() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &codex.provider_account_id,
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("codex authenticated");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveToolProgressAuditPreference(input: {{
                    providerAccountId: "{}"
                    modelProfile: "gpt-5.4-mini"
                  }}) {{
                    providerKind
                    providerAccountId
                    modelProfile
                  }}
                }}
                "#,
                codex.provider_account_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let saved = store
            .get_auxiliary_model_preference(crate::store::TOOL_PROGRESS_AUDIT_TASK_ID)
            .await
            .expect("preference read")
            .expect("preference saved");
        assert_eq!(saved.provider_kind, "codex");
        assert_eq!(saved.provider_account_id, codex.provider_account_id);
        assert_eq!(saved.model_profile, "gpt-5.4-mini");
    }

    #[tokio::test]
    async fn save_tool_progress_audit_preference_persists_reasoning_effort() {
        let (schema, account_id) = schema_with_reasoning_openai_profile().await;
        let response = schema
            .execute(format!(
                r#"
            mutation {{
              saveToolProgressAuditPreference(input: {{
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: MEDIUM
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["saveToolProgressAuditPreference"]["reasoningEffort"],
            "MEDIUM"
        );
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_rejects_unavailable_provider() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                crate::ProviderAccountStatus::Unavailable,
                Some("unsupported_platform"),
                Some("/Users/alice/.secret/token.txt failed with token abc123"),
            )
            .await
            .expect("status");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveWebFetchSummarizerPreference(input: {{
                    providerAccountId: "{}"
                    modelProfile: "default"
                  }}) {{
                    providerKind
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert_eq!(response.errors.len(), 1);
        let message = response.errors[0].message.as_str();
        assert_eq!(message, "Provider is unavailable on this platform.");
        assert!(!message.contains("/Users/alice"));
        assert!(!message.contains("abc123"));
        assert!(
            store
                .get_auxiliary_model_preference(crate::WEB_FETCH_SUMMARIZER_TASK_ID)
                .await
                .expect("preference read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_rejects_unavailable_profile() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &codex.provider_account_id,
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("codex authenticated");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveWebFetchSummarizerPreference(input: {{
                    providerAccountId: "{}"
                    modelProfile: "gpt-5.4-mini"
                  }}) {{
                    providerKind
                  }}
                }}
                "#,
                codex.provider_account_id
            )))
            .await;

        assert_eq!(response.errors.len(), 1);
        assert_eq!(
            response.errors[0].message,
            "model profile is not available for provider"
        );
        assert!(
            store
                .get_auxiliary_model_preference(crate::WEB_FETCH_SUMMARIZER_TASK_ID)
                .await
                .expect("preference read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_agent_model_preference_mutation_persists_valid_profile() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("foundation available");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary"
                    providerAccountId: "{}"
                    modelProfile: "default"
                  }}) {{
                    providerKind
                    providerAccountId
                    modelProfile
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let saved = store
            .get_agent_runtime_preference("agent:primary")
            .await
            .expect("preference read")
            .expect("preference saved");
        assert_eq!(saved.provider_kind, "foundation_local");
        assert_eq!(saved.model_profile, "default");
    }

    #[tokio::test]
    async fn save_agent_model_preference_requires_reasoning_for_reasoning_profile() {
        let (schema, account_id) = schema_with_reasoning_openai_profile().await;
        let response = schema
            .execute(format!(
                r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5"
              }}) {{
                modelProfile
              }}
            }}
            "#
            ))
            .await;
        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("reasoning"));
    }

    #[tokio::test]
    async fn save_agent_model_preference_persists_reasoning_effort() {
        let (schema, account_id) = schema_with_reasoning_openai_profile().await;
        let response = schema
            .execute(format!(
                r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: HIGH
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["saveAgentModelPreference"]["reasoningEffort"], "HIGH");
    }

    #[tokio::test]
    async fn start_primary_conversation_uses_saved_agent_provider_preference() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .update_agent_display_name("agent:primary", "Noema")
            .await
            .expect("name primary");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
                agent_id: "agent:primary".to_string(),
                provider_kind: "foundation_local".to_string(),
                provider_account_id: foundation.provider_account_id,
                model_profile: "default".to_string(),
                reasoning_effort: None,
            })
            .await
            .expect("preference");

        let codex_provider = Arc::new(AutofillTestProvider {
            text: "codex".to_string(),
            tool_classification_model: None,
            requests: Arc::new(Mutex::new(Vec::new())),
        });
        let foundation_provider = Arc::new(AutofillTestProvider {
            text: "foundation".to_string(),
            tool_classification_model: None,
            requests: Arc::new(Mutex::new(Vec::new())),
        });
        let runtime = crate::daemon::CodexRuntimeHandle::spawn_with_provider_map(
            "codex",
            vec![
                (
                    "codex".to_string(),
                    codex_provider as Arc<dyn crate::daemon::RuntimeModelProvider>,
                ),
                (
                    "foundation_local".to_string(),
                    foundation_provider as Arc<dyn crate::daemon::RuntimeModelProvider>,
                ),
            ],
            store.clone(),
        )
        .await
        .expect("runtime");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  ensurePrimaryConversation {
                    provider
                    conversationId
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["ensurePrimaryConversation"]["provider"],
            "foundation_local"
        );
    }

    #[tokio::test]
    async fn primary_conversation_returns_identity_without_transcript() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        let conversation = store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                "codex",
                Some("gpt-test".to_string()),
                None,
            )
            .await
            .expect("primary conversation");
        let runtime = test_autofill_runtime(store.clone(), "ok").await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store, runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                query {
                  primaryConversation {
                    provider
                    conversationId
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["primaryConversation"]["conversationId"],
            conversation.conversation_id
        );
        assert_eq!(data["primaryConversation"]["provider"], "codex");
        assert!(data["primaryConversation"].get("replay").is_none());
    }

    #[tokio::test]
    async fn primary_conversation_returns_latest_transcript_page() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                "codex",
                Some("gpt-test".to_string()),
                None,
            )
            .await
            .expect("primary conversation");
        let turn = store
            .create_conversation_turn(crate::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("turn");

        for label in ["one", "two", "three"] {
            store
                .append_conversation_item(crate::NewConversationItem {
                    conversation_id: conversation.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: None,
                    kind: crate::ConversationItemKind::UserText,
                    status: crate::ConversationItemStatus::Completed,
                    author: crate::ActorRef::human("human:local"),
                    content_text: Some(label.to_string()),
                    payload_json: serde_json::json!({}),
                    metadata: serde_json::json!({ "turn_index": 1 }),
                })
                .await
                .expect("item");
        }

        let runtime = test_autofill_runtime(store.clone(), "ok").await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store, runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                query {
                  primaryConversation {
                    provider
                    conversationId
                    latestTranscriptPage(limit: 2) {
                      items {
                        itemId
                        cursor
                        item { __typename ... on UserText { text } }
                      }
                      pageInfo { beforeCursor hasMoreBefore limit }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let primary = &data["primaryConversation"];
        assert_eq!(primary["conversationId"], conversation.conversation_id);
        assert_eq!(primary["provider"], "codex");
        let page = &primary["latestTranscriptPage"];
        assert_eq!(page["items"][0]["item"]["text"], "two");
        assert_eq!(page["items"][1]["item"]["text"], "three");
        assert_eq!(page["pageInfo"]["hasMoreBefore"], true);
        assert_eq!(page["pageInfo"]["limit"], 2);
        assert!(page["pageInfo"]["beforeCursor"].as_str().is_some());
    }

    #[tokio::test]
    async fn conversation_transcript_page_supports_latest_and_cursor_reads() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(crate::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(crate::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("turn");

        for label in ["one", "two", "three"] {
            store
                .append_conversation_item(crate::NewConversationItem {
                    conversation_id: conversation.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: None,
                    kind: crate::ConversationItemKind::UserText,
                    status: crate::ConversationItemStatus::Completed,
                    author: crate::ActorRef::human("human:local"),
                    content_text: Some(label.to_string()),
                    payload_json: serde_json::json!({}),
                    metadata: serde_json::json!({ "turn_index": 1 }),
                })
                .await
                .expect("item");
        }

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let latest = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}", limit: 2 }}) {{
                    items {{
                      itemId
                      cursor
                      item {{ __typename ... on UserText {{ text }} }}
                    }}
                    pageInfo {{ beforeCursor hasMoreBefore limit }}
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(latest.errors.is_empty(), "{:?}", latest.errors);
        let latest_data = latest.data.into_json().expect("latest json");
        let page = &latest_data["conversationTranscriptPage"];
        assert_eq!(page["items"][0]["item"]["text"], "two");
        assert_eq!(page["items"][1]["item"]["text"], "three");
        assert_eq!(page["pageInfo"]["hasMoreBefore"], true);
        let before_cursor = page["pageInfo"]["beforeCursor"].as_str().expect("cursor");

        let older = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}", cursor: "{}", limit: 2 }}) {{
                    items {{ item {{ __typename ... on UserText {{ text }} }} }}
                    pageInfo {{ hasMoreBefore }}
                  }}
                }}
                "#,
                conversation.conversation_id, before_cursor
            )))
            .await;

        assert!(older.errors.is_empty(), "{:?}", older.errors);
        let older_data = older.data.into_json().expect("older json");
        assert_eq!(
            older_data["conversationTranscriptPage"]["items"][0]["item"]["text"],
            "one"
        );
        assert_eq!(
            older_data["conversationTranscriptPage"]["pageInfo"]["hasMoreBefore"],
            false
        );
    }

    #[tokio::test]
    async fn conversation_transcript_page_returns_item_metadata() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(crate::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(crate::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("turn");
        store
            .append_conversation_item(crate::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: crate::ConversationItemKind::AssistantText,
                status: crate::ConversationItemStatus::Completed,
                author: crate::ActorRef::agent("agent:primary"),
                content_text: Some("hello".to_string()),
                payload_json: serde_json::json!({}),
                metadata: serde_json::json!({
                    "provider_usage": {
                        "provider": "codex",
                        "model": "gpt-test",
                        "phase": "initial",
                        "response_index": 0,
                        "input_tokens": 100,
                        "cached_input_tokens": 25,
                        "cache_hit_ratio": 0.25,
                        "output_tokens": 5,
                        "total_tokens": 105
                    }
                }),
            })
            .await
            .expect("item");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}", limit: 10 }}) {{
                    items {{
                      metadata
                      item {{ __typename ... on AssistantText {{ text }} }}
                    }}
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let item = &data["conversationTranscriptPage"]["items"][0];
        assert_eq!(item["item"]["text"], "hello");
        assert_eq!(
            item["metadata"]["provider_usage"]["cached_input_tokens"],
            25
        );
        assert_eq!(item["metadata"]["provider_usage"]["cache_hit_ratio"], 0.25);
    }

    #[tokio::test]
    async fn runtime_turn_passes_conversation_id_to_provider_request() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let (requests, runtime) =
            test_autofill_runtime_with_requests(store, "captured", None).await;
        let started = runtime
            .start_conversation(None)
            .await
            .expect("conversation");
        let (item_tx, _item_rx) = tokio::sync::mpsc::unbounded_channel::<TurnStreamEvent>();

        runtime
            .turn(
                started.conversation_id.clone(),
                "hello from durable chat".to_string(),
                item_tx,
            )
            .await
            .expect("turn");

        let requests = requests.lock().expect("requests");
        assert!(
            requests.iter().any(|request| {
                request.conversation_id.as_deref() == Some(started.conversation_id.as_str())
                    && matches!(
                        &request.input,
                        crate::provider::GenerateInput::Messages(messages)
                            if messages.len() == 1
                                && messages[0].role == crate::provider::GenerateMessageRole::User
                                && messages[0].content == "hello from durable chat"
                    )
            }),
            "captured requests: {requests:?}"
        );
    }

    #[tokio::test]
    async fn agents_query_sanitizes_unavailable_provider_errors() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                crate::ProviderAccountStatus::Unavailable,
                Some("bridge_missing"),
                Some("/Users/alice/.secret/token.txt failed with token abc123"),
            )
            .await
            .expect("status");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    modelOptions {
                      providerKind
                      disabledReason
                      profiles {
                        id
                        disabledReason
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let foundation_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("options")
            .iter()
            .find(|option| option["providerKind"] == "foundation_local")
            .expect("foundation option");
        let disabled_reason = foundation_option["disabledReason"]
            .as_str()
            .expect("disabled reason");
        assert_eq!(
            disabled_reason,
            "Apple Foundation Models bridge is unavailable."
        );
        assert!(
            !serde_json::to_string(foundation_option)
                .expect("json text")
                .contains("/Users/alice")
        );
        assert!(
            !serde_json::to_string(foundation_option)
                .expect("json text")
                .contains("abc123")
        );
    }

    #[tokio::test]
    async fn agents_query_disables_unknown_foundation_local_provider() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    modelOptions {
                      providerKind
                      disabledReason
                      profiles {
                        id
                        disabledReason
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let foundation_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("options")
            .iter()
            .find(|option| option["providerKind"] == "foundation_local")
            .expect("foundation option");
        assert_eq!(
            foundation_option["disabledReason"],
            "Apple Foundation Models availability has not been checked."
        );
        assert_eq!(
            foundation_option["profiles"][0]["disabledReason"],
            "Apple Foundation Models availability has not been checked."
        );
    }

    #[tokio::test]
    async fn save_agent_model_preference_rejects_unknown_foundation_local_provider() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary"
                    providerAccountId: "{}"
                    modelProfile: "default"
                  }}) {{
                    providerKind
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert!(
            response.errors.iter().any(|error| error.message
                == "Apple Foundation Models availability has not been checked."),
            "{:?}",
            response.errors
        );
    }

    #[tokio::test]
    async fn save_agent_model_preference_rejects_unavailable_provider() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                crate::ProviderAccountStatus::Unavailable,
                Some("unsupported_platform"),
                Some("/Users/alice/.secret/token.txt failed with token abc123"),
            )
            .await
            .expect("status");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary"
                    providerAccountId: "{}"
                    modelProfile: "default"
                  }}) {{
                    providerKind
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert_eq!(response.errors.len(), 1);
        let message = response.errors[0].message.as_str();
        assert_eq!(message, "Provider is unavailable on this platform.");
        assert!(!message.contains("/Users/alice"));
        assert!(!message.contains("abc123"));
        assert!(
            store
                .get_agent_runtime_preference("agent:primary")
                .await
                .expect("preference read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn mcp_settings_query_returns_servers_and_trusted_identity_selectors() {
        use crate::{
            McpTransportKind, NewMcpServer, NewMcpTool, NewTrustedIdentitySelector,
            TrustedIdentitySelectorEffect, TrustedIdentitySelectorKind, store::tests::test_store,
        };

        let store = test_store().await;
        store
            .create_mcp_server(NewMcpServer {
                mcp_server_id: "mcp_server:local-test".to_string(),
                display_name: "Local Test".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: json!({"command": "test-mcp"}),
            })
            .await
            .expect("create server");
        store
            .upsert_discovered_mcp_tool(NewMcpTool {
                mcp_tool_id: "mcp_tool:local-test:read".to_string(),
                mcp_server_id: "mcp_server:local-test".to_string(),
                name: "read".to_string(),
                description: Some("Read metadata".to_string()),
                input_schema: json!({"type": "object"}),
                output_schema: Some(json!({"type": "object"})),
                annotations: json!({"readOnlyHint": true}),
                metadata_fingerprint: "fingerprint:local-test:read:v1".to_string(),
            })
            .await
            .expect("upsert tool");
        store
            .create_mcp_server(NewMcpServer {
                mcp_server_id: "mcp:browser-oauth".to_string(),
                display_name: "Browser OAuth".to_string(),
                transport_kind: McpTransportKind::StreamableHttp,
                safe_config: json!({
                    "url": "https://example.com/mcp",
                    "headers": {},
                    "secret_refs": { "oauth_credentials": true }
                }),
            })
            .await
            .expect("create browser oauth server");
        store
            .create_trusted_identity_selector(NewTrustedIdentitySelector {
                selector_id: "trusted_identity:human-local:email".to_string(),
                owner_scope_id: "human:local".to_string(),
                selector_kind: TrustedIdentitySelectorKind::Email,
                raw_value: "Kevin@Noema.Example".to_string(),
                effect: TrustedIdentitySelectorEffect::Trust,
                issuer_actor_id: "human:local".to_string(),
            })
            .await
            .expect("create selector");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                query McpSettings {
                  mcpServers {
                    mcpServerId
                    displayName
                    transportKind
                    enabled
                    healthStatus
                    toolCount
                    browserOauthReauthenticationSupported
                  }
                  trustedIdentitySelectors(ownerScopeId: "human:local") {
                    selectorId
                    ownerScopeId
                    selectorKind
                    normalizedValue
                    effect
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let server = data["mcpServers"]
            .as_array()
            .expect("servers")
            .iter()
            .find(|server| server["mcpServerId"] == "mcp_server:local-test")
            .expect("local test server");
        assert_eq!(server["mcpServerId"], "mcp_server:local-test");
        assert_eq!(server["displayName"], "Local Test");
        assert_eq!(server["transportKind"], "stdio");
        assert_eq!(server["enabled"], false);
        assert_eq!(server["healthStatus"], "unknown");
        assert_eq!(server["toolCount"], 1);
        assert_eq!(server["browserOauthReauthenticationSupported"], false);

        let browser_oauth_server = data["mcpServers"]
            .as_array()
            .expect("servers")
            .iter()
            .find(|server| server["mcpServerId"] == "mcp:browser-oauth")
            .expect("browser oauth server");
        assert_eq!(
            browser_oauth_server["browserOauthReauthenticationSupported"],
            true
        );

        let selector = &data["trustedIdentitySelectors"][0];
        assert_eq!(selector["selectorId"], "trusted_identity:human-local:email");
        assert_eq!(selector["ownerScopeId"], "human:local");
        assert_eq!(selector["selectorKind"], "email");
        assert_eq!(selector["normalizedValue"], "kevin@noema.example");
        assert_eq!(selector["effect"], "trust");
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_returns_ready_for_calibration_without_secrets() {
        let fixture =
            GraphqlMcpSetupFixture::new(vec![TestMcpSetupOutcome::Ok(vec![discovered_mcp_tool(
                "list_repos",
                "List repositories",
                Some(json!({"type": "object"})),
                json!({"readOnlyHint": true}),
            )])])
            .await;
        let schema = build_schema(fixture.state);
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "GitHub"
                    transportKind: "stdio"
                    stdio: {
                      command: "npx"
                      args: ["-y", "server"]
                      env: { GITHUB_OWNER: "example" }
                      secretEnv: { GITHUB_TOKEN: "top-secret" }
                    }
                  }) {
                    setupStatus
                    discoveryStatus
                    discoveredToolCount
                    setupError
                    server {
                      mcpServerId
                      displayName
                      transportKind
                      enabled
                      healthStatus
                      authStatus
                      toolCount
                    }
                  }
                }
                "#,
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let result = &data["createMcpServer"];
        assert_eq!(result["setupStatus"], "ready_for_calibration");
        assert_eq!(result["discoveryStatus"], "discovered");
        assert_eq!(result["discoveredToolCount"], 1);
        assert_eq!(result["server"]["mcpServerId"], "mcp:github");
        assert_eq!(result["server"]["toolCount"], 1);
        assert!(
            !serde_json::to_string(&data)
                .expect("response json")
                .contains("top-secret")
        );
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_autodetects_browser_oauth_before_persisting() {
        let fixture =
            GraphqlMcpSetupFixture::new(vec![TestMcpSetupOutcome::Ok(vec![discovered_mcp_tool(
                "search",
                "Search contacts",
                Some(json!({"type": "object"})),
                json!({"readOnlyHint": true}),
            )])])
            .await;
        let store = fixture.store.clone();
        let schema = build_schema(fixture.state.with_mcp_browser_oauth_supported(true));
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Dex"
                    transportKind: "streamable_http"
                    http: { url: "https://mcp.getdex.com/mcp" }
                  }) {
                    setupStatus
                    discoveryStatus
                    discoveredToolCount
                    setupError
                    auth {
                      oauthAuthorizationSupported
                      oauthClientCredentialsSupported
                    }
                    server { mcpServerId }
                  }
                }
                "#,
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let result = &data["createMcpServer"];
        assert_eq!(result["setupStatus"], "needs_auth");
        assert_eq!(result["discoveryStatus"], "needs_auth");
        assert_eq!(result["discoveredToolCount"], 0);
        assert_eq!(result["auth"]["oauthAuthorizationSupported"], true);
        assert_eq!(result["auth"]["oauthClientCredentialsSupported"], true);
        assert_eq!(result["server"], serde_json::Value::Null);
        assert!(
            store
                .get_mcp_server("mcp:dex")
                .await
                .expect("get server")
                .is_none()
        );
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_rejects_secret_shaped_safe_keys() {
        let fixture = GraphqlMcpSetupFixture::new(Vec::new()).await;
        let schema = build_schema(fixture.state);
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Unsafe"
                    transportKind: "streamable_http"
                    http: {
                      url: "https://example.com/mcp"
                      headers: { Authorization: "Bearer unsafe" }
                    }
                  }) {
                    setupStatus
                  }
                }
                "#,
        )
        .await;

        assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
        assert!(response.errors[0].message.contains("Authorization"));
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_does_not_persist_until_auth_and_discovery_succeed() {
        let fixture = GraphqlMcpSetupFixture::new(vec![
            TestMcpSetupOutcome::AuthRequired("missing authorization".to_string()),
            TestMcpSetupOutcome::Ok(vec![discovered_mcp_tool(
                "retry_tool",
                "Retry tool",
                None,
                json!({}),
            )]),
        ])
        .await;
        let schema = build_schema(fixture.state);
        let store = fixture.store.clone();
        let create = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Remote"
                    transportKind: "streamable_http"
                    http: { url: "https://example.com/mcp" }
                  }) {
                    setupStatus
                    setupError
                    server { mcpServerId authStatus }
                  }
                }
                "#,
        )
        .await;
        assert!(create.errors.is_empty(), "{:?}", create.errors);
        let create_data = create.data.into_json().expect("create json");
        assert_eq!(create_data["createMcpServer"]["setupStatus"], "needs_auth");
        assert_eq!(
            create_data["createMcpServer"]["server"],
            serde_json::Value::Null
        );
        assert!(
            store
                .get_mcp_server("mcp:remote")
                .await
                .expect("get server")
                .is_none()
        );

        let retry = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Remote"
                    transportKind: "streamable_http"
                    http: {
                      url: "https://example.com/mcp"
                      secretHeaders: { Authorization: "Bearer retry" }
                    }
                  }) {
                    setupStatus
                    discoveredToolCount
                    server { mcpServerId authStatus toolCount }
                  }
                }
                "#,
        )
        .await;

        assert!(retry.errors.is_empty(), "{:?}", retry.errors);
        let retry_data = retry.data.into_json().expect("retry json");
        assert_eq!(
            retry_data["createMcpServer"]["setupStatus"],
            "ready_for_calibration"
        );
        assert_eq!(retry_data["createMcpServer"]["discoveredToolCount"], 1);
        assert_eq!(retry_data["createMcpServer"]["server"]["toolCount"], 1);
        assert!(
            !serde_json::to_string(&retry_data)
                .expect("retry json")
                .contains("Bearer retry")
        );
    }

    #[tokio::test]
    async fn mcp_tools_query_and_delete_mutation_use_persisted_setup_state() {
        let fixture =
            GraphqlMcpSetupFixture::new(vec![TestMcpSetupOutcome::Ok(vec![discovered_mcp_tool(
                "read_doc",
                "Read a document",
                None,
                json!({"readOnlyHint": true}),
            )])])
            .await;
        let schema = build_schema(fixture.state);

        let create = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Docs"
                    transportKind: "stdio"
                    stdio: {
                      command: "docs-mcp"
                      args: []
                      secretEnv: { DOCS_TOKEN: "secret" }
                    }
                  }) {
                    setupStatus
                    server { mcpServerId toolCount }
                  }
                }
                "#,
        )
        .await;
        assert!(create.errors.is_empty(), "{:?}", create.errors);
        assert!(
            fixture
                .paths
                .mcp_server_home("mcp:docs")
                .join("secrets.json")
                .exists()
        );

        let tools = execute_graphql(
            &schema,
            r#"
                query {
                  mcpTools(mcpServerId: "mcp:docs") {
                    mcpToolId
                    mcpServerId
                    name
                    description
                    inputSchema
                    annotations
                    metadataFingerprint
                    calibration { status }
                  }
                }
                "#,
        )
        .await;
        assert!(tools.errors.is_empty(), "{:?}", tools.errors);
        let tools_data = tools.data.into_json().expect("tools json");
        let tool = &tools_data["mcpTools"][0];
        assert_eq!(tool["mcpServerId"], "mcp:docs");
        assert_eq!(tool["name"], "read_doc");
        assert_eq!(tool["description"], "Read a document");
        assert_eq!(tool["inputSchema"], json!({"type": "object"}));
        assert_eq!(tool["calibration"], serde_json::Value::Null);

        let delete = execute_graphql(
            &schema,
            r#"
                mutation {
                  deleteMcpServer(mcpServerId: "mcp:docs")
                }
                "#,
        )
        .await;
        assert!(delete.errors.is_empty(), "{:?}", delete.errors);
        let delete_data = delete.data.into_json().expect("delete json");
        assert_eq!(delete_data["deleteMcpServer"], true);
        assert!(
            fixture
                .store
                .get_mcp_server("mcp:docs")
                .await
                .expect("server")
                .is_none()
        );
        assert!(
            fixture
                .store
                .list_mcp_tools_for_server("mcp:docs")
                .await
                .expect("tools")
                .is_empty()
        );
        assert!(!fixture.paths.mcp_server_home("mcp:docs").exists());
    }

    struct GraphqlMcpSetupFixture {
        state: GraphqlState,
        store: crate::NoemaStore,
        paths: crate::NoemaPaths,
        _home: TempDir,
    }

    impl GraphqlMcpSetupFixture {
        async fn new(outcomes: Vec<TestMcpSetupOutcome>) -> Self {
            let home = TempDir::new().expect("temp noema home");
            let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
            let config = crate::StoreConfig::from_paths(&paths);
            let store = crate::NoemaStore::open(&config).await.expect("open store");
            let state = GraphqlState::for_tests_with_store_paths_and_mcp_setup(
                store.clone(),
                paths.clone(),
                outcomes,
            );
            Self {
                state,
                store,
                paths,
                _home: home,
            }
        }
    }

    async fn execute_graphql(
        schema: &GraphqlSchema,
        query: &'static str,
    ) -> async_graphql::Response {
        schema.execute(async_graphql::Request::new(query)).await
    }

    fn discovered_mcp_tool(
        name: &str,
        description: &str,
        output_schema: Option<serde_json::Value>,
        annotations: serde_json::Value,
    ) -> crate::mcp::DiscoveredMcpTool {
        crate::mcp::DiscoveredMcpTool {
            name: name.to_string(),
            description: Some(description.to_string()),
            input_schema: json!({"type": "object"}),
            output_schema,
            annotations,
        }
    }

    async fn seed_google_mcp_tools(store: &crate::NoemaStore, tools: &[(&str, &str)]) {
        store
            .create_mcp_server(crate::NewMcpServer {
                mcp_server_id: "mcp_server:google".to_string(),
                display_name: "Google".to_string(),
                transport_kind: crate::McpTransportKind::Stdio,
                safe_config: json!({}),
            })
            .await
            .expect("create server");
        for (name, fingerprint) in tools {
            store
                .upsert_discovered_mcp_tool(crate::NewMcpTool {
                    mcp_tool_id: format!("mcp_tool:google:{name}"),
                    mcp_server_id: "mcp_server:google".to_string(),
                    name: (*name).to_string(),
                    description: Some(format!("Tool {name}")),
                    input_schema: json!({"type": "object"}),
                    output_schema: None,
                    annotations: json!({}),
                    metadata_fingerprint: (*fingerprint).to_string(),
                })
                .await
                .expect("upsert tool");
        }
    }

    #[tokio::test]
    async fn mcp_approval_requests_query_filters_by_status() {
        use crate::{NewMcpApprovalRequest, store::tests::test_store};

        let store = test_store().await;
        store
            .create_mcp_approval_request(NewMcpApprovalRequest {
                approval_id: "approval:mcp:pending".to_string(),
                action_summary: "Share Google Doc".to_string(),
                tool_invocation_id: "tool_invocation:mcp:pending".to_string(),
                mcp_server_id: Some("mcp_server:google".to_string()),
                mcp_tool_id: Some("mcp_tool:google:share_doc".to_string()),
                requester_actor_id: "agent:primary".to_string(),
                owner_scope_id: "human:local".to_string(),
                active_scope_id: "human:local".to_string(),
                destination_summary: "person@example.com".to_string(),
                data_source_summary: "Google Doc: Project plan".to_string(),
                source_owner_identity: "kevin@example.com".to_string(),
                source_owner_trust: "trusted".to_string(),
                destination_owner_identity: "person@example.com".to_string(),
                destination_owner_trust: "untrusted".to_string(),
                export_summary: "Document title and share permission".to_string(),
                payload_preview: json!({"recipient": "person@example.com"}),
            })
            .await
            .expect("create pending approval");
        store
            .create_mcp_approval_request(NewMcpApprovalRequest {
                approval_id: "approval:mcp:denied".to_string(),
                action_summary: "Publish note".to_string(),
                tool_invocation_id: "tool_invocation:mcp:denied".to_string(),
                mcp_server_id: Some("mcp_server:publish".to_string()),
                mcp_tool_id: Some("mcp_tool:publish:post".to_string()),
                requester_actor_id: "agent:primary".to_string(),
                owner_scope_id: "human:local".to_string(),
                active_scope_id: "human:local".to_string(),
                destination_summary: "example.com".to_string(),
                data_source_summary: "Draft note".to_string(),
                source_owner_identity: "kevin@example.com".to_string(),
                source_owner_trust: "trusted".to_string(),
                destination_owner_identity: "example.com".to_string(),
                destination_owner_trust: "untrusted".to_string(),
                export_summary: "Draft note content".to_string(),
                payload_preview: json!({"destination": "example.com"}),
            })
            .await
            .expect("create denied approval");
        store
            .with_connection(|conn| {
                conn.execute(
                    r#"
                    UPDATE approval_requests
                    SET status = 'denied',
                        decision_actor_id = 'human:local',
                        decision_comment = 'No',
                        decided_at = '2026-06-30T00:00:00Z',
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    WHERE approval_id = 'approval:mcp:denied'
                    "#,
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("deny approval");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                query ApprovalSettings {
                  mcpApprovalRequests(status: "pending") {
                    approvalId
                    actionSummary
                    mcpServerId
                    mcpToolId
                    requesterActorId
                    ownerScopeId
                    activeScopeId
                    destinationSummary
                    dataSourceSummary
                    sourceOwnerIdentity
                    sourceOwnerTrust
                    destinationOwnerIdentity
                    destinationOwnerTrust
                    exportSummary
                    payloadPreview
                    status
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let approvals = data["mcpApprovalRequests"].as_array().expect("approvals");
        assert_eq!(approvals.len(), 1);
        let approval = &approvals[0];
        assert_eq!(approval["approvalId"], "approval:mcp:pending");
        assert_eq!(approval["actionSummary"], "Share Google Doc");
        assert_eq!(approval["mcpServerId"], "mcp_server:google");
        assert_eq!(approval["mcpToolId"], "mcp_tool:google:share_doc");
        assert_eq!(approval["requesterActorId"], "agent:primary");
        assert_eq!(approval["ownerScopeId"], "human:local");
        assert_eq!(approval["activeScopeId"], "human:local");
        assert_eq!(approval["destinationSummary"], "person@example.com");
        assert_eq!(approval["dataSourceSummary"], "Google Doc: Project plan");
        assert_eq!(approval["sourceOwnerIdentity"], "kevin@example.com");
        assert_eq!(approval["sourceOwnerTrust"], "trusted");
        assert_eq!(approval["destinationOwnerIdentity"], "person@example.com");
        assert_eq!(approval["destinationOwnerTrust"], "untrusted");
        assert_eq!(
            approval["exportSummary"],
            "Document title and share permission"
        );
        assert_eq!(
            approval["payloadPreview"]["recipient"],
            "person@example.com"
        );
        assert_eq!(approval["status"], "pending");
    }

    #[tokio::test]
    async fn save_tool_calibration_mutation_persists_reviewed_policy() {
        use crate::{McpCalibrationStatus, store::tests::test_store};

        let store = test_store().await;
        seed_google_mcp_tools(&store, &[("read_doc", "fingerprint_1")]).await;

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  saveToolCalibration(input: {
                    calibrationId: "tool_calibration:read_doc"
                    mcpToolId: "mcp_tool:google:read_doc"
                    readClassification: "mixed"
                    writeClassification: "none"
                    exportClassification: "none"
                    ownerExtractors: []
                    status: "blocked_unresolved_ownership"
                    reviewedBy: "human:local"
                    reviewedMetadataFingerprint: "fingerprint_1"
                  }) {
                    calibrationId
                    mcpToolId
                    status
                    readClassification
                    writeClassification
                    exportClassification
                  }
                }
                "#,
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let calibration = &data["saveToolCalibration"];
        assert_eq!(calibration["calibrationId"], "tool_calibration:read_doc");
        assert_eq!(calibration["mcpToolId"], "mcp_tool:google:read_doc");
        assert_eq!(calibration["status"], "blocked_unresolved_ownership");
        assert_eq!(calibration["readClassification"], "mixed");
        assert_eq!(calibration["writeClassification"], "none");
        assert_eq!(calibration["exportClassification"], "none");

        let persisted = store
            .get_tool_calibration("mcp_tool:google:read_doc")
            .await
            .expect("get calibration")
            .expect("calibration exists");
        assert_eq!(
            persisted.status,
            McpCalibrationStatus::BlockedUnresolvedOwnership
        );
        assert_eq!(persisted.reviewed_by.as_deref(), Some("human:local"));
        assert_eq!(
            persisted.reviewed_metadata_fingerprint.as_deref(),
            Some("fingerprint_1")
        );
    }

    #[tokio::test]
    async fn save_tool_calibrations_mutation_persists_multiple_policies_in_one_request() {
        use crate::{McpCalibrationStatus, McpTrustClassification, store::tests::test_store};

        let store = test_store().await;
        seed_google_mcp_tools(
            &store,
            &[
                ("read_doc", "fingerprint_read"),
                ("share_doc", "fingerprint_share"),
            ],
        )
        .await;

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  saveToolCalibrations(inputs: [
                    {
                      calibrationId: "tool_calibration:read_doc"
                      mcpToolId: "mcp_tool:google:read_doc"
                      readClassification: "mixed"
                      writeClassification: "none"
                      exportClassification: "none"
                      ownerExtractors: []
                      status: "blocked_unresolved_ownership"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "fingerprint_read"
                    },
                    {
                      calibrationId: "tool_calibration:share_doc"
                      mcpToolId: "mcp_tool:google:share_doc"
                      readClassification: "none"
                      writeClassification: "trusted"
                      exportClassification: "untrusted"
                      ownerExtractors: []
                      status: "ready"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "fingerprint_share"
                    }
                  ]) {
                    mcpToolId
                    status
                    readClassification
                    writeClassification
                    exportClassification
                  }
                }
                "#,
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let calibrations = data["saveToolCalibrations"]
            .as_array()
            .expect("calibrations");
        assert_eq!(calibrations.len(), 2);
        assert_eq!(calibrations[0]["mcpToolId"], "mcp_tool:google:read_doc");
        assert_eq!(calibrations[1]["mcpToolId"], "mcp_tool:google:share_doc");

        let read_doc = store
            .get_tool_calibration("mcp_tool:google:read_doc")
            .await
            .expect("get read calibration")
            .expect("read calibration exists");
        let share_doc = store
            .get_tool_calibration("mcp_tool:google:share_doc")
            .await
            .expect("get share calibration")
            .expect("share calibration exists");
        assert_eq!(
            read_doc.status,
            McpCalibrationStatus::BlockedUnresolvedOwnership
        );
        assert_eq!(
            share_doc.write_classification,
            McpTrustClassification::Trusted
        );
        assert_eq!(
            share_doc.export_classification,
            McpTrustClassification::Untrusted
        );
    }

    #[tokio::test]
    async fn save_tool_calibrations_mutation_rejects_invalid_batch_without_partial_writes() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        seed_google_mcp_tools(
            &store,
            &[
                ("read_doc", "fingerprint_read"),
                ("share_doc", "fingerprint_share"),
            ],
        )
        .await;

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  saveToolCalibrations(inputs: [
                    {
                      calibrationId: "tool_calibration:read_doc"
                      mcpToolId: "mcp_tool:google:read_doc"
                      readClassification: "mixed"
                      writeClassification: "none"
                      exportClassification: "none"
                      ownerExtractors: []
                      status: "blocked_unresolved_ownership"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "fingerprint_read"
                    },
                    {
                      calibrationId: "tool_calibration:share_doc"
                      mcpToolId: "mcp_tool:google:share_doc"
                      readClassification: "none"
                      writeClassification: "trusted"
                      exportClassification: "untrusted"
                      ownerExtractors: []
                      status: "ready"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "wrong_fingerprint"
                    }
                  ]) {
                    mcpToolId
                  }
                }
                "#,
        )
        .await;

        assert!(!response.errors.is_empty());
        assert!(
            store
                .get_tool_calibration("mcp_tool:google:read_doc")
                .await
                .expect("get read calibration")
                .is_none()
        );
        assert!(
            store
                .get_tool_calibration("mcp_tool:google:share_doc")
                .await
                .expect("get share calibration")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_tool_calibration_mutation_rejects_invalid_enum_strings() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        seed_google_mcp_tools(&store, &[("read_doc", "fingerprint_1")]).await;

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  saveToolCalibration(input: {
                    calibrationId: "tool_calibration:read_doc"
                    mcpToolId: "mcp_tool:google:read_doc"
                    readClassification: "Mixed"
                    writeClassification: "none"
                    exportClassification: "none"
                    ownerExtractors: []
                    status: "blocked_unresolved_ownership"
                  }) {
                    calibrationId
                  }
                }
                "#,
        )
        .await;

        assert!(!response.errors.is_empty());
        assert!(
            response.errors[0]
                .message
                .contains("invalid readClassification"),
            "{:?}",
            response.errors
        );
        assert!(
            store
                .get_tool_calibration("mcp_tool:google:read_doc")
                .await
                .expect("get calibration")
                .is_none()
        );
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_returns_validated_suggestions_without_persisting() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        seed_autofill_server(&store).await;
        let runtime = test_autofill_runtime(
            store.clone(),
            r#"{"suggestions":[{"tool":"read_doc","read":"m","write":"n","export":"n","d":false}]}"#,
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  autofillToolCalibrations(mcpServerId: "mcp_server:docs") {
                    suggestions {
                      mcpToolId
                      readClassification
                      writeClassification
                      exportClassification
                      disabled
                      ownerExtractors { source selectorKind path }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let suggestion = &data["autofillToolCalibrations"]["suggestions"][0];
        assert_eq!(suggestion["mcpToolId"], "mcp_tool:docs:read_doc");
        assert_eq!(suggestion["readClassification"], "mixed");
        assert_eq!(suggestion["writeClassification"], "none");
        assert_eq!(suggestion["exportClassification"], "none");
        assert_eq!(suggestion["ownerExtractors"][0]["source"], "arguments");
        assert_eq!(suggestion["ownerExtractors"][0]["selectorKind"], "email");
        assert_eq!(suggestion["ownerExtractors"][0]["path"], "/owner_email");
        assert_eq!(suggestion["disabled"], false);
        assert!(
            store
                .get_tool_calibration("mcp_tool:docs:read_doc")
                .await
                .expect("get calibration")
                .is_none()
        );
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_rejects_invalid_model_output_without_persisting() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        seed_autofill_server(&store).await;
        let runtime = test_autofill_runtime(
            store.clone(),
            r#"{"suggestions":[{"tool":"missing_doc","read":"m","write":"n","export":"n","d":false}]}"#,
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  autofillToolCalibrations(mcpServerId: "mcp_server:docs") {
                    suggestions { mcpToolId }
                  }
                }
                "#,
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(
            response.errors[0].message.contains("unknown MCP tool name"),
            "{:?}",
            response.errors
        );
        assert!(
            store
                .get_tool_calibration("mcp_tool:docs:read_doc")
                .await
                .expect("get calibration")
                .is_none()
        );
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_returns_null_when_disabled_is_omitted() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        seed_autofill_server(&store).await;
        let runtime = test_autofill_runtime(
            store.clone(),
            r#"{"suggestions":[{"tool":"read_doc","read":"m","write":"n","export":"n"}]}"#,
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  autofillToolCalibrations(mcpServerId: "mcp_server:docs") {
                    suggestions {
                      mcpToolId
                      disabled
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let suggestion = &data["autofillToolCalibrations"]["suggestions"][0];
        assert_eq!(suggestion["mcpToolId"], "mcp_tool:docs:read_doc");
        assert_eq!(suggestion["disabled"], serde_json::Value::Null);
        assert!(
            store
                .get_tool_calibration("mcp_tool:docs:read_doc")
                .await
                .expect("get calibration")
                .is_none()
        );
    }

    async fn seed_autofill_server(store: &crate::NoemaStore) {
        use crate::{McpTransportKind, NewMcpServer, NewMcpTool};

        store
            .create_mcp_server(NewMcpServer {
                mcp_server_id: "mcp_server:docs".to_string(),
                display_name: "Docs".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: json!({}),
            })
            .await
            .expect("create server");
        store
            .upsert_discovered_mcp_tool(NewMcpTool {
                mcp_tool_id: "mcp_tool:docs:read_doc".to_string(),
                mcp_server_id: "mcp_server:docs".to_string(),
                name: "read_doc".to_string(),
                description: Some("Read a document by id".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "owner_email": { "type": "string" }
                    }
                }),
                output_schema: None,
                annotations: json!({"readOnlyHint": true}),
                metadata_fingerprint: "fingerprint_1".to_string(),
            })
            .await
            .expect("upsert tool");
    }

    #[derive(Debug)]
    struct AutofillTestProvider {
        text: String,
        tool_classification_model: Option<String>,
        requests: Arc<Mutex<Vec<crate::provider::GenerateRequest>>>,
    }

    impl crate::provider::ModelProvider for AutofillTestProvider {
        async fn generate(
            &self,
            request: crate::provider::GenerateRequest,
        ) -> Result<crate::provider::GenerateResponse, crate::provider::ProviderError> {
            self.requests.lock().expect("requests").push(request);
            Ok(crate::provider::GenerateResponse {
                responses: vec![crate::provider::GenerateResponseItem::Text {
                    phase: None,
                    text: self.text.clone(),
                }],
                tool_calls: Vec::new(),
                reasoning_items: Vec::new(),
                response_status: crate::provider::GenerateResponseStatus::Final,
                provider: "test".to_string(),
                model: "test-autofill".to_string(),
                response_id: None,
                usage: None,
            })
        }

        fn default_tool_classification_model(&self) -> Option<String> {
            self.tool_classification_model.clone()
        }
    }

    async fn test_autofill_runtime(
        store: crate::NoemaStore,
        text: &str,
    ) -> crate::daemon::CodexRuntimeHandle {
        let (_runtime, runtime) =
            test_autofill_runtime_with_requests(store, text, Some("test-tool-classifier")).await;
        runtime
    }

    async fn test_autofill_runtime_with_requests(
        store: crate::NoemaStore,
        text: &str,
        tool_classification_model: Option<&str>,
    ) -> (
        Arc<Mutex<Vec<crate::provider::GenerateRequest>>>,
        crate::daemon::CodexRuntimeHandle,
    ) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let provider = Arc::new(AutofillTestProvider {
            text: text.to_string(),
            tool_classification_model: tool_classification_model.map(str::to_string),
            requests: requests.clone(),
        });
        let runtime = crate::daemon::CodexRuntimeHandle::spawn_with_provider(provider, store)
            .await
            .expect("runtime");
        (requests, runtime)
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_uses_runtime_tool_classification_model() {
        use crate::store::tests::test_store;

        let store = test_store().await;
        seed_autofill_server(&store).await;
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            r#"{"suggestions":[{"tool":"read_doc","read":"m","write":"n","export":"n","d":false}]}"#,
            Some("gpt-test-tool-classifier"),
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  autofillToolCalibrations(mcpServerId: "mcp_server:docs") {
                    suggestions { mcpToolId }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].model.as_deref(),
            Some("gpt-test-tool-classifier")
        );
    }

    #[tokio::test]
    async fn conversation_events_emits_ready_before_live_events() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on SubscriptionReadyEvent {
                  conversationId
                }
                ... on TurnCompletedEvent {
                  conversationId
                  clientMessageId
                }
              }
            }
            "#,
        ));

        let response = stream.next().await.expect("ready response");
        let data = response.data.into_json().expect("ready json");
        assert_eq!(
            data.pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str),
            Some("SubscriptionReadyEvent")
        );
        assert_eq!(
            data.pointer("/conversationEvents/conversationId")
                .and_then(serde_json::Value::as_str),
            Some("conversation_1")
        );

        subscriptions.publish(ConversationLiveEvent::Completed {
            conversation_id: "conversation_1".to_string(),
            client_message_id: Some("client_1".to_string()),
        });
        let response = stream.next().await.expect("completion response");
        let data = response.data.into_json().expect("completion json");
        assert_eq!(
            data.pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str),
            Some("TurnCompletedEvent")
        );
        assert_eq!(
            data.pointer("/conversationEvents/clientMessageId")
                .and_then(serde_json::Value::as_str),
            Some("client_1")
        );
    }

    #[tokio::test]
    async fn subscription_streams_assistant_text_delta_event() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on AssistantTextDeltaEvent {
                  conversationId
                  turnId
                  streamId
                  delta
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "SubscriptionReadyEvent"
        );

        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::AssistantTextDelta {
                conversation_id: "conversation_1".to_string(),
                turn_id: "turn_1".to_string(),
                stream_id: "assistant_stream:turn_1:initial".to_string(),
                response_index: 0,
                delta: "Hel".to_string(),
            }),
        });

        let response = stream.next().await.expect("delta response");
        let data = response.data.into_json().expect("delta json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "AssistantTextDeltaEvent");
        assert_eq!(event["conversationId"], "conversation_1");
        assert_eq!(event["turnId"], "turn_1");
        assert_eq!(event["streamId"], "assistant_stream:turn_1:initial");
        assert_eq!(event["delta"], "Hel");
    }

    #[tokio::test]
    async fn subscription_streams_conversation_item_metadata() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on ConversationItemEvent {
                  conversationId
                  itemId
                  cursor
                  metadata
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "SubscriptionReadyEvent"
        );

        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: "conversation_1".to_string(),
                item_id: "transient:activity_1".to_string(),
                cursor: None,
                turn_id: Some("turn_1".to_string()),
                metadata: json!({
                    "runtime_item_id": "activity_1",
                    "transient": true,
                }),
                item: Box::new(crate::TurnTranscriptItem::AssistantText {
                    text: "Hello".to_string(),
                }),
            }),
        });

        let response = stream.next().await.expect("item response");
        let data = response.data.into_json().expect("item json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "ConversationItemEvent");
        assert_eq!(event["conversationId"], "conversation_1");
        assert_eq!(event["itemId"], "transient:activity_1");
        assert!(event["cursor"].is_null());
        assert_eq!(
            event["metadata"],
            json!({
                "runtime_item_id": "activity_1",
                "transient": true,
            })
        );
    }

    #[tokio::test]
    async fn subscription_streams_conversation_item_cursor_when_present() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on ConversationItemEvent {
                  itemId
                  cursor
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "SubscriptionReadyEvent"
        );

        subscriptions.publish(ConversationLiveEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: "conversation_1".to_string(),
                item_id: "item_1".to_string(),
                cursor: Some("conversation_item:1".to_string()),
                turn_id: Some("turn_1".to_string()),
                metadata: serde_json::json!({}),
                item: Box::new(crate::TurnTranscriptItem::UserText {
                    text: "Hello".to_string(),
                }),
            }),
        });

        let response = stream.next().await.expect("item response");
        let data = response.data.into_json().expect("item json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "ConversationItemEvent");
        assert_eq!(event["itemId"], "item_1");
        assert_eq!(event["cursor"], "conversation_item:1");
    }
}
