use async_graphql::{Result, SimpleObject};

use super::{errors::graphql_error, schema::GraphqlState};

/// Local status returned by `Query.localStatus`.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "LocalStatus")]
pub struct GraphqlLocalStatus {
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
        primary_agent_display_name,
    })
}
