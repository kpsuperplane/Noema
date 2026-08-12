//! Source-neutral API and MCP management views with structured dispatch.

use async_graphql::{Json, Result};
use noema_capabilities::{
    CapabilityConnectionPolicy, CapabilityDataSharingPolicy, CapabilityExecutionDecision,
    CapabilityToolBehavior, CapabilityToolHint, CapabilityToolHintSource, CapabilityToolPolicy,
    CapabilityToolPolicyOverride, CapabilityToolPolicyStatus, CapabilityUnsafeActionPolicy,
    resolve_capability_execution_decision,
};
use noema_capabilities_mcp::{
    McpControlPlaneTool, McpListToolsCommand, McpResetToolPolicyCommand,
    McpSaveConnectionLabelCommand, McpSaveProviderPolicyCommand, McpSaveToolOverrideCommand,
    McpServerRecord, McpSetToolEnabledCommand,
};
use noema_capability_adapters::{
    AdapterConnectionAuthenticationV1, AdapterManagementFence, CompiledAdapterDefinition,
    CompiledOperation, ConnectionInstall, DefinitionInstall,
};
use serde_json::json;
use std::collections::BTreeMap;

pub(super) use super::capability_integration_models::*;
use super::{GraphqlState, adapters, errors::graphql_error};

pub(super) async fn integrations(
    state: &GraphqlState,
    kind: GraphqlCapabilityIntegrationKind,
) -> Result<Vec<GraphqlCapabilityIntegration>> {
    match kind {
        GraphqlCapabilityIntegrationKind::Api => api_integrations(state),
        GraphqlCapabilityIntegrationKind::Mcp => mcp_integrations(state).await,
    }
}

pub(super) async fn connection(
    state: &GraphqlState,
    reference: GraphqlCapabilityConnectionRefInput,
) -> Result<Option<GraphqlCapabilityConnection>> {
    Ok(integrations(state, reference.kind)
        .await?
        .into_iter()
        .flat_map(|integration| integration.connections)
        .find(|connection| connection.connection_id == reference.connection_id))
}

pub(super) async fn tools(
    state: &GraphqlState,
    reference: GraphqlCapabilityConnectionRefInput,
) -> Result<Vec<GraphqlCapabilityManagedTool>> {
    match reference.kind {
        GraphqlCapabilityIntegrationKind::Api => api_tools(state, &reference.connection_id),
        GraphqlCapabilityIntegrationKind::Mcp => mcp_tools(state, &reference.connection_id).await,
    }
}

pub(super) async fn save_connection_policy(
    state: &GraphqlState,
    input: GraphqlSaveCapabilityConnectionPolicyInput,
) -> Result<GraphqlCapabilityConnection> {
    let data_sharing = input
        .data_sharing_policy
        .parse::<CapabilityDataSharingPolicy>()
        .map_err(|_| async_graphql::Error::new("invalid capability connection policy"))?;
    let unsafe_actions = input
        .unsafe_action_policy
        .parse::<CapabilityUnsafeActionPolicy>()
        .map_err(|_| async_graphql::Error::new("invalid capability connection policy"))?;
    match input.kind {
        GraphqlCapabilityIntegrationKind::Api => {
            let installed = state
                .adapter_operations()?
                .save_management_policy(
                    api_fence(
                        input.connection_id.clone(),
                        &input.expected_connection_revision,
                        input.expected_policy_revision,
                    )?,
                    data_sharing,
                    unsafe_actions,
                )
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?;
            let definition = adapters::adapter_definitions(state)
                .await?
                .into_iter()
                .find(|definition| {
                    definition.semantic_digest == installed.descriptor.semantic_digest
                })
                .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))?;
            adapters::publish_primary_interventions_changed(state).await;
            adapters::queue_ready_adapter_setup(
                state,
                &definition.display_name,
                &installed.descriptor,
            );
        }
        GraphqlCapabilityIntegrationKind::Mcp => {
            state
                .mcp_operations()?
                .save_provider_policy(McpSaveProviderPolicyCommand {
                    mcp_server_id: input.connection_id.clone(),
                    data_sharing_policy: data_sharing,
                    unsafe_action_policy: unsafe_actions,
                    expected_policy_revision: input.expected_policy_revision,
                    expected_connection_revision: input.expected_connection_revision,
                })
                .await
                .map_err(graphql_error)?;
        }
    }
    require_connection(state, input.kind, input.connection_id).await
}

