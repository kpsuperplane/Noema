//! Unified human-attention projection for permission and MCP authentication.

use async_graphql::{Enum, InputObject, Result, SimpleObject, Union};
use noema_capabilities_mcp::{
    McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus, StartMcpOAuthReauthenticationCommand,
};
use noema_store::{CapabilityAuthenticationRequestRecord, CapabilityAuthenticationRequestState};

use super::{
    adapters::{GraphqlAdapterDefinition, adapter_definitions},
    governed_actions::{GraphqlGovernedAction, pending_governed_actions},
    mcp::GraphqlMcpOAuthSetupAttempt,
    runtime_state::GraphqlState,
};

/// Durable MCP sign-in interruption state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "McpAuthenticationRequestState")]
pub enum GraphqlMcpAuthenticationRequestState {
    AwaitingUser,
    Authorizing,
    Resuming,
    Completed,
    Cancelled,
    Superseded,
}

/// Safe projection of an MCP sign-in request; exact arguments remain private.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpAuthenticationIntervention")]
pub struct GraphqlMcpAuthenticationIntervention {
    pub request_id: String,
    pub revision: u64,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub mcp_server_id: String,
    pub server_display_name: String,
    pub capability_name: String,
    pub state: GraphqlMcpAuthenticationRequestState,
    pub failure_code: Option<String>,
}

/// Human intervention variants share presentation, but retain separate authorities.
#[derive(Clone, Debug, Union)]
#[graphql(name = "HumanIntervention")]
pub enum GraphqlHumanIntervention {
    GovernedAction(GraphqlGovernedAction),
    McpAuthentication(GraphqlMcpAuthenticationIntervention),
    AdapterDefinition(GraphqlAdapterDefinition),
}

/// Start browser sign-in for one exact request revision.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "StartMcpAuthenticationInput")]
pub struct GraphqlStartMcpAuthenticationInput {
    pub request_id: String,
    pub expected_revision: u64,
    pub redirect_uri: String,
}

/// Skip one exact MCP call without granting credentials.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SkipMcpAuthenticationInput")]
pub struct GraphqlSkipMcpAuthenticationInput {
    pub request_id: String,
    pub expected_revision: u64,
}

