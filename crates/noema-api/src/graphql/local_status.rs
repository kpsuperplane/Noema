use async_graphql::{Enum, Result, SimpleObject};

use super::{errors::graphql_error, schema::GraphqlState};

/// Local service status shown by clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "LocalServiceStatus")]
pub enum GraphqlLocalServiceStatus {
    /// The local Noema service is running.
    Running,
}

/// Assistant connection exposed to clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "AssistantConnection")]
pub enum GraphqlAssistantConnection {
    /// The daemon is using Codex for chat.
    Codex,
}

/// Memory storage readiness shown by clients.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Enum)]
#[graphql(name = "MemoryStorageStatus")]
pub enum GraphqlMemoryStorageStatus {
    /// The local memory service is ready.
    #[default]
    Ready,
    /// The local memory service is initializing.
    Initializing,
    /// Memory service writes and retrieval are not available yet.
    Unavailable,
}

/// Local status returned by `Query.localStatus`.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalStatus")]
pub struct GraphqlLocalStatus {
    /// Local service status.
    pub local_service: GraphqlLocalServiceStatus,
    /// Assistant connection status.
    pub assistant_connection: GraphqlAssistantConnection,
    /// Memory storage status.
    pub memory_storage: GraphqlMemoryStorageStatus,
    /// Current primary agent display name, if the agent has been named.
    pub primary_agent_display_name: Option<String>,
}

pub(super) async fn local_status(state: &GraphqlState) -> Result<GraphqlLocalStatus> {
    let primary_agent_display_name = match state.optional_store() {
        Some(store) => store
            .get_agent("agent:primary")
            .await
            .map_err(graphql_error)?
            .and_then(|agent| agent.display_name),
        None => None,
    };

    Ok(GraphqlLocalStatus {
        local_service: GraphqlLocalServiceStatus::Running,
        assistant_connection: GraphqlAssistantConnection::Codex,
        memory_storage: state.memory_storage(),
        primary_agent_display_name,
    })
}