pub(super) async fn save_connection_label(
    state: &GraphqlState,
    input: GraphqlSaveCapabilityConnectionLabelInput,
) -> Result<GraphqlCapabilityConnection> {
    match input.kind {
        GraphqlCapabilityIntegrationKind::Api => {
            let expected_connection_revision = input
                .expected_connection_revision
                .parse::<u64>()
                .map_err(|_| async_graphql::Error::new("invalid connection revision"))?;
            state
                .adapter_operations()?
                .save_connection_label(
                    input.connection_id.clone(),
                    expected_connection_revision,
                    input.expected_connection_label,
                    input.connection_label,
                )
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?;
            adapters::reconcile_adapter_connections(state).await?;
        }
        GraphqlCapabilityIntegrationKind::Mcp => {
            state
                .mcp_operations()?
                .save_connection_label(McpSaveConnectionLabelCommand {
                    mcp_server_id: input.connection_id.clone(),
                    expected_connection_revision: input.expected_connection_revision,
                    expected_connection_label: input.expected_connection_label,
                    connection_label: input.connection_label,
                })
                .await
                .map_err(graphql_error)?;
        }
    }
    require_connection(state, input.kind, input.connection_id).await
}

pub(super) async fn save_tool_override(
    state: &GraphqlState,
    input: GraphqlSaveCapabilityToolOverrideInput,
) -> Result<GraphqlCapabilityManagedTool> {
    let policy = CapabilityToolPolicyOverride {
        tool_id: input.tool_id.clone(),
        read_only: input.read_only,
        idempotent: input.idempotent,
        destructive: input.destructive,
        open_world: input.open_world,
        source_revision: input.source_revision.clone(),
    };
    match input.kind {
        GraphqlCapabilityIntegrationKind::Api => {
            state
                .adapter_operations()?
                .save_management_tool_override(
                    api_fence(
                        input.connection_id.clone(),
                        &input.expected_connection_revision,
                        input.expected_policy_revision,
                    )?,
                    policy,
                )
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        }
        GraphqlCapabilityIntegrationKind::Mcp => {
            state
                .mcp_operations()?
                .save_tool_override(McpSaveToolOverrideCommand {
                    policy,
                    expected_policy_revision: input.expected_policy_revision,
                    expected_connection_revision: input.expected_connection_revision,
                })
                .await
                .map_err(graphql_error)?;
        }
    }
    require_tool(state, input.kind, input.connection_id, input.tool_id).await
}

pub(super) async fn reset_tool_policy(
    state: &GraphqlState,
    input: GraphqlResetCapabilityToolPolicyInput,
) -> Result<GraphqlCapabilityManagedTool> {
    match input.kind {
        GraphqlCapabilityIntegrationKind::Api => {
            state
                .adapter_operations()?
                .reset_management_tool_policy(
                    api_fence(
                        input.connection_id.clone(),
                        &input.expected_connection_revision,
                        input.expected_policy_revision,
                    )?,
                    input.tool_id.clone(),
                    input.source_revision.clone(),
                )
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        }
        GraphqlCapabilityIntegrationKind::Mcp => {
            state
                .mcp_operations()?
                .reset_tool_policy(McpResetToolPolicyCommand {
                    mcp_tool_id: input.tool_id.clone(),
                    source_revision: input.source_revision,
                    expected_policy_revision: input.expected_policy_revision,
                    expected_connection_revision: input.expected_connection_revision,
                })
                .await
                .map_err(graphql_error)?;
        }
    }
    require_tool(state, input.kind, input.connection_id, input.tool_id).await
}

