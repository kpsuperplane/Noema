//! GraphQL models for source-neutral API and MCP management.

use async_graphql::{Enum, InputObject, Json, SimpleObject};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "CapabilityIntegrationKind")]
pub enum GraphqlCapabilityIntegrationKind {
    Api,
    Mcp,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CapabilityConnectionRefInput")]
pub struct GraphqlCapabilityConnectionRefInput {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub connection_id: String,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "CapabilityManagedToolHint")]
pub struct GraphqlCapabilityManagedToolHint {
    pub value: Option<bool>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "CapabilityIntegration")]
pub struct GraphqlCapabilityIntegration {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub definition_id: String,
    pub name: String,
    pub source_revision: String,
    pub reviewed: bool,
    pub source_summary: String,
    pub connections: Vec<GraphqlCapabilityConnection>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "CapabilityConnection")]
pub struct GraphqlCapabilityConnection {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub definition_id: String,
    pub connection_id: String,
    pub name: String,
    pub source_revision: String,
    pub connection_revision: String,
    pub policy_revision: u64,
    pub status: String,
    pub health_status: String,
    pub auth_status: String,
    pub data_sharing_policy: Option<String>,
    pub unsafe_action_policy: Option<String>,
    pub tool_count: usize,
    pub available_tool_count: usize,
    pub pending_tool_count: usize,
    pub defaulted_tool_count: usize,
    pub disabled_tool_count: usize,
    pub source_details: Json<serde_json::Value>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "CapabilityManagedTool")]
pub struct GraphqlCapabilityManagedTool {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub connection_id: String,
    pub tool_id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub read_only: GraphqlCapabilityManagedToolHint,
    pub idempotent: GraphqlCapabilityManagedToolHint,
    pub destructive: GraphqlCapabilityManagedToolHint,
    pub open_world: GraphqlCapabilityManagedToolHint,
    pub status: String,
    pub policy_revision: u64,
    pub source_revision: String,
    pub decision_preview: Option<String>,
    pub source_details: Json<serde_json::Value>,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveCapabilityConnectionPolicyInput")]
pub struct GraphqlSaveCapabilityConnectionPolicyInput {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub connection_id: String,
    pub expected_connection_revision: String,
    pub expected_policy_revision: u64,
    pub data_sharing_policy: String,
    pub unsafe_action_policy: String,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveCapabilityToolOverrideInput")]
pub struct GraphqlSaveCapabilityToolOverrideInput {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub connection_id: String,
    pub expected_connection_revision: String,
    pub tool_id: String,
    pub source_revision: String,
    pub expected_policy_revision: u64,
    pub read_only: bool,
    pub idempotent: bool,
    pub destructive: bool,
    pub open_world: bool,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ResetCapabilityToolPolicyInput")]
pub struct GraphqlResetCapabilityToolPolicyInput {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub connection_id: String,
    pub expected_connection_revision: String,
    pub tool_id: String,
    pub source_revision: String,
    pub expected_policy_revision: u64,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SetCapabilityToolEnabledInput")]
pub struct GraphqlSetCapabilityToolEnabledInput {
    pub kind: GraphqlCapabilityIntegrationKind,
    pub connection_id: String,
    pub expected_connection_revision: String,
    pub tool_id: String,
    pub source_revision: String,
    pub expected_policy_revision: u64,
    pub enabled: bool,
}
