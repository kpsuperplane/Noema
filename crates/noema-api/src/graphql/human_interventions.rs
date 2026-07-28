//! Unified human-attention projection for permission and capability authentication.

use async_graphql::{Enum, InputObject, Result, SimpleObject, Union};
use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallengeKind,
};
use noema_capabilities_mcp::{
    McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus, StartMcpOAuthReauthenticationCommand,
};
use noema_capability_adapters::AdapterConnectionRevisions;
use noema_store::{CapabilityAuthenticationRequestRecord, CapabilityAuthenticationRequestState};
use std::collections::BTreeMap;

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

/// Safe projection of an API adapter sign-in request; exact arguments remain private.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AdapterAuthenticationIntervention")]
pub struct GraphqlAdapterAuthenticationIntervention {
    pub request_id: String,
    pub revision: u64,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub adapter_connection_id: String,
    pub service_display_name: String,
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
    AdapterAuthentication(GraphqlAdapterAuthenticationIntervention),
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

/// Start browser sign-in for one exact API adapter interruption.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "StartAdapterAuthenticationInput")]
pub struct GraphqlStartAdapterAuthenticationInput {
    pub request_id: String,
    pub expected_revision: u64,
}

/// Skip one exact API adapter call without replacing its credential.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SkipAdapterAuthenticationInput")]
pub struct GraphqlSkipAdapterAuthenticationInput {
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
    let adapter_service_names = adapter_service_names(state, &authentications);
    let adapter_reviews = if conversation_id.is_some() && task_id.is_none() {
        adapter_definitions(state)
            .await?
            .into_iter()
            .filter(|definition| {
                (!definition.reviewed && !definition.superseded)
                    || (definition.reviewed
                        && ((definition.connection_count == 0
                            && definition.accepts_oauth_client_json)
                            || definition.connections.iter().any(|connection| {
                                connection.status == "authentication_required"
                                    || (connection.status == "active"
                                        && !connection.policy_configured)
                            })))
            })
            .map(GraphqlHumanIntervention::AdapterDefinition)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(actions
        .into_iter()
        .map(GraphqlHumanIntervention::GovernedAction)
        .chain(authentications.into_iter().filter_map(|request| {
            match request.challenge.authority_kind() {
                CapabilityAuthenticationAuthorityKind::McpServer => {
                    GraphqlMcpAuthenticationIntervention::from_mcp(request)
                        .map(GraphqlHumanIntervention::McpAuthentication)
                }
                CapabilityAuthenticationAuthorityKind::AdapterConnection => {
                    let display_name = adapter_service_names
                        .get(request.challenge.authority_id())
                        .cloned();
                    GraphqlAdapterAuthenticationIntervention::from_adapter(request, display_name)
                        .map(GraphqlHumanIntervention::AdapterAuthentication)
                }
            }
        }))
        .chain(adapter_reviews)
        .take(first)
        .collect())
}

fn adapter_service_names(
    state: &GraphqlState,
    requests: &[CapabilityAuthenticationRequestRecord],
) -> BTreeMap<String, String> {
    if !requests
        .iter()
        .any(|request| request.adapter_connection_id().is_some())
    {
        return BTreeMap::new();
    }
    let Ok(operations) = state.adapter_operations() else {
        return BTreeMap::new();
    };
    let Ok(snapshot) = operations.management_snapshot() else {
        return BTreeMap::new();
    };
    let definitions = snapshot
        .definitions
        .into_iter()
        .map(|definition| {
            let name = definition
                .compiled
                .display_name
                .clone()
                .unwrap_or_else(|| definition.compiled.adapter_id.clone());
            (definition.compiled.semantic_digest.to_string(), name)
        })
        .collect::<BTreeMap<_, _>>();
    snapshot
        .connections
        .into_iter()
        .filter_map(|connection| {
            definitions
                .get(&connection.descriptor.semantic_digest)
                .cloned()
                .map(|name| (connection.descriptor.connection_id, name))
        })
        .collect()
}

pub(super) async fn start_adapter_authentication(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlStartAdapterAuthenticationInput,
) -> Result<super::adapters::GraphqlAdapterOauthSetupAttempt> {
    let request =
        owned_adapter_request(state, principal, &input.request_id, input.expected_revision).await?;
    if request.challenge.challenge_kind() != CapabilityAuthenticationChallengeKind::Reauthenticate {
        return Err(async_graphql::Error::new(
            "API authentication request requires credential replacement",
        ));
    }
    let connection_id = request
        .adapter_connection_id()
        .ok_or_else(|| async_graphql::Error::new("API authentication request is unavailable"))?;
    let descriptor = state
        .adapter_operations()?
        .management_snapshot()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?
        .connections
        .into_iter()
        .find(|connection| connection.descriptor.connection_id == connection_id)
        .map(|connection| connection.descriptor)
        .ok_or_else(|| async_graphql::Error::new("API connection is unavailable"))?;
    let callback_url = state.adapter_oauth_callback_url()?;
    let attempt = state
        .adapter_operations()?
        .start_oauth_setup(
            principal,
            connection_id,
            AdapterConnectionRevisions {
                connection: descriptor.revisions.connection,
                credential: descriptor.revisions.credential,
                grant: descriptor.revisions.grant,
                policy: descriptor.revisions.policy,
            },
            super::adapters::adapter_callback_mode(callback_url)?,
            callback_url,
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    state
        .store()?
        .begin_capability_authentication(
            &request.request_id,
            request.revision,
            principal,
            &attempt.attempt_id,
        )
        .await?;
    Ok(super::adapters::GraphqlAdapterOauthSetupAttempt {
        attempt_id: attempt.attempt_id,
        authorization_url: attempt.authorization_url,
        expires_at_epoch_seconds: attempt.expires_at_epoch_seconds,
    })
}

pub(super) async fn skip_adapter_authentication(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlSkipAdapterAuthenticationInput,
) -> Result<GraphqlAdapterAuthenticationIntervention> {
    owned_adapter_request(state, principal, &input.request_id, input.expected_revision).await?;
    let request = state
        .runtime()?
        .skip_mcp_authentication_request(
            input.request_id,
            input.expected_revision,
            principal.to_string(),
        )
        .await?;
    let display_name = request.adapter_connection_id().and_then(|connection_id| {
        adapter_service_names(state, std::slice::from_ref(&request)).remove(connection_id)
    });
    GraphqlAdapterAuthenticationIntervention::from_adapter(request, display_name)
        .ok_or_else(|| async_graphql::Error::new("API authentication request is unavailable"))
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

async fn owned_adapter_request(
    state: &GraphqlState,
    principal: &str,
    request_id: &str,
    revision: u64,
) -> Result<CapabilityAuthenticationRequestRecord> {
    state
        .store()?
        .get_capability_authentication_request(request_id, revision)
        .await?
        .filter(|request| {
            request.owner_human_id == principal && request.adapter_connection_id().is_some()
        })
        .ok_or_else(|| async_graphql::Error::new("API authentication request is unavailable"))
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

impl GraphqlAdapterAuthenticationIntervention {
    fn from_adapter(
        request: CapabilityAuthenticationRequestRecord,
        display_name: Option<String>,
    ) -> Option<Self> {
        let adapter_connection_id = request.adapter_connection_id()?.to_string();
        Some(Self {
            request_id: request.request_id,
            revision: request.revision,
            conversation_id: request.conversation_id,
            task_id: request.task_id,
            run_id: request.run_id,
            adapter_connection_id,
            service_display_name: display_name.unwrap_or(request.authority_display_name),
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
