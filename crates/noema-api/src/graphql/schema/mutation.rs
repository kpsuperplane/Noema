/// Root GraphQL mutation object.
use super::*;

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Install one curated local model using the selected machine build.
    async fn install_local_model(
        &self,
        ctx: &Context<'_>,
        input: GraphqlInstallLocalModelInput,
    ) -> Result<GraphqlLocalModelInstallation> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_models::install_local_model(state, input).await
    }

    /// Import a public or local GGUF into Noema's content-addressed store.
    async fn import_local_model(
        &self,
        ctx: &Context<'_>,
        input: GraphqlImportLocalModelInput,
    ) -> Result<GraphqlLocalModelInstallation> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_models::import_local_model(state, input).await
    }

    /// Cancel one queued or active local-model transfer.
    async fn cancel_local_model_install(
        &self,
        ctx: &Context<'_>,
        installation_id: String,
    ) -> Result<GraphqlLocalModelInstallation> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_models::cancel_local_model_install(state, installation_id).await
    }

    /// Remove one local-model installation and unreferenced model bytes.
    async fn remove_local_model(&self, ctx: &Context<'_>, installation_id: String) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_models::remove_local_model(state, installation_id).await
    }

    /// Make one installed local model active.
    async fn activate_local_model(
        &self,
        ctx: &Context<'_>,
        installation_id: String,
    ) -> Result<GraphqlLocalModelInstallation> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_models::activate_local_model(state, installation_id).await
    }

    /// Save Noema's system model default.
    async fn save_default_model_preference(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveDefaultModelPreferenceInput,
    ) -> Result<GraphqlDefaultModelPreference> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_models::save_default_model_preference(state, input).await
    }

    /// Retry the supervised local llama.cpp runtime.
    async fn retry_local_model_runtime(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GraphqlLocalModelRuntimeStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_models::retry_local_model_runtime(state).await
    }

    /// Create a project through the semantic Work command service.
    async fn create_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCreateProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::create_project(state, principal, input).await
    }

    /// Update a project through the semantic Work command service.
    async fn update_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlUpdateProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::update_project(state, principal, input).await
    }

    /// Archive a project through the semantic Work command service.
    async fn archive_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlArchiveProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::archive_project(state, principal, input).await
    }

    /// Reopen a project through the semantic Work command service.
    async fn reopen_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlReopenProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::reopen_project(state, principal, input).await
    }

    /// Capture a task in Inbox through the semantic Work command service.
    async fn capture_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCaptureTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::capture_task(state, principal, input).await
    }

    /// Edit Inbox capture fields through the semantic Work command service.
    async fn update_inbox_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlUpdateInboxTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::update_inbox_task(state, principal, input).await
    }

    /// Queue an Inbox task through the semantic Work command service.
    async fn queue_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlQueueTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::queue_task(state, principal, input).await
    }

    /// Resolve a clarification, approval, or recovery gate.
    async fn answer_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlAnswerTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::answer_task(state, principal, input).await
    }

    /// Retry an eligible recovery gate.
    async fn retry_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRetryTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::retry_task(state, principal, input).await
    }

    /// Accept a reviewer-approved task result.
    async fn accept_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlAcceptTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::accept_task(state, principal, input).await
    }

    /// Request changes to an approved result.
    async fn request_task_changes(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRequestTaskChangesInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::request_task_changes(state, principal, input).await
    }

    /// Cancel a nonterminal task.
    async fn cancel_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCancelTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::cancel_task(state, principal, input).await
    }

    /// Reopen terminal task history into Inbox.
    async fn reopen_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlReopenTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::reopen_task(state, principal, input).await
    }

    /// Replace one human-controlled executor model-pool entry.
    async fn update_task_model_pool_entry(
        &self,
        ctx: &Context<'_>,
        pool_entry_id: String,
        input: GraphqlTaskModelPoolEntryInput,
    ) -> Result<GraphqlTaskModelPoolEntry> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::update_task_model_pool_entry(state, principal, pool_entry_id, input).await
    }

    /// Replace user-controlled task safety limits while retaining Work-owned bounds.
    async fn update_task_execution_policy(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskExecutionPolicyInput,
    ) -> Result<GraphqlTaskExecutionPolicy> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::update_task_execution_policy(state, principal, input).await
    }

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
        let principal = crate::graphql::request_principal_subject(ctx)?;
        chat::ensure_primary_conversation(state, principal, cwd).await
    }

    /// Send a conversation turn.
    async fn send_conversation_turn(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSendConversationTurnInput,
    ) -> Result<GraphqlTurnAccepted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        chat::send_conversation_turn(state, principal, input).await
    }

    /// Send a multiple-choice selection as a user turn.
    async fn send_multiple_choice_selection(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSendMultipleChoiceSelectionInput,
    ) -> Result<GraphqlTurnAccepted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        chat::send_multiple_choice_selection(state, principal, input).await
    }

    /// Create a conversation-owned external artifact.
    async fn create_conversation_external_artifact(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCreateConversationExternalArtifactInput,
    ) -> Result<artifacts::GraphqlArtifact> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        artifacts::create_conversation_external_artifact(state, principal, input).await
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
