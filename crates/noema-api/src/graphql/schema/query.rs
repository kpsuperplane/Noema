/// Root GraphQL query object.
use super::*;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
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

    /// Return one owner-authorized Work task detail.
    async fn task(&self, ctx: &Context<'_>, task_id: String) -> Result<GraphqlTaskDetail> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::task(state, principal, task_id).await
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

    /// Return one coherent board bootstrap projection.
    async fn work_overview(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        project_id: Option<String>,
    ) -> Result<GraphqlWorkOverview> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::work_overview(state, principal, workspace_id, project_id).await
    }

    /// List board/list tasks through one bounded batch-hydrated Store query.
    async fn work_tasks(
        &self,
        ctx: &Context<'_>,
        input: GraphqlWorkTasksInput,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlTaskConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::work_tasks(state, principal, input, first, after).await
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

    /// Return the durable Work activity ledger for a scope.
    async fn work_activity(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        project_id: Option<String>,
        task_id: Option<String>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<GraphqlWorkEventConnection> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        tasks::work_activity(
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

    /// Return completed/cancelled task history through the terminal scope.
    #[allow(
        clippy::too_many_arguments,
        reason = "GraphQL preserves the existing flat completedTasks field contract"
    )]
    async fn completed_tasks(
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
        tasks::completed_tasks(
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
