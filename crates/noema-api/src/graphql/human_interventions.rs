//! Unified human-attention projection for task gates, permissions, setup, and authentication.

use async_graphql::{Enum, InputObject, Result, SimpleObject, Union};
use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallengeKind,
};
use noema_capabilities_mcp::{
    McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus, McpServerRecord,
    StartMcpOAuthReauthenticationCommand,
};
use noema_capability_adapters::AdapterConnectionRevisions;
use noema_store::{CapabilityAuthenticationRequestRecord, CapabilityAuthenticationRequestState};
use serde::Deserialize;
use std::collections::BTreeMap;

use super::{
    adapters::{GraphqlAdapterDefinition, adapter_definitions},
    governed_actions::{GraphqlGovernedAction, pending_governed_actions},
    mcp::GraphqlMcpOAuthSetupAttempt,
    runtime_state::GraphqlState,
    tasks::{self, GraphqlTaskAttention},
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

/// Durable chat-driven MCP setup waiting for authentication or policy.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "McpSetupIntervention")]
pub struct GraphqlMcpSetupIntervention {
    pub item_id: String,
    pub conversation_id: String,
    pub setup_status: String,
    pub display_name: String,
    pub description: Option<String>,
    pub service_url: String,
    pub endpoint_url: String,
    pub oauth_supported: bool,
    pub discovered_tool_count: usize,
    pub mcp_server_id: Option<String>,
    pub connection_revision: Option<String>,
    pub policy_revision: Option<u64>,
    pub tool_count: Option<usize>,
}

/// Human intervention variants share presentation, but retain separate authorities.
#[derive(Clone, Debug, Union)]
#[graphql(name = "HumanIntervention")]
pub enum GraphqlHumanIntervention {
    TaskAttention(Box<GraphqlTaskAttention>),
    GovernedAction(GraphqlGovernedAction),
    McpAuthentication(GraphqlMcpAuthenticationIntervention),
    AdapterAuthentication(GraphqlAdapterAuthenticationIntervention),
    McpSetup(GraphqlMcpSetupIntervention),
    AdapterDefinition(Box<GraphqlAdapterDefinition>),
}

/// Resolve one exact MCP setup intervention after its policy is configured.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ResolveMcpSetupInterventionInput")]
pub struct GraphqlResolveMcpSetupInterventionInput {
    pub item_id: String,
    pub mcp_server_id: String,
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
    project_id: Option<String>,
    first: Option<i32>,
) -> Result<Vec<GraphqlHumanIntervention>> {
    let first = usize::try_from(first.unwrap_or(50).clamp(1, 100)).unwrap_or(50);
    let task_attentions = pending_task_attentions(
        state,
        principal,
        conversation_id.as_deref(),
        task_id.as_deref(),
        project_id.as_deref(),
        first,
    )
    .await?;
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
    let mcp_setups =
        pending_mcp_setups(state, principal, conversation_id.as_deref(), first).await?;
    let adapter_service_names = adapter_service_names(state, &authentications);
    let adapter_reviews = if conversation_id.is_some() && task_id.is_none() {
        adapter_definitions(state)
            .await?
            .into_iter()
            .filter(|definition| {
                if definition.superseded {
                    return false;
                }
                !definition.reviewed
                    || (definition.connection_count == 0 && definition.credential_setup.is_some())
                    || definition.connections.iter().any(|connection| {
                        connection.status == "authentication_required"
                            || (connection.status == "active" && !connection.policy_configured)
                    })
            })
            .map(Box::new)
            .map(GraphqlHumanIntervention::AdapterDefinition)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(task_attentions
        .into_iter()
        .map(|attention| GraphqlHumanIntervention::TaskAttention(Box::new(attention)))
        .chain(
            actions
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
                            GraphqlAdapterAuthenticationIntervention::from_adapter(
                                request,
                                display_name,
                            )
                            .map(GraphqlHumanIntervention::AdapterAuthentication)
                        }
                    }
                }))
                .chain(
                    mcp_setups
                        .into_iter()
                        .map(GraphqlHumanIntervention::McpSetup),
                )
                .chain(adapter_reviews),
        )
        .take(first)
        .collect())
}

