//! GraphQL client API facade.
//!
//! The GraphQL layer is the first-party client API. Resolvers must stay thin:
//! they call Noema read models, command/runtime paths, provider auth, and
//! repository methods instead of owning product behavior.

mod agents;
mod chat;
mod errors;
mod local_status;
mod mcp;
mod memory;
mod onboarding;
mod provider_accounts;
mod resolvers;
mod runtime_state;
mod schema;
mod subscriptions;
mod usage_settings;
mod web_fetch_settings;
mod web_tool_settings;

pub use mcp::complete_mcp_server_oauth_setup;
pub(crate) use runtime_state::GraphqlRuntimeState;
pub use schema::{GraphqlSchema, GraphqlState, build_schema};
pub(crate) use subscriptions::{ConversationLiveEvent, ConversationSubscriptionRegistry};