pub(super) async fn set_tool_enabled(
    state: &GraphqlState,
    input: GraphqlSetCapabilityToolEnabledInput,
) -> Result<GraphqlCapabilityManagedTool> {
    match input.kind {
        GraphqlCapabilityIntegrationKind::Api => {
            state
                .adapter_operations()?
                .set_management_tool_enabled(
                    api_fence(
                        input.connection_id.clone(),
                        &input.expected_connection_revision,
                        input.expected_policy_revision,
                    )?,
                    input.tool_id.clone(),
                    input.source_revision.clone(),
                    input.enabled,
                )
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        }
        GraphqlCapabilityIntegrationKind::Mcp => {
            state
                .mcp_operations()?
                .set_tool_enabled(McpSetToolEnabledCommand {
                    mcp_tool_id: input.tool_id.clone(),
                    enabled: input.enabled,
                    source_revision: input.source_revision,
                    expected_policy_revision: input.expected_policy_revision,
                    expected_connection_revision: input.expected_connection_revision,
                })
                .await
                .map_err(graphql_error)?;
        }
    }
    require_tool(state, input.kind, input.connection_id, input.tool_id).await
}

fn api_integrations(state: &GraphqlState) -> Result<Vec<GraphqlCapabilityIntegration>> {
    let snapshot = state
        .adapter_operations()?
        .management_snapshot()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let mut grouped = BTreeMap::<String, Vec<&DefinitionInstall>>::new();
    for definition in &snapshot.definitions.definitions {
        grouped
            .entry(definition.compiled.definition_id.clone())
            .or_default()
            .push(definition);
    }
    Ok(grouped
        .into_iter()
        .map(|(definition_id, definitions)| {
            let rank = |left: &&DefinitionInstall, right: &&DefinitionInstall| {
                left.compiled
                    .definition_revision
                    .cmp(&right.compiled.definition_revision)
                    .then_with(|| {
                        left.compiled
                            .semantic_digest
                            .cmp(&right.compiled.semantic_digest)
                    })
            };
            let current = definitions
                .iter()
                .copied()
                .filter(|definition| {
                    definition.compiled.reviewed
                        && !snapshot
                            .replaced_definition_digests
                            .contains(definition.compiled.semantic_digest.as_str())
                })
                .max_by(rank)
                .or_else(|| {
                    definitions
                        .iter()
                        .copied()
                        .filter(|definition| !definition.compiled.reviewed)
                        .max_by(rank)
                })
                .or_else(|| {
                    definitions
                        .iter()
                        .copied()
                        .filter(|definition| definition.compiled.reviewed)
                        .max_by(rank)
                })
                .expect("group is non-empty");
            let mut connections = snapshot
                .connections
                .connections
                .iter()
                .filter_map(|connection| {
                    let definition = definitions.iter().find(|definition| {
                        definition.compiled.semantic_digest.as_str()
                            == connection.descriptor.semantic_digest
                    })?;
                    Some(api_connection(connection, &definition.compiled))
                })
                .collect::<Vec<_>>();
            connections.sort_by(|left, right| left.name.cmp(&right.name));
            GraphqlCapabilityIntegration {
                kind: GraphqlCapabilityIntegrationKind::Api,
                definition_id,
                name: current.compiled.adapter_id.clone(),
                source_revision: current.compiled.semantic_digest.to_string(),
                reviewed: current.compiled.reviewed,
                source_summary: current.compiled.origin.clone(),
                connections,
            }
        })
        .collect())
}

async fn mcp_integrations(state: &GraphqlState) -> Result<Vec<GraphqlCapabilityIntegration>> {
    let servers = state
        .mcp_operations()?
        .list_servers()
        .await
        .map_err(graphql_error)?
        .servers;
    let mut grouped = BTreeMap::<String, Vec<McpServerRecord>>::new();
    for server in servers {
        grouped
            .entry(server.mcp_definition_id.clone())
            .or_default()
            .push(server);
    }
    Ok(grouped
        .into_iter()
        .map(|(definition_id, mut servers)| {
            servers.sort_by(|left, right| left.mcp_server_id.cmp(&right.mcp_server_id));
            let definition = &servers[0];
            GraphqlCapabilityIntegration {
                kind: GraphqlCapabilityIntegrationKind::Mcp,
                definition_id,
                name: definition.display_name.clone(),
                source_revision: definition.definition_revision.clone(),
                reviewed: true,
                source_summary: definition.transport_kind.as_str().to_string(),
                connections: servers.into_iter().map(mcp_connection).collect(),
            }
        })
        .collect())
}

