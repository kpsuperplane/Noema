//! GraphQL client API facade.
//!
//! The GraphQL layer is the first-party client API. Resolvers must stay thin:
//! they call Noema read models, command/runtime paths, provider auth, and
//! repository methods instead of owning product behavior.

mod agents;
mod artifacts;
mod chat;
mod errors;
mod local_status;
mod mcp;
mod memory;
mod onboarding;
mod provider_accounts;
mod provider_auth;
mod replay;
mod resolvers;
mod runtime_state;
mod schema;
mod subscriptions;
#[cfg(test)]
mod support_tests;
mod tasks;
mod usage_settings;
mod web_fetch_settings;
mod web_tool_settings;

pub use artifacts::{
    AuthorizedArtifactDownload, AuthorizedArtifactDownloadError, authorized_artifact_download,
};
pub use mcp::complete_mcp_server_oauth_setup;
pub(crate) use provider_auth::{
    ProviderAuthStartRequest, is_user_onboarded_for_chat,
    persist_provider_account_status_from_attempt, reconcile_onboarding_provider_account,
    start_provider_auth_attempt_view_from_parts,
};
pub(crate) use replay::{ConversationReplayItem, web_conversation_item_from_record};
pub(crate) use runtime_state::GraphqlRuntimeState;
pub use schema::{GraphqlSchema, GraphqlState, build_schema};
pub(crate) use subscriptions::{ConversationLiveEvent, ConversationSubscriptionRegistry};

/// Server-derived identity attached to every authenticated GraphQL operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestPrincipal {
    subject_id: &'static str,
}

impl RequestPrincipal {
    /// Return the single local authenticated subject.
    #[must_use]
    pub fn local() -> Self {
        Self {
            subject_id: "human:local",
        }
    }

    /// Return the stable subject identifier.
    #[must_use]
    pub fn subject_id(&self) -> &str {
        self.subject_id
    }
}
