/// Root GraphQL mutation object.
use super::*;

macro_rules! task_mutation {
    ($ctx:expr, $input:expr, $resolver:path $(, $extra:expr)?) => {{
        let state = $ctx.data_unchecked::<GraphqlState>();
        $resolver(state, crate::graphql::request_principal_subject($ctx)?, $input $(, $extra)?).await
    }};
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn configure_apns_provider(
        &self,
        ctx: &Context<'_>,
        input: GraphqlConfigureApnsProviderInput,
    ) -> Result<GraphqlApnsProviderStatus> {
        browser_notifications(ctx)?
            .configure_apns_provider(input)
            .await
    }

    async fn remove_apns_provider(
        &self,
        ctx: &Context<'_>,
        expected_revision: i64,
    ) -> Result<GraphqlApnsProviderStatus> {
        browser_notifications(ctx)?
            .remove_apns_provider(expected_revision)
            .await
    }

    async fn register_client_notifications(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRegisterClientNotificationsInput,
    ) -> Result<GraphqlClientNotificationStatus> {
        let (notifications, client_id) = paired_notifications(ctx)?;
        notifications
            .register_client_notifications(&client_id, input)
            .await
    }

    async fn disable_client_notifications(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GraphqlClientNotificationStatus> {
        let (notifications, client_id) = paired_notifications(ctx)?;
        notifications.disable_client_notifications(&client_id).await
    }

    async fn register_client_live_activities(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRegisterClientLiveActivitiesInput,
    ) -> Result<GraphqlClientLiveActivityStatus> {
        let (notifications, client_id) = paired_notifications(ctx)?;
        notifications
            .register_client_live_activities(&client_id, input)
            .await
    }

    async fn register_client_live_activity_update(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRegisterClientLiveActivityUpdateInput,
    ) -> Result<bool> {
        let (notifications, client_id) = paired_notifications(ctx)?;
        notifications
            .register_client_live_activity_update(&client_id, input)
            .await
    }

    async fn dismiss_client_live_activity(
        &self,
        ctx: &Context<'_>,
        activity_id: String,
    ) -> Result<bool> {
        let (notifications, client_id) = paired_notifications(ctx)?;
        notifications
            .dismiss_client_live_activity(&client_id, &activity_id)
            .await
    }

    async fn disable_client_live_activities(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GraphqlClientLiveActivityStatus> {
        let (notifications, client_id) = paired_notifications(ctx)?;
        notifications
            .disable_client_live_activities(&client_id)
            .await
    }

    /// Register or refresh this browser's notification subscription.
    async fn register_web_push_subscription(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRegisterWebPushSubscriptionInput,
    ) -> Result<GraphqlWebPushStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        let browser_session_hash = crate::graphql::browser_session_hash(ctx)?;
        state
            .notifications()
            .ok_or_else(|| async_graphql::Error::new("Web Push requires an HTTPS public origin"))?
            .register(principal, browser_session_hash, input)
            .await
    }

    /// Remove one caller-owned browser notification subscription.
    async fn remove_web_push_subscription(
        &self,
        ctx: &Context<'_>,
        subscription_id: String,
    ) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        let browser_session_hash = crate::graphql::browser_session_hash(ctx)?;
        state
            .notifications()
            .ok_or_else(|| async_graphql::Error::new("Web Push requires an HTTPS public origin"))?
            .remove(principal, browser_session_hash, &subscription_id)
            .await
    }

    /// Revoke one paired client without deleting its durable audit row.
    async fn revoke_client(&self, ctx: &Context<'_>, client_id: String) -> Result<GraphqlClient> {
        let principal = crate::graphql::request_principal(ctx)?;
        clients::revoke_client(ctx.data_unchecked::<GraphqlState>(), &principal, client_id).await
    }

    /// Revoke every native client owned by the authenticated local human.
    async fn revoke_all_clients(&self, ctx: &Context<'_>) -> Result<i32> {
        let principal = crate::graphql::request_principal(ctx)?;
        clients::revoke_all_clients(ctx.data_unchecked::<GraphqlState>(), &principal).await
    }

    /// Atomically confirm every first-run model assignment.
    async fn confirm_onboarding_model_selections(
        &self,
        ctx: &Context<'_>,
        input: GraphqlConfirmOnboardingModelSelectionsInput,
    ) -> Result<GraphqlOnboardingStatus> {
        onboarding::confirm_onboarding_model_selections(ctx.data_unchecked::<GraphqlState>(), input)
            .await
    }

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

    /// Save a human-visible label without rotating connection authority.
    async fn save_capability_connection_label(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveCapabilityConnectionLabelInput,
    ) -> Result<GraphqlCapabilityConnection> {
        capability_integrations::save_connection_label(ctx.data_unchecked::<GraphqlState>(), input)
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

    /// Resolve one chat-driven MCP setup after its exact connection is configured.
    async fn resolve_mcp_setup_intervention(
        &self,
        ctx: &Context<'_>,
        input: GraphqlResolveMcpSetupInterventionInput,
    ) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        human_interventions::resolve_mcp_setup_intervention(state, principal, input).await
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

    /// Create a project through the task command service.
    async fn create_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCreateProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::create_project(state, principal, input).await
    }

    /// Update a project through the task command service.
    async fn update_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlUpdateProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::update_project(state, principal, input).await
    }

    /// Archive a project through the task command service.
    async fn archive_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlArchiveProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::archive_project(state, principal, input).await
    }

    /// Reopen a project through the task command service.
    async fn reopen_project(
        &self,
        ctx: &Context<'_>,
        input: GraphqlReopenProjectInput,
    ) -> Result<GraphqlProjectCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::reopen_project(state, principal, input).await
    }

    /// Capture a task in Inbox through the task command service.
    async fn capture_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCaptureTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::capture_task(state, principal, input).await
    }

    /// Edit Inbox capture fields through the task command service.
    async fn update_inbox_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlUpdateInboxTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::update_inbox_task(state, principal, input).await
    }

    /// Queue an Inbox task through the task command service.
    async fn queue_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlQueueTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::queue_task(state, principal, input).await
    }

    /// Schedule an Inbox task for future execution.
    async fn schedule_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlScheduleTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(ctx, input, tasks::set_task_schedule, false)
    }

    /// Replace an Inbox task's future execution configuration.
    async fn reschedule_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlScheduleTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(ctx, input, tasks::set_task_schedule, true)
    }

    /// Remove future execution from an Inbox task.
    async fn unschedule_task(
        &self,
        ctx: &Context<'_>,
        input: GraphqlUnscheduleTaskInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(ctx, input, tasks::unschedule_task)
    }

    /// Start an already scheduled Inbox task immediately.
    async fn run_scheduled_task_now(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRunScheduledTaskNowInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(ctx, input, tasks::run_scheduled_task_now)
    }

    /// Edit the future authority for a recurring task.
    async fn update_task_recurrence(
        &self,
        ctx: &Context<'_>,
        input: GraphqlUpdateTaskRecurrenceInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(ctx, input, tasks::update_task_recurrence)
    }

    /// Pause a recurring task.
    async fn pause_task_recurrence(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskRecurrenceCommandInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(
            ctx,
            input,
            tasks::change_task_recurrence,
            RecurrenceCommandKind::Pause
        )
    }

    /// Resume a recurring task.
    async fn resume_task_recurrence(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskRecurrenceCommandInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(
            ctx,
            input,
            tasks::change_task_recurrence,
            RecurrenceCommandKind::Resume
        )
    }

    /// Skip the next exact recurring slot.
    async fn skip_task_recurrence_next(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskRecurrenceCommandInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(
            ctx,
            input,
            tasks::change_task_recurrence,
            RecurrenceCommandKind::SkipNext
        )
    }

    /// End future recurrence without changing active occurrences.
    async fn end_task_recurrence(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskRecurrenceCommandInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(
            ctx,
            input,
            tasks::change_task_recurrence,
            RecurrenceCommandKind::End
        )
    }

    /// Create one extra immediate occurrence without advancing the schedule.
    async fn run_task_recurrence_now(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskRecurrenceCommandInput,
    ) -> Result<GraphqlTaskCommandPayload> {
        task_mutation!(ctx, input, tasks::run_task_recurrence_now)
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

    /// Replace user-controlled task safety limits while retaining task-owned bounds.
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

    /// Cancel a pending provider auth attempt.
    async fn cancel_provider_auth_attempt(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCancelProviderAuthAttemptInput,
    ) -> Result<Option<GraphqlProviderAuthAttempt>> {
        onboarding::cancel_provider_auth_attempt(ctx.data_unchecked::<GraphqlState>(), input).await
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

    /// Change the service tier for future requests from one provider account.
    async fn set_provider_fast_mode(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSetProviderFastModeInput,
    ) -> Result<GraphqlProviderAccount> {
        let state = ctx.data_unchecked::<GraphqlState>();
        provider_accounts::set_provider_fast_mode(state, input).await
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

    /// Register a trusted operator-managed ACP executable.
    async fn create_acp_agent(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCreateAcpAgentInput,
    ) -> Result<GraphqlAcpAgent> {
        let principal = crate::graphql::request_principal_subject(ctx)?;
        agents::create_acp_agent(ctx.data_unchecked::<GraphqlState>(), principal, input).await
    }

    /// Replace one ACP launch configuration or disable it.
    async fn update_acp_agent(
        &self,
        ctx: &Context<'_>,
        input: GraphqlUpdateAcpAgentInput,
    ) -> Result<GraphqlAcpAgent> {
        let principal = crate::graphql::request_principal_subject(ctx)?;
        agents::update_acp_agent(ctx.data_unchecked::<GraphqlState>(), principal, input).await
    }

    /// Hard-delete one configured ACP executor.
    async fn delete_acp_agent(
        &self,
        ctx: &Context<'_>,
        input: GraphqlDeleteAcpAgentInput,
    ) -> Result<bool> {
        let principal = crate::graphql::request_principal_subject(ctx)?;
        agents::delete_acp_agent(ctx.data_unchecked::<GraphqlState>(), principal, input).await
    }

    /// Initialize one ACP executable and persist its safe implementation metadata.
    async fn test_acp_agent(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTestAcpAgentInput,
    ) -> Result<GraphqlAcpAgent> {
        let principal = crate::graphql::request_principal_subject(ctx)?;
        agents::test_acp_agent(ctx.data_unchecked::<GraphqlState>(), principal, input).await
    }

    /// Run one advertised agent-managed ACP authentication method.
    async fn authenticate_acp_agent(
        &self,
        ctx: &Context<'_>,
        input: GraphqlAuthenticateAcpAgentInput,
    ) -> Result<GraphqlAcpAgent> {
        let principal = crate::graphql::request_principal_subject(ctx)?;
        agents::authenticate_acp_agent(ctx.data_unchecked::<GraphqlState>(), principal, input).await
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

    /// Submit one exact action to a pending A2UI surface.
    #[graphql(name = "sendA2UIAction")]
    async fn send_a2ui_action(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSendA2UIActionInput,
    ) -> Result<GraphqlTurnAccepted> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        chat::send_a2ui_action(state, principal, input).await
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

    /// Cancel one exact current pending adapter definition as the local human.
    async fn cancel_adapter_definition(
        &self,
        ctx: &Context<'_>,
        input: GraphqlCancelAdapterDefinitionInput,
    ) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::cancel_adapter_definition(state, principal, input).await
    }

    /// Normalize transient credential input and create one adapter connection.
    async fn setup_adapter_connection(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSetupAdapterConnectionInput,
    ) -> Result<GraphqlAdapterDefinition> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::setup_adapter_connection(state, principal, input).await
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

    /// Import one reusable OAuth client document.
    async fn import_adapter_oauth_application(
        &self,
        ctx: &Context<'_>,
        input: GraphqlImportAdapterOauthApplicationInput,
    ) -> Result<GraphqlAdapterOauthApplication> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::import_adapter_oauth_application(state, principal, input).await
    }

    /// Replace one exact OAuth application document.
    async fn replace_adapter_oauth_application(
        &self,
        ctx: &Context<'_>,
        input: GraphqlReplaceAdapterOauthApplicationInput,
    ) -> Result<GraphqlAdapterOauthApplication> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::replace_adapter_oauth_application(state, principal, input).await
    }

    /// Attach one API definition to one account grant.
    async fn attach_adapter_oauth_connection(
        &self,
        ctx: &Context<'_>,
        input: GraphqlAttachAdapterOauthConnectionInput,
    ) -> Result<GraphqlAdapterDefinition> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::attach_adapter_oauth_connection(state, principal, input).await
    }

    /// Disconnect one reusable account grant.
    async fn disconnect_adapter_oauth_grant(
        &self,
        ctx: &Context<'_>,
        input: GraphqlDisconnectAdapterOauthGrantInput,
    ) -> Result<GraphqlAdapterAuthorizationGrant> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::disconnect_adapter_oauth_grant(state, principal, input).await
    }

    /// Save a label for an account without stable provider identity.
    async fn save_adapter_oauth_grant_label(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveAdapterOauthGrantLabelInput,
    ) -> Result<GraphqlAdapterAuthorizationGrant> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::save_adapter_oauth_grant_label(state, principal, input).await
    }

    /// Delete one OAuth application without dependent grants.
    async fn delete_adapter_oauth_application(
        &self,
        ctx: &Context<'_>,
        input: GraphqlDeleteAdapterOauthApplicationInput,
    ) -> Result<bool> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::delete_adapter_oauth_application(state, principal, input).await
    }

    /// Suspend or resume one API connection.
    async fn set_adapter_connection_active(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSetAdapterConnectionActiveInput,
    ) -> Result<GraphqlAdapterDefinition> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        adapters::set_adapter_connection_active(state, principal, input).await
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

fn notifications<'a>(ctx: &'a Context<'_>) -> Result<&'a crate::graphql::NotificationCoordinator> {
    ctx.data_unchecked::<GraphqlState>()
        .notifications()
        .ok_or_else(|| async_graphql::Error::new("notifications are unavailable"))
}

fn browser_notifications<'a>(
    ctx: &'a Context<'_>,
) -> Result<&'a crate::graphql::NotificationCoordinator> {
    if crate::graphql::request_principal(ctx)?
        .client_id()
        .is_some()
    {
        return Err(async_graphql::Error::new(
            "browser session authentication required",
        ));
    }
    notifications(ctx)
}

fn paired_notifications<'a>(
    ctx: &'a Context<'_>,
) -> Result<(&'a crate::graphql::NotificationCoordinator, String)> {
    let client_id = crate::graphql::request_principal(ctx)?
        .client_id()
        .ok_or_else(|| async_graphql::Error::new("paired client authentication required"))?
        .to_string();
    Ok((notifications(ctx)?, client_id))
}
