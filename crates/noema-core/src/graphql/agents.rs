use async_graphql::{Result, SimpleObject};

use crate::AgentRecord;

use super::{errors::graphql_error, schema::GraphqlState};

/// Agent metadata safe to expose in read-only Settings.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAgent {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
    /// Whether this is Noema's built-in primary agent.
    pub is_primary: bool,
}

impl From<AgentRecord> for GraphqlAgent {
    fn from(agent: AgentRecord) -> Self {
        let is_primary = agent.agent_id == "agent:primary";
        Self {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
            is_primary,
        }
    }
}

pub(super) async fn agents(state: &GraphqlState) -> Result<Vec<GraphqlAgent>> {
    let store = state.store()?;
    let agents = store.list_agents().await.map_err(graphql_error)?;
    Ok(agents.into_iter().map(Into::into).collect())
}
