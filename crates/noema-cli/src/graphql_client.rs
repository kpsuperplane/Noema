//! Compatibility facade for the CLI GraphQL client.

#[allow(unused_imports)]
pub(crate) use crate::graphql::GraphqlTurnEvent;
pub(crate) use crate::graphql::{
    GraphqlRequest, execute, start_primary_conversation, stream_conversation_turn,
    validate_graphql_base_url,
};