fn api_tools(
    state: &GraphqlState,
    connection_id: &str,
) -> Result<Vec<GraphqlCapabilityManagedTool>> {
    let snapshot = state
        .adapter_operations()?
        .management_snapshot()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let connection = snapshot
        .connections
        .connections
        .iter()
        .find(|candidate| candidate.descriptor.connection_id == connection_id)
        .ok_or_else(|| async_graphql::Error::new("capability connection was not found"))?;
    let definition = snapshot
        .definitions
        .definitions
        .iter()
        .find(|candidate| {
            candidate.compiled.semantic_digest.as_str() == connection.descriptor.semantic_digest
        })
        .ok_or_else(|| async_graphql::Error::new("capability connection is unavailable"))?;
    Ok(definition
        .compiled
        .operations
        .iter()
        .map(|operation| api_tool(connection, operation))
        .collect())
}

async fn mcp_tools(
    state: &GraphqlState,
    connection_id: &str,
) -> Result<Vec<GraphqlCapabilityManagedTool>> {
    let list = state
        .mcp_operations()?
        .list_tools(McpListToolsCommand {
            mcp_server_id: connection_id.to_string(),
        })
        .await
        .map_err(graphql_error)?;
    let connection_policy = mcp_policy(&list.server);
    Ok(list
        .tools
        .into_iter()
        .map(|tool| mcp_tool(&list.server, connection_policy, tool))
        .collect())
}

fn api_connection(
    connection: &ConnectionInstall,
    definition: &CompiledAdapterDefinition,
) -> GraphqlCapabilityConnection {
    let enabled = connection.descriptor.allowed_operations.len();
    let total = definition.operations.len();
    GraphqlCapabilityConnection {
        kind: GraphqlCapabilityIntegrationKind::Api,
        definition_id: definition.definition_id.clone(),
        connection_id: connection.descriptor.connection_id.clone(),
        name: connection
            .descriptor
            .connection_label
            .clone()
            .unwrap_or_else(|| connection.descriptor.connection_slug.clone()),
        connection_label: connection.descriptor.connection_label.clone(),
        source_revision: connection.descriptor.semantic_digest.clone(),
        connection_revision: connection.descriptor.connection_revision.to_string(),
        credential_revision: match connection.descriptor.authentication {
            AdapterConnectionAuthenticationV1::Credential { revision, .. } => Some(revision),
            _ => None,
        },
        grant_revision: None,
        policy_revision: connection.descriptor.policy_revision,
        status: connection.descriptor.status.as_str().to_string(),
        health_status: connection.descriptor.status.as_str().to_string(),
        auth_status: if matches!(
            connection.descriptor.status,
            noema_capability_adapters::AdapterConnectionStatus::AuthenticationRequired
        ) {
            "required".to_string()
        } else {
            "authenticated".to_string()
        },
        data_sharing_policy: connection
            .descriptor
            .policy
            .map(|policy| policy.data_sharing.as_str().to_string()),
        unsafe_action_policy: connection
            .descriptor
            .policy
            .map(|policy| policy.unsafe_actions.as_str().to_string()),
        tool_count: total,
        available_tool_count: enabled,
        pending_tool_count: usize::from(connection.descriptor.policy.is_none()),
        defaulted_tool_count: definition
            .operations
            .iter()
            .filter(|operation| {
                operation.tool_policy.status == CapabilityToolPolicyStatus::Defaulted
            })
            .count(),
        disabled_tool_count: total.saturating_sub(enabled),
        source_details: Json(json!({
            "origin": definition.origin,
            "authenticationMode": definition.authentication.mode(),
            "operationScopes": definition.operations.iter().map(|operation| json!({
                "operationId": operation.operation_id,
                "acceptedScopeSets": operation.authorization.accepted_scope_sets(),
            })).collect::<Vec<_>>(),
            "grantId": match &connection.descriptor.authentication {
                AdapterConnectionAuthenticationV1::OauthGrant { grant_id } => Some(grant_id),
                _ => None,
            },
        })),
    }
}