pub(super) async fn pending_human_interventions(
    state: &GraphqlState,
    principal: &str,
    conversation_id: Option<String>,
    task_id: Option<String>,
    first: Option<i32>,
) -> Result<Vec<GraphqlHumanIntervention>> {
    let first = usize::try_from(first.unwrap_or(50).clamp(1, 100)).unwrap_or(50);
    let actions = pending_governed_actions(
        state,
        principal,
        conversation_id.clone(),
        task_id.clone(),
        Some(i32::try_from(first).unwrap_or(50)),
    )
    .await?;
    let authentications = state
        .store()?
        .list_pending_capability_authentication_requests(
            principal,
            conversation_id.as_deref(),
            task_id.as_deref(),
            first,
        )
        .await?;
    let adapter_reviews = if conversation_id.is_some() && task_id.is_none() {
        adapter_definitions(state)
            .await?
            .into_iter()
            .filter(|definition| {
                (!definition.reviewed && !definition.superseded)
                    || (definition.reviewed
                        && definition.connection_count == 0
                        && definition.accepts_oauth_client_json)
            })
            .map(GraphqlHumanIntervention::AdapterDefinition)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(actions
        .into_iter()
        .map(GraphqlHumanIntervention::GovernedAction)
        .chain(
            authentications
                .into_iter()
                .filter_map(GraphqlMcpAuthenticationIntervention::from_mcp)
                .map(GraphqlHumanIntervention::McpAuthentication),
        )
        .chain(adapter_reviews)
        .take(first)
        .collect())
}

pub(super) async fn start_mcp_authentication(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlStartMcpAuthenticationInput,
) -> Result<GraphqlMcpOAuthSetupAttempt> {
    super::mcp::require_exact_oauth_callback(state, &input.redirect_uri)?;
    let _start = state.mcp_oauth_start_lock().lock().await;
    let request =
        owned_request(state, principal, &input.request_id, input.expected_revision).await?;
    let mcp_server_id = request
        .mcp_server_id()
        .ok_or_else(|| async_graphql::Error::new("MCP authentication request is unavailable"))?
        .to_string();
    let operations = state.mcp_operations()?;
    if let Some(attempt_id) = state
        .store()?
        .active_mcp_authentication_attempt(principal, &mcp_server_id)
        .await?
    {
        if let Some(attempt) = operations
            .oauth_setup_attempt(McpOAuthSetupAttemptQuery {
                attempt_id: attempt_id.clone(),
                owner_human_id: Some(principal.to_string()),
            })
            .await
            .map_err(super::errors::graphql_error)?
            && attempt.status != McpOAuthSetupAttemptStatus::Failed
        {
            state
                .store()?
                .begin_capability_authentication(
                    &request.request_id,
                    request.revision,
                    principal,
                    &attempt_id,
                )
                .await?;
            if attempt.status == McpOAuthSetupAttemptStatus::Completed {
                state
                    .runtime()?
                    .resume_mcp_authentication_attempt(attempt_id)
                    .await?;
            }
            return Ok(attempt.into());
        }
        state
            .store()?
            .reset_capability_authentication_attempt(&attempt_id, "oauth_attempt_missing")
            .await?;
    }
    let attempt = operations
        .start_oauth_reauthentication(StartMcpOAuthReauthenticationCommand {
            owner_human_id: principal.to_string(),
            mcp_server_id,
            redirect_uri: input.redirect_uri,
        })
        .await
        .map_err(super::errors::graphql_error)?;
    state
        .store()?
        .begin_capability_authentication(
            &request.request_id,
            request.revision,
            principal,
            &attempt.attempt_id,
        )
        .await?;
    Ok(attempt.into())
}

pub(super) async fn skip_mcp_authentication(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlSkipMcpAuthenticationInput,
) -> Result<GraphqlMcpAuthenticationIntervention> {
    owned_request(state, principal, &input.request_id, input.expected_revision).await?;
    let request = state
        .runtime()?
        .skip_mcp_authentication_request(
            input.request_id,
            input.expected_revision,
            principal.to_string(),
        )
        .await?;
    GraphqlMcpAuthenticationIntervention::from_mcp(request)
        .ok_or_else(|| async_graphql::Error::new("MCP authentication request is unavailable"))
}

async fn owned_request(
    state: &GraphqlState,
    principal: &str,
    request_id: &str,
    revision: u64,
) -> Result<CapabilityAuthenticationRequestRecord> {
    state
        .store()?
        .get_capability_authentication_request(request_id, revision)
        .await?
        .filter(|request| request.owner_human_id == principal && request.mcp_server_id().is_some())
        .ok_or_else(|| async_graphql::Error::new("MCP authentication request is unavailable"))
}

impl GraphqlMcpAuthenticationIntervention {
    fn from_mcp(request: CapabilityAuthenticationRequestRecord) -> Option<Self> {
        let mcp_server_id = request.mcp_server_id()?.to_string();
        Some(Self {
            request_id: request.request_id,
            revision: request.revision,
            conversation_id: request.conversation_id,
            task_id: request.task_id,
            run_id: request.run_id,
            mcp_server_id,
            server_display_name: request.authority_display_name,
            capability_name: request.capability_name,
            state: request.state.into(),
            failure_code: request.failure_code,
        })
    }
}

graphql_enum_from!(CapabilityAuthenticationRequestState => GraphqlMcpAuthenticationRequestState {
    AwaitingUser => AwaitingUser,
    Authorizing => Authorizing,
    Resuming => Resuming,
    Completed => Completed,
    Cancelled => Cancelled,
    Superseded => Superseded,
});