async fn pending_mcp_setups(
    state: &GraphqlState,
    principal: &str,
    conversation_id: Option<&str>,
    first: usize,
) -> Result<Vec<GraphqlMcpSetupIntervention>> {
    let Some(conversation_id) = conversation_id else {
        return Ok(Vec::new());
    };
    if !state
        .store()?
        .conversation_is_owned_by_human(conversation_id, principal)
        .await?
    {
        return Ok(Vec::new());
    }
    let items = state
        .store()?
        .list_pending_mcp_setup_items(conversation_id, first)
        .await?;
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let servers = state
        .mcp_operations()?
        .list_servers()
        .await
        .map_err(super::errors::graphql_error)?
        .servers;
    Ok(items
        .into_iter()
        .filter_map(|item| {
            let setup = stored_mcp_setup(item.payload_json.get("metadata")?)?;
            let server = servers
                .iter()
                .find(|server| mcp_server_matches(server, &setup));
            if server.is_some_and(mcp_server_policy_configured) {
                return None;
            }
            GraphqlMcpSetupIntervention::from_stored(
                item.item_id,
                item.conversation_id,
                setup,
                server,
            )
        })
        .collect())
}

pub(super) async fn resolve_mcp_setup_intervention(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlResolveMcpSetupInterventionInput,
) -> Result<bool> {
    let item = state
        .store()?
        .get_visible_conversation_item(&input.item_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("MCP setup intervention is unavailable"))?;
    if !state
        .store()?
        .conversation_is_owned_by_human(&item.conversation_id, principal)
        .await?
    {
        return Err(async_graphql::Error::new(
            "MCP setup intervention is unavailable",
        ));
    }
    let setup = item
        .payload_json
        .get("metadata")
        .and_then(stored_mcp_setup)
        .ok_or_else(|| async_graphql::Error::new("MCP setup intervention is unavailable"))?;
    if setup
        .intervention_resolution
        .as_ref()
        .is_some_and(|resolution| resolution.mcp_server_id == input.mcp_server_id)
    {
        return Ok(true);
    }
    let server = state
        .mcp_operations()?
        .list_servers()
        .await
        .map_err(super::errors::graphql_error)?
        .servers
        .into_iter()
        .find(|server| server.mcp_server_id == input.mcp_server_id)
        .filter(|server| mcp_server_matches(server, &setup) && mcp_server_policy_configured(server))
        .ok_or_else(|| async_graphql::Error::new("MCP setup is not configured"))?;
    let changed = state
        .store()?
        .resolve_mcp_setup_item(&item.conversation_id, &item.item_id, &server.mcp_server_id)
        .await?;
    if changed {
        state.subscriptions().publish_conversation(
            noema_runtime::ConversationRuntimeEvent::HumanInterventionsChanged {
                conversation_id: item.conversation_id.clone(),
            },
        );
        if let Some(runtime) = state.optional_runtime().cloned() {
            let completion = noema_runtime::CapabilitySetupCompletion {
                human_id: principal.to_string(),
                integration_kind: noema_runtime::CapabilityIntegrationKind::Mcp,
                integration_name: setup.display_name,
                connection_id: server.mcp_server_id,
                connection_revision: server.authority_generation,
                granted_scopes: Vec::new(),
                enabled_tool_count: server.tool_count,
            };
            tokio::spawn(async move {
                let _ = runtime
                    .narrate_capability_setup_completion(completion)
                    .await;
            });
        }
    }
    Ok(changed)
}

#[derive(Deserialize)]
struct StoredMcpSetupMetadata {
    action: StoredMcpSetupAction,
}

#[derive(Deserialize)]
struct StoredMcpSetupAction {
    name: String,
    success: bool,
    payload: StoredMcpSetup,
}

