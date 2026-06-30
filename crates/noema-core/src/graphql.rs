//! GraphQL client API facade.
//!
//! The GraphQL layer is the first-party client API. Resolvers must stay thin:
//! they call Noema read models, command/runtime paths, provider auth, and
//! repository methods instead of owning product behavior.

mod chat;
mod errors;
mod local_status;
mod memory;
mod onboarding;
mod resolvers;
mod schema;
mod subscriptions;
mod types;

pub use schema::{GraphqlSchema, GraphqlState, build_schema};
pub(crate) use subscriptions::{ConversationLiveEvent, ConversationSubscriptionRegistry};
