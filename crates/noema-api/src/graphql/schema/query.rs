/// Root GraphQL query object.
use super::*;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Return browser-managed APNs provider status without private key material.
    async fn apns_provider_status(&self, ctx: &Context<'_>) -> Result<GraphqlApnsProviderStatus> {
        ctx.data_unchecked::<GraphqlState>()
            .notifications()
            .ok_or_else(|| async_graphql::Error::new("notifications are unavailable"))?
            .apns_provider_status()
            .await
    }

    /// Return the paired client's native notification registration state.
    async fn client_notification_status(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GraphqlClientNotificationStatus> {
        let principal = crate::graphql::request_principal(ctx)?;
        let client_id = principal
            .client_id()
            .ok_or_else(|| async_graphql::Error::new("paired client authentication required"))?;
        ctx.data_unchecked::<GraphqlState>()
            .notifications()
            .ok_or_else(|| async_graphql::Error::new("notifications are unavailable"))?
            .client_notification_status(client_id)
            .await
    }

    async fn client_live_activity_status(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GraphqlClientLiveActivityStatus> {
        let principal = crate::graphql::request_principal(ctx)?;
        let client_id = principal
            .client_id()
            .ok_or_else(|| async_graphql::Error::new("paired client authentication required"))?;
        ctx.data_unchecked::<GraphqlState>()
            .notifications()
            .ok_or_else(|| async_graphql::Error::new("notifications are unavailable"))?
            .client_live_activity_status(client_id)
            .await
    }

    /// Return installed-web notification capability and this browser's registration.
    async fn web_push_status(
        &self,
        ctx: &Context<'_>,
        endpoint: Option<String>,
    ) -> Result<GraphqlWebPushStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        let browser_session_hash = crate::graphql::browser_session_hash(ctx)?;
        match state.notifications() {
            Some(web_push) => {
                web_push
                    .status(principal, browser_session_hash, endpoint.as_deref())
                    .await
            }
            None => Ok(GraphqlWebPushStatus::unavailable()),
        }
    }

    /// List all paired clients, including retained revoked rows.
    async fn clients(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlClient>> {
        let principal = crate::graphql::request_principal(ctx)?;
        clients::clients(ctx.data_unchecked::<GraphqlState>(), &principal).await
    }

    /// List API or MCP definitions with their concrete connections.
    async fn capability_integrations(
        &self,
        ctx: &Context<'_>,
        kind: GraphqlCapabilityIntegrationKind,
    ) -> Result<Vec<GraphqlCapabilityIntegration>> {
        capability_integrations::integrations(ctx.data_unchecked::<GraphqlState>(), kind).await
    }

    /// Return one concrete source-owned connection by structured identity.
    async fn capability_connection(
        &self,
        ctx: &Context<'_>,
        r#ref: GraphqlCapabilityConnectionRefInput,
    ) -> Result<Option<GraphqlCapabilityConnection>> {
        capability_integrations::connection(ctx.data_unchecked::<GraphqlState>(), r#ref).await
    }

    /// Return tools for one concrete source-owned connection.
    async fn capability_tools(
        &self,
        ctx: &Context<'_>,
        r#ref: GraphqlCapabilityConnectionRefInput,
    ) -> Result<Vec<GraphqlCapabilityManagedTool>> {
        capability_integrations::tools(ctx.data_unchecked::<GraphqlState>(), r#ref).await
    }

    #[cfg(any(test, feature = "test-support"))]
    async fn test_request_principal(&self, ctx: &Context<'_>) -> String {
        ctx.data_unchecked::<crate::graphql::RequestPrincipal>()
            .subject_id()
            .to_string()
    }

    /// Return local Noema status.
    async fn local_status(&self, ctx: &Context<'_>) -> Result<GraphqlLocalStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        local_status::local_status(state).await
    }

    /// Return the first-run local-model recommendation and readiness state.
    async fn local_model_setup(&self, _ctx: &Context<'_>) -> Result<GraphqlLocalModelSetup> {
        let state = _ctx.data_unchecked::<GraphqlState>();
        local_models::local_model_setup(state).await
    }

    /// List curated models and machine-selected builds.
    async fn local_model_catalog(
        &self,
        _ctx: &Context<'_>,
    ) -> Result<Vec<GraphqlLocalModelCatalogEntry>> {
        let state = _ctx.data_unchecked::<GraphqlState>();
        local_models::local_model_catalog(state).await
    }

    /// List durable local-model installations and transfer state.
    async fn local_model_installations(
        &self,
        _ctx: &Context<'_>,
    ) -> Result<Vec<GraphqlLocalModelInstallation>> {
        let state = _ctx.data_unchecked::<GraphqlState>();
        local_models::local_model_installations(state).await
    }

    /// Return Noema's system model default.
    async fn default_model_preference(
        &self,
        _ctx: &Context<'_>,
    ) -> Result<Option<GraphqlDefaultModelPreference>> {
        let state = _ctx.data_unchecked::<GraphqlState>();
        local_models::default_model_preference(state).await
    }

    /// Return onboarding status.
    async fn onboarding_status(&self, ctx: &Context<'_>) -> Result<GraphqlOnboardingStatus> {
        let state = ctx.data_unchecked::<GraphqlState>();
        onboarding::onboarding_status(state).await
    }

    /// Return selectable models and role-aware proposals for one ready account.
    async fn onboarding_model_setup(
        &self,
        ctx: &Context<'_>,
        provider_account_id: String,
    ) -> Result<GraphqlOnboardingModelSetup> {
        onboarding::onboarding_model_setup(
            ctx.data_unchecked::<GraphqlState>(),
            provider_account_id,
        )
        .await
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

    /// List configured ACP task executors.
    async fn acp_agents(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlAcpAgent>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        agents::acp_agents(state, principal).await
    }

    /// Return one owner-authorized task detail.
    async fn task(&self, ctx: &Context<'_>, task_id: String) -> Result<GraphqlTaskDetail> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::task(state, principal, task_id).await
    }

    /// Preview resolved future schedule instants.
    async fn task_schedule_preview(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskSchedulePreviewInput,
    ) -> Result<GraphqlTaskSchedulePreview> {
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::task_schedule_preview(principal, input).await
    }

    /// Return recurring authority with newest occurrence history.
    async fn task_recurrence(
        &self,
        ctx: &Context<'_>,
        recurrence_id: String,
        first: Option<i32>,
    ) -> Result<GraphqlTaskRecurrence> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::task_recurrence(state, principal, recurrence_id, first).await
    }

    /// List owner-authorized projects in stable update order.
    async fn projects(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        include_archived: Option<bool>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlProjectConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::projects(
            state,
            principal,
            workspace_id,
            include_archived.unwrap_or(false),
            first,
            after,
        )
        .await
    }

    /// Return one coherent Tasks overview.
    async fn tasks_overview(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        project_id: Option<String>,
    ) -> Result<GraphqlTaskOverview> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::tasks_overview(state, principal, workspace_id, project_id).await
    }

    /// List tasks through one bounded store query.
    async fn tasks(
        &self,
        ctx: &Context<'_>,
        input: GraphqlTaskListInput,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::task_list(state, principal, input, first, after).await
    }

    /// Return unresolved gate/review attention cards.
    async fn needs_you(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        project_id: Option<String>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskAttentionConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::needs_you(state, principal, workspace_id, project_id, first, after).await
    }

    /// List unresolved external actions requiring the current human's decision.
    async fn pending_governed_actions(
        &self,
        ctx: &Context<'_>,
        conversation_id: Option<String>,
        task_id: Option<String>,
        first: Option<i32>,
    ) -> Result<Vec<GraphqlGovernedAction>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        governed_actions::pending_governed_actions(
            state,
            principal,
            conversation_id,
            task_id,
            first,
        )
        .await
    }

    /// List unresolved task, permission, setup, and sign-in interventions for the current human.
    async fn pending_human_interventions(
        &self,
        ctx: &Context<'_>,
        conversation_id: Option<String>,
        task_id: Option<String>,
        project_id: Option<String>,
        first: Option<i32>,
    ) -> Result<Vec<GraphqlHumanIntervention>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        human_interventions::pending_human_interventions(
            state,
            principal,
            conversation_id,
            task_id,
            project_id,
            first,
        )
        .await
    }

    /// Return saved Tasks activity for a scope.
    async fn tasks_activity(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        project_id: Option<String>,
        task_id: Option<String>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskEventConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::tasks_activity(
            state,
            principal,
            workspace_id,
            project_id,
            task_id,
            first,
            after,
        )
        .await
    }

    /// Return Done and Cancelled task history through the terminal scope.
    #[allow(
        clippy::too_many_arguments,
        reason = "GraphQL preserves the flat taskHistory field contract"
    )]
    async fn task_history(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        project_id: Option<String>,
        kind: Option<GraphqlTerminalTaskKind>,
        text: Option<String>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::task_history(
            state,
            principal,
            workspace_id,
            project_id,
            kind.unwrap_or(GraphqlTerminalTaskKind::All),
            text,
            first,
            after,
        )
        .await
    }

    /// Return a newest-page, owner-authorized task-run transcript connection.
    async fn task_run_items(
        &self,
        ctx: &Context<'_>,
        run_id: String,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskRunItemConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::task_run_items(state, principal, run_id, after, first).await
    }

    /// Return a live or durable runtime profile for one authorized turn or run.
    async fn runtime_debug_profile(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRuntimeDebugProfileInput,
    ) -> Result<Option<GraphqlRuntimeDebugProfile>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        runtime_debug::runtime_debug_profile(state, principal, input).await
    }

    /// Return human-controlled executor model-pool entries.
    async fn task_model_pools(
        &self,
        ctx: &Context<'_>,
        complexity: Option<GraphqlTaskComplexity>,
    ) -> Result<Vec<GraphqlTaskModelPoolEntry>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        tasks::task_model_pools(state, complexity).await
    }

    /// Return provider-independent safety limits shared by every task tier.
    async fn task_execution_policy(&self, ctx: &Context<'_>) -> Result<GraphqlTaskExecutionPolicy> {
        let state = ctx.data_unchecked::<GraphqlState>();
        tasks::task_execution_policy(state).await
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

    /// Return Safety privacy settings safe to show in Settings.
    async fn privacy_settings(&self, ctx: &Context<'_>) -> Result<GraphqlPrivacySettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        privacy_settings::privacy_settings(state).await
    }

    /// List MCP server metadata safe to show in Settings.
    async fn mcp_servers(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlMcpServer>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        mcp::mcp_servers(state).await
    }

    /// List filesystem-canonical adapter definitions safe to review in Settings.
    async fn adapter_definitions(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<GraphqlAdapterDefinition>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        adapters::adapter_definitions(state).await
    }

    /// Return the reusable OAuth application and account hierarchy.
    async fn adapter_oauth_state(&self, ctx: &Context<'_>) -> Result<GraphqlAdapterOauthState> {
        let state = ctx.data_unchecked::<GraphqlState>();
        adapters::adapter_oauth_state(state).await
    }

    /// Return the latest process-local state for one OAuth attempt.
    async fn adapter_oauth_attempt(
        &self,
        ctx: &Context<'_>,
        attempt_id: String,
    ) -> Result<Option<GraphqlAdapterOauthAttemptEvent>> {
        if crate::graphql::request_principal_subject(ctx)? != "human:local" {
            return Err(async_graphql::Error::new(
                "adapter OAuth attempt is unauthorized",
            ));
        }
        adapters::adapter_oauth_attempt(ctx.data_unchecked::<GraphqlState>(), &attempt_id)
    }

    /// Return a short-lived MCP OAuth setup attempt.
    async fn mcp_oauth_setup_attempt(
        &self,
        ctx: &Context<'_>,
        attempt_id: String,
    ) -> Result<Option<GraphqlMcpOAuthSetupAttempt>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        mcp::mcp_oauth_setup_attempt(state, principal, attempt_id).await
    }

    /// Return the native memory model preference and selectable options.
    async fn memory_settings(&self, ctx: &Context<'_>) -> Result<GraphqlNativeMemorySettings> {
        let state = ctx.data_unchecked::<GraphqlState>();
        native_memory::memory_settings(state).await
    }

    /// Return the canonical native Markdown memory tree.
    async fn memory_tree(&self, ctx: &Context<'_>) -> Result<GraphqlNativeMemoryTree> {
        native_memory::memory_tree(ctx.data_unchecked::<GraphqlState>()).await
    }

    /// Read one native Markdown memory page.
    async fn memory_page(
        &self,
        ctx: &Context<'_>,
        page_id: String,
    ) -> Result<Option<GraphqlNativeMemoryPage>> {
        native_memory::memory_page(ctx.data_unchecked::<GraphqlState>(), page_id).await
    }

    /// Search native Markdown memory and return snippets/page references.
    async fn search_memory(
        &self,
        ctx: &Context<'_>,
        query: String,
        limit: Option<i32>,
    ) -> Result<Vec<GraphqlNativeMemorySearchResult>> {
        native_memory::search_memory(ctx.data_unchecked::<GraphqlState>(), query, limit).await
    }

    /// Return the primary conversation identity without creating it or replaying transcript.
    async fn primary_conversation(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GraphqlPrimaryConversation>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        chat::primary_conversation(state, principal).await
    }

    /// Return a cursor-based page of visible conversation transcript items.
    async fn conversation_transcript_page(
        &self,
        ctx: &Context<'_>,
        input: GraphqlConversationTranscriptPageInput,
    ) -> Result<GraphqlConversationTranscriptPage> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        chat::conversation_transcript_page(state, principal, input).await
    }

    /// List artifacts for one concrete owner.
    async fn artifacts(
        &self,
        ctx: &Context<'_>,
        owner_object_type: String,
        owner_object_id: String,
        limit: Option<i32>,
    ) -> Result<Vec<artifacts::GraphqlArtifact>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        artifacts::artifacts(state, principal, owner_object_type, owner_object_id, limit).await
    }

    /// Load one artifact by id.
    async fn artifact(
        &self,
        ctx: &Context<'_>,
        artifact_id: String,
    ) -> Result<Option<artifacts::GraphqlArtifact>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        artifacts::artifact(state, principal, artifact_id).await
    }

    /// Load one artifact version detail payload for the chat detail rail.
    async fn artifact_version_detail(
        &self,
        ctx: &Context<'_>,
        artifact_version_id: String,
    ) -> Result<Option<artifacts::GraphqlArtifactVersionDetail>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        artifacts::artifact_version_detail(state, principal, artifact_version_id).await
    }
}