#[derive(Deserialize)]
struct StoredMcpSetup {
    status: String,
    service_url: String,
    display_name: String,
    description: Option<String>,
    endpoint_url: String,
    #[serde(default)]
    setup_result: StoredMcpSetupResult,
    #[serde(default)]
    intervention_resolution: Option<StoredMcpSetupResolution>,
}

#[derive(Default, Deserialize)]
struct StoredMcpSetupResult {
    #[serde(default)]
    discovered_tool_count: usize,
}

#[derive(Deserialize)]
struct StoredMcpSetupResolution {
    mcp_server_id: String,
}

fn stored_mcp_setup(metadata: &serde_json::Value) -> Option<StoredMcpSetup> {
    let stored: StoredMcpSetupMetadata = serde_json::from_value(metadata.clone()).ok()?;
    if stored.action.name != "mcp.connect_service" || !stored.action.success {
        return None;
    }
    let setup = stored.action.payload;
    if !matches!(
        setup.status.as_str(),
        "needs_auth" | "authentication_available" | "ready_for_policy"
    ) {
        return None;
    }
    Some(setup)
}

fn mcp_server_matches(server: &McpServerRecord, setup: &StoredMcpSetup) -> bool {
    server.transport_kind == noema_capabilities_mcp::McpTransportKind::StreamableHttp
        && server
            .safe_config
            .get("url")
            .and_then(serde_json::Value::as_str)
            == Some(setup.endpoint_url.as_str())
}

fn mcp_server_policy_configured(server: &McpServerRecord) -> bool {
    server.data_sharing_policy.is_some() && server.unsafe_action_policy.is_some()
}

impl GraphqlMcpSetupIntervention {
    fn from_stored(
        item_id: String,
        conversation_id: String,
        setup: StoredMcpSetup,
        server: Option<&McpServerRecord>,
    ) -> Option<Self> {
        if setup.intervention_resolution.is_some() {
            return None;
        }
        let setup_status = if server.is_some() {
            "ready_for_policy".to_string()
        } else {
            setup.status.clone()
        };
        if setup_status == "ready_for_policy" && server.is_none() {
            return None;
        }
        Some(Self {
            item_id,
            conversation_id,
            setup_status: setup_status.clone(),
            display_name: setup.display_name,
            description: setup.description,
            service_url: setup.service_url,
            endpoint_url: setup.endpoint_url,
            oauth_supported: matches!(
                setup_status.as_str(),
                "needs_auth" | "authentication_available"
            ),
            discovered_tool_count: setup.setup_result.discovered_tool_count,
            mcp_server_id: server.map(|server| server.mcp_server_id.clone()),
            connection_revision: server.map(|server| server.authority_generation.clone()),
            policy_revision: server.map(|server| server.policy_revision),
            tool_count: server.map(|server| server.tool_count),
        })
    }
}

async fn pending_task_attentions(
    state: &GraphqlState,
    principal: &str,
    conversation_id: Option<&str>,
    task_id: Option<&str>,
    project_id: Option<&str>,
    first: usize,
) -> Result<Vec<GraphqlTaskAttention>> {
    if let Some(task_id) = task_id {
        let detail = tasks::task(state, principal, task_id.to_string()).await?;
        if conversation_id.is_some_and(|conversation_id| {
            detail.source.conversation_id.as_deref() != Some(conversation_id)
        }) || project_id.is_some_and(|project_id| {
            detail
                .project
                .as_ref()
                .map(|project| project.project_id.as_str())
                != Some(project_id)
        }) {
            return Ok(Vec::new());
        }
        return Ok(detail.attention.into_iter().collect());
    }
    if conversation_id.is_some() {
        return Ok(Vec::new());
    }
    let connection = tasks::needs_you(
        state,
        principal,
        "workspace:personal".to_string(),
        project_id.map(str::to_string),
        Some(i32::try_from(first).unwrap_or(50)),
        None,
    )
    .await?;
    Ok(connection.edges.into_iter().map(|edge| edge.node).collect())
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