fn mcp_connection(server: McpServerRecord) -> GraphqlCapabilityConnection {
    let connection_label = server.connection_label.clone();
    GraphqlCapabilityConnection {
        kind: GraphqlCapabilityIntegrationKind::Mcp,
        definition_id: server.mcp_definition_id,
        connection_id: server.mcp_server_id,
        name: connection_label
            .clone()
            .unwrap_or_else(|| server.display_name.clone()),
        connection_label,
        source_revision: server.definition_revision,
        connection_revision: server.authority_generation,
        credential_revision: None,
        grant_revision: None,
        policy_revision: server.policy_revision,
        status: if server.enabled {
            "ready"
        } else {
            "setup_required"
        }
        .to_string(),
        health_status: server.health_status.as_str().to_string(),
        auth_status: server.auth_status.as_str().to_string(),
        data_sharing_policy: server
            .data_sharing_policy
            .map(|policy| policy.as_str().to_string()),
        unsafe_action_policy: server
            .unsafe_action_policy
            .map(|policy| policy.as_str().to_string()),
        tool_count: server.tool_count,
        available_tool_count: server.available_tool_count,
        pending_tool_count: server.pending_tool_count,
        defaulted_tool_count: server.defaulted_tool_count,
        disabled_tool_count: server.disabled_tool_count,
        source_details: Json(json!({
            "transportKind": server.transport_kind.as_str(),
        })),
    }
}

fn api_tool(
    connection: &ConnectionInstall,
    operation: &CompiledOperation,
) -> GraphqlCapabilityManagedTool {
    let enabled = connection
        .descriptor
        .allowed_operations
        .contains(&operation.operation_id);
    let mut policy = operation.tool_policy.clone();
    if let Some(override_policy) = connection
        .descriptor
        .tool_overrides
        .iter()
        .find(|candidate| candidate.tool_id == operation.operation_id)
    {
        policy.read_only = human_hint(override_policy.read_only);
        policy.idempotent = human_hint(override_policy.idempotent);
        policy.destructive = human_hint(override_policy.destructive);
        policy.open_world = human_hint(override_policy.open_world);
        policy.status = CapabilityToolPolicyStatus::Ready;
    }
    policy.policy_revision = connection.descriptor.policy_revision;
    if !enabled {
        policy.status = CapabilityToolPolicyStatus::Disabled;
    }
    managed_tool(
        GraphqlCapabilityIntegrationKind::Api,
        &connection.descriptor.connection_id,
        operation.operation_id.clone(),
        operation.operation_id.clone(),
        None,
        enabled,
        policy,
        connection.descriptor.policy,
        json!({ "method": operation.method, "path": operation.path }),
    )
}

fn mcp_tool(
    server: &McpServerRecord,
    connection_policy: Option<CapabilityConnectionPolicy>,
    tool: McpControlPlaneTool,
) -> GraphqlCapabilityManagedTool {
    let policy = tool.policy.unwrap_or_else(|| CapabilityToolPolicy {
        tool_id: tool.tool.mcp_tool_id.clone(),
        read_only: CapabilityToolHint {
            value: None,
            source: None,
        },
        idempotent: CapabilityToolHint {
            value: None,
            source: None,
        },
        destructive: CapabilityToolHint {
            value: None,
            source: None,
        },
        open_world: CapabilityToolHint {
            value: None,
            source: None,
        },
        status: CapabilityToolPolicyStatus::Pending,
        policy_revision: 0,
        source_revision: tool.tool.metadata_fingerprint.clone(),
    });
    let enabled = policy.status != CapabilityToolPolicyStatus::Disabled;
    managed_tool(
        GraphqlCapabilityIntegrationKind::Mcp,
        &server.mcp_server_id,
        tool.tool.mcp_tool_id,
        tool.tool.name,
        tool.tool.description,
        enabled,
        policy,
        connection_policy,
        json!({}),
    )
}

