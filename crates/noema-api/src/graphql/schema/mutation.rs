/// Root GraphQL mutation object.
use super::*;

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Add a fresh connection to one explicitly selected MCP definition revision.
    async fn add_mcp_connection(
        &self,
        ctx: &Context<'_>,
        input: GraphqlAddMcpConnectionInput,
    ) -> Result<GraphqlMcpServerSetupResult> {
        mcp::add_mcp_connection(ctx.data_unchecked::<GraphqlState>(), input).await
    }

    /// Save both sharing and unsafe-call choices for one exact connection.
    async fn save_capability_connection_policy(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveCapabilityConnectionPolicyInput,
    ) -> Result<GraphqlCapabilityConnection> {
        capability_integrations::save_connection_policy(ctx.data_unchecked::<GraphqlState>(), input)
            .await
    }

    /// Save all four human behavior hints for one exact tool revision.
    async fn save_capability_tool_override(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveCapabilityToolOverrideInput,
    ) -> Result<GraphqlCapabilityManagedTool> {
        capability_integrations::save_tool_override(ctx.data_unchecked::<GraphqlState>(), input)
            .await
    }

    /// Reset one exact tool to its current source/default behavior.
    async fn reset_capability_tool_policy(
        &self,
        ctx: &Context<'_>,
        input: GraphqlResetCapabilityToolPolicyInput,
    ) -> Result<GraphqlCapabilityManagedTool> {
        capability_integrations::reset_tool_policy(ctx.data_unchecked::<GraphqlState>(), input)
            .await
    }

    /// Enable or disable one exact current tool revision.
    async fn set_capability_tool_enabled(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSetCapabilityToolEnabledInput,
    ) -> Result<GraphqlCapabilityManagedTool> {
        capability_integrations::set_tool_enabled(ctx.data_unchecked::<GraphqlState>(), input).await
    }

    /// Approve or decline one immutable governed-action revision.
    async fn resolve_governed_action(
        &self,
        ctx: &Context<'_>,
        input: GraphqlResolveGovernedActionInput,
    ) -> Result<GraphqlGovernedAction> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        governed_actions::resolve_governed_action(state, principal, input).await
    }

    /// Start browser OAuth for one exact MCP authentication interruption.
    async fn start_mcp_authentication(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartMcpAuthenticationInput,
    ) -> Result<GraphqlMcpOAuthSetupAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        human_interventions::start_mcp_authentication(state, principal, input).await
    }

    /// Skip one exact MCP call and continue its interrupted origin.
    async fn skip_mcp_authentication(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSkipMcpAuthenticationInput,
    ) -> Result<GraphqlMcpAuthenticationIntervention> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        human_interventions::skip_mcp_authentication(state, principal, input).await
    }

    /// Start browser OAuth for one exact API adapter authentication interruption.
    async fn start_adapter_authentication(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartAdapterAuthenticationInput,
    ) -> Result<GraphqlAdapterOauthSetupAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        human_interventions::start_adapter_authentication(state, principal, input).await
    }

    /// Skip one exact API adapter call and continue its interrupted origin.
    async fn skip_adapter_authentication(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSkipAdapterAuthenticationInput,
    ) -> Result<GraphqlAdapterAuthenticationIntervention> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        human_interventions::skip_adapter_authentication(state, principal, input).await
    }

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

    /// Reopen a completed task into Queue with new direction.
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

    /// Save the model/provider preference used for governed action review.
    async fn save_action_reviewer_preference(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveActionReviewerPreferenceInput,
    ) -> Result<GraphqlAgentModelPreference> {
        let state = ctx.data_unchecked::<GraphqlState>();
        privacy_settings::save_action_reviewer_preference(state, input).await
    }

    /// Queue one native Markdown memory update for the local primary conversation.
    async fn update_memory(&self, ctx: &Context<'_>) -> Result<GraphqlNativeMemoryUpdateResult> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        native_memory::update_memory(state, principal).await
    }

    /// Save the model preference used by native Markdown memory updates.
    async fn save_memory_model_preference(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveMemoryModelPreferenceInput,
    ) -> Result<GraphqlAgentModelPreference> {
        native_memory::save_memory_model_preference(ctx.data_unchecked::<GraphqlState>(), input)
            .await
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

    /// Approve one exact pending adapter definition as the local human.
    async fn approve_adapter_definition(
        &self,
        ctx: &Context<'_>,
        input: GraphqlApproveAdapterDefinitionInput,
    ) -> Result<GraphqlAdapterDefinition> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::approve_adapter_definition(state, principal, input).await
    }

    /// Import one human-selected OAuth client JSON document without retaining
    /// the raw upload.
    async fn import_adapter_oauth_client_json(
        &self,
        ctx: &Context<'_>,
        input: GraphqlImportAdapterOauthClientJsonInput,
    ) -> Result<GraphqlAdapterDefinition> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::import_adapter_oauth_client_json(state, principal, input).await
    }

    /// Delete one exact native-adapter connection revision.
    async fn delete_adapter_connection(
        &self,
        ctx: &Context<'_>,
        input: GraphqlDeleteAdapterConnectionInput,
    ) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::delete_adapter_connection(state, principal, input).await
    }

    /// Delete one adapter service after all of its connections are removed.
    async fn delete_adapter_service(
        &self,
        ctx: &Context<'_>,
        input: GraphqlDeleteAdapterServiceInput,
    ) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::delete_adapter_service(state, principal, input).await
    }

    /// Start provider-neutral browser OAuth for one exact native-adapter
    /// connection. The serving shell owns the callback URI and mode.
    async fn start_adapter_oauth_setup(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartAdapterOauthSetupInput,
    ) -> Result<GraphqlAdapterOauthSetupAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::start_adapter_oauth_setup(state, principal, input).await
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
        let principal = crate::graphql::request_principal_subject(ctx)?;
        mcp::start_mcp_server_oauth_setup(state, principal, input).await
    }

    /// Start browser OAuth reauthentication for an existing hosted MCP server.
    async fn start_mcp_server_reauthentication_oauth_setup(
        &self,
        ctx: &Context<'_>,
        input: GraphqlStartMcpServerReauthenticationOAuthSetupInput,
    ) -> Result<GraphqlMcpOAuthSetupAttempt> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        mcp::start_mcp_server_reauthentication_oauth_setup(state, principal, input).await
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