#[allow(clippy::too_many_arguments)]
fn managed_tool(
    kind: GraphqlCapabilityIntegrationKind,
    connection_id: &str,
    tool_id: String,
    name: String,
    description: Option<String>,
    enabled: bool,
    policy: CapabilityToolPolicy,
    connection_policy: Option<CapabilityConnectionPolicy>,
    source_details: serde_json::Value,
) -> GraphqlCapabilityManagedTool {
    let decision_preview =
        connection_policy
            .zip(raw_behavior(&policy))
            .map(|(connection, behavior)| {
                decision_label(resolve_capability_execution_decision(connection, behavior))
                    .to_string()
            });
    GraphqlCapabilityManagedTool {
        kind,
        connection_id: connection_id.to_string(),
        tool_id,
        name,
        description,
        enabled,
        read_only: policy.read_only.into(),
        idempotent: policy.idempotent.into(),
        destructive: policy.destructive.into(),
        open_world: policy.open_world.into(),
        status: policy.status.as_str().to_string(),
        policy_revision: policy.policy_revision,
        source_revision: policy.source_revision,
        decision_preview,
        source_details: Json(source_details),
    }
}

impl From<CapabilityToolHint> for GraphqlCapabilityManagedToolHint {
    fn from(hint: CapabilityToolHint) -> Self {
        Self {
            value: hint.value,
            source: hint.source.map(|source| source.as_str().to_string()),
        }
    }
}

fn raw_behavior(policy: &CapabilityToolPolicy) -> Option<CapabilityToolBehavior> {
    Some(CapabilityToolBehavior {
        read_only: policy.read_only.value?,
        idempotent: policy.idempotent.value?,
        destructive: policy.destructive.value?,
        open_world: policy.open_world.value?,
    })
}

const fn decision_label(decision: CapabilityExecutionDecision) -> &'static str {
    match decision {
        CapabilityExecutionDecision::ExecuteImmediately => "EXECUTE_IMMEDIATELY",
        CapabilityExecutionDecision::HumanReview => "HUMAN_REVIEW",
        CapabilityExecutionDecision::LlmReview => "LLM_REVIEW",
    }
}

fn human_hint(value: bool) -> CapabilityToolHint {
    CapabilityToolHint {
        value: Some(value),
        source: Some(CapabilityToolHintSource::Human),
    }
}

fn mcp_policy(server: &McpServerRecord) -> Option<CapabilityConnectionPolicy> {
    Some(CapabilityConnectionPolicy {
        data_sharing: server.data_sharing_policy?,
        unsafe_actions: server.unsafe_action_policy?,
        revision: server.policy_revision,
    })
}

fn api_fence(
    connection_id: String,
    connection_revision: &str,
    policy_revision: u64,
) -> Result<AdapterManagementFence> {
    Ok(AdapterManagementFence {
        connection_id,
        expected_connection_revision: connection_revision
            .parse()
            .map_err(|_| async_graphql::Error::new("invalid capability connection revision"))?,
        expected_policy_revision: policy_revision,
    })
}

async fn require_connection(
    state: &GraphqlState,
    kind: GraphqlCapabilityIntegrationKind,
    connection_id: String,
) -> Result<GraphqlCapabilityConnection> {
    connection(
        state,
        GraphqlCapabilityConnectionRefInput {
            kind,
            connection_id,
        },
    )
    .await?
    .ok_or_else(|| async_graphql::Error::new("capability connection was not found"))
}

async fn require_tool(
    state: &GraphqlState,
    kind: GraphqlCapabilityIntegrationKind,
    connection_id: String,
    tool_id: String,
) -> Result<GraphqlCapabilityManagedTool> {
    tools(
        state,
        GraphqlCapabilityConnectionRefInput {
            kind,
            connection_id,
        },
    )
    .await?
    .into_iter()
    .find(|tool| tool.tool_id == tool_id)
    .ok_or_else(|| async_graphql::Error::new("capability tool was not found"))
}
